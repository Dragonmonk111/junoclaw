//! Groth16 verification backend — pure-arkworks by default, BN254
//! precompile path behind `--features bn254-precompile`.
//!
//! Both paths expose the same [`verify_groth16`] signature. The contract
//! entry point in `contract.rs` calls through this module so that the
//! only code difference between the two build flavours is which module
//! this file compiles down to.
//!
//! # Why two paths
//!
//! The default path is what currently runs on Juno uni-7 and consumes
//! ~371 486 gas per `VerifyProof`. The precompile path targets a wasmvm
//! fork that exposes `bn254_add`, `bn254_scalar_mul`, and
//! `bn254_pairing_equality` as host functions. On that fork, the same
//! verification lands at ~187 000 gas (~2× reduction) — the headline
//! claim of `docs/BN254_PRECOMPILE_CASE.md`.
//!
//! The precompile path uses **all three** host functions (not just
//! `pairing_equality`) so that the governance-proposal benchmark
//! exercises the whole surface end-to-end.

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Proof, VerifyingKey};

use crate::error::ContractError;

// `Groth16`, `G1Affine`, `G2Affine`, and arkworks trait methods are only used
// in one of the two feature-gated paths, so we import them inside each path
// rather than at the module root to avoid dead-code warnings.
#[cfg(not(feature = "bn254-precompile"))]
use ark_groth16::Groth16;

/// Verify a Groth16 proof against the supplied verification key and
/// public inputs. Returns `Ok(true)` on a valid proof, `Ok(false)` on a
/// well-formed but failing proof, and `Err` on malformed input or a
/// precompile host error.
///
/// The implementation dispatches on the `bn254-precompile` cargo feature:
///
/// - **off (default):** `Groth16::<Bn254>::verify_proof` via arkworks.
/// - **on:** custom 4-pair equality form evaluated through
///   `bn254_pairing_equality`, with the `vk_x` linear combination also
///   computed through `bn254_scalar_mul` + `bn254_add`.
pub fn verify_groth16(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_inputs: &[Fr],
) -> Result<bool, ContractError> {
    #[cfg(not(feature = "bn254-precompile"))]
    {
        verify_pure_wasm(vk, proof, public_inputs)
    }
    #[cfg(feature = "bn254-precompile")]
    {
        precompile::verify_via_precompile(vk, proof, public_inputs)
    }
}

// ── Pure-Wasm path (default) ──────────────────────────────────────────────

#[cfg(not(feature = "bn254-precompile"))]
fn verify_pure_wasm(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_inputs: &[Fr],
) -> Result<bool, ContractError> {
    let pvk = ark_groth16::prepare_verifying_key(vk);
    Groth16::<Bn254>::verify_proof(&pvk, proof, public_inputs).map_err(|e| {
        ContractError::DeserializationError {
            reason: format!("verify error: {e}"),
        }
    })
}

// ── Precompile path (feature-gated) ───────────────────────────────────────

#[cfg(feature = "bn254-precompile")]
mod precompile {
    use super::*;
    use ark_bn254::{Fq, G1Affine, G2Affine};
    use ark_ec::{AffineRepr, CurveGroup};
    use ark_ff::{BigInteger, PrimeField};

    use cosmwasm_std_bn254_ext::{
        bn254_add_call, bn254_pairing_equality_call, bn254_scalar_mul_call, Bn254ExtError,
    };

    /// Byte size constants (mirror `wasmvm-fork/cosmwasm-crypto-bn254`).
    const FQ: usize = 32;
    const FR: usize = 32;
    const G1: usize = 64;
    const G2: usize = 128;
    const PAIR: usize = G1 + G2;

    pub(super) fn verify_via_precompile(
        vk: &VerifyingKey<Bn254>,
        proof: &Proof<Bn254>,
        public_inputs: &[Fr],
    ) -> Result<bool, ContractError> {
        // ─── 1. Compute vk_x = γ_abc[0] + Σ input[i]·γ_abc[i+1] via host ──
        //
        // This exercises both `bn254_scalar_mul` and `bn254_add`. We keep
        // the accumulator in encoded-bytes form so every intermediate
        // value round-trips through the host — that way any encoding bug
        // shows up in the lincomb rather than silently passing the proof
        // check.
        if vk.gamma_abc_g1.len() != public_inputs.len() + 1 {
            return Err(ContractError::DeserializationError {
                reason: format!(
                    "public_inputs length {} != gamma_abc_g1 length - 1 ({})",
                    public_inputs.len(),
                    vk.gamma_abc_g1.len() - 1
                ),
            });
        }

        let mut acc_bytes: [u8; G1] = encode_g1(&vk.gamma_abc_g1[0]);
        for (i, x) in public_inputs.iter().enumerate() {
            let mut mul_input = [0u8; G1 + FR];
            mul_input[..G1].copy_from_slice(&encode_g1(&vk.gamma_abc_g1[i + 1]));
            mul_input[G1..].copy_from_slice(&encode_fr(x));
            let term = bn254_scalar_mul_call(&mul_input).map_err(ext_err)?;
            let mut add_input = [0u8; 2 * G1];
            add_input[..G1].copy_from_slice(&acc_bytes);
            add_input[G1..].copy_from_slice(&term);
            let sum = bn254_add_call(&add_input).map_err(ext_err)?;
            acc_bytes = slice_to_64(&sum)?;
        }

        // ─── 2. Assemble the 4-pair pairing equality input ───────────────
        //
        // e(A, B) · e(-α, β) · e(-vk_x, γ) · e(-C, δ) = 1
        //
        // The negations on G1 are free (flip y in Fq), so we do those in
        // arkworks rather than burning another host call.
        let neg_alpha = (-vk.alpha_g1.into_group()).into_affine();
        let neg_c = (-proof.c.into_group()).into_affine();
        let neg_vk_x_bytes = negate_g1_bytes(&acc_bytes);

        let mut input = [0u8; 4 * PAIR];

        // pair 0: (A, B)
        copy_pair(&mut input[0..PAIR], &encode_g1(&proof.a), &encode_g2(&proof.b));
        // pair 1: (-α, β)
        copy_pair(
            &mut input[PAIR..2 * PAIR],
            &encode_g1(&neg_alpha),
            &encode_g2(&vk.beta_g2),
        );
        // pair 2: (-vk_x, γ)
        copy_pair(
            &mut input[2 * PAIR..3 * PAIR],
            &neg_vk_x_bytes,
            &encode_g2(&vk.gamma_g2),
        );
        // pair 3: (-C, δ)
        copy_pair(
            &mut input[3 * PAIR..4 * PAIR],
            &encode_g1(&neg_c),
            &encode_g2(&vk.delta_g2),
        );

        // ─── 3. One host call covers the whole pairing ──────────────────
        bn254_pairing_equality_call(&input).map_err(ext_err)
    }

    // ── Encoders / decoders ────────────────────────────────────────────

    fn encode_fq(fq: &Fq) -> [u8; FQ] {
        let be = fq.into_bigint().to_bytes_be();
        let mut out = [0u8; FQ];
        let pad = FQ.saturating_sub(be.len());
        out[pad..].copy_from_slice(&be);
        out
    }

    fn encode_fr(fr: &Fr) -> [u8; FR] {
        let be = fr.into_bigint().to_bytes_be();
        let mut out = [0u8; FR];
        let pad = FR.saturating_sub(be.len());
        out[pad..].copy_from_slice(&be);
        out
    }

    fn encode_g1(p: &G1Affine) -> [u8; G1] {
        let mut out = [0u8; G1];
        if p.is_zero() {
            return out;
        }
        out[..FQ].copy_from_slice(&encode_fq(&p.x));
        out[FQ..].copy_from_slice(&encode_fq(&p.y));
        out
    }

    fn encode_g2(p: &G2Affine) -> [u8; G2] {
        let mut out = [0u8; G2];
        if p.is_zero() {
            return out;
        }
        out[0..FQ].copy_from_slice(&encode_fq(&p.x.c1));
        out[FQ..2 * FQ].copy_from_slice(&encode_fq(&p.x.c0));
        out[2 * FQ..3 * FQ].copy_from_slice(&encode_fq(&p.y.c1));
        out[3 * FQ..].copy_from_slice(&encode_fq(&p.y.c0));
        out
    }

    /// Negate an encoded G1 point by flipping the y coordinate in Fq.
    /// `(0, 0)` stays `(0, 0)` (the point at infinity is self-negating).
    fn negate_g1_bytes(bytes: &[u8; G1]) -> [u8; G1] {
        if bytes.iter().all(|&b| b == 0) {
            return [0u8; G1];
        }
        let mut out = [0u8; G1];
        out[..FQ].copy_from_slice(&bytes[..FQ]); // x stays the same
        let y = Fq::from_be_bytes_mod_order(&bytes[FQ..]);
        let neg_y = -y;
        out[FQ..].copy_from_slice(&encode_fq(&neg_y));
        out
    }

    fn slice_to_64(v: &[u8]) -> Result<[u8; 64], ContractError> {
        v.try_into().map_err(|_| ContractError::PrecompileError {
            reason: format!("host returned {} bytes, expected 64", v.len()),
        })
    }

    fn copy_pair(dst: &mut [u8], g1: &[u8; G1], g2: &[u8; G2]) {
        debug_assert_eq!(dst.len(), PAIR);
        dst[..G1].copy_from_slice(g1);
        dst[G1..].copy_from_slice(g2);
    }

    fn ext_err(e: Bn254ExtError) -> ContractError {
        ContractError::PrecompileError {
            reason: e.to_string(),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────
//
// The unit tests in `tests.rs` exercise the default path end-to-end. The
// precompile path is exercised by the devnet-level test documented in
// `../../wasmvm-fork/BUILD_AND_TEST.md`: the same .wasm is built with and
// without the feature flag, deployed side by side on the patched devnet,
// and asked to verify 1 000 random proofs. Any difference in acceptance
// decision fails the test.
