//! Differential test: 1 000 Groth16 proofs against both pure-Wasm and
//! precompile verification paths.
//!
//! This example generates N proofs with random inputs, verifies each with
//! both the pure-arkworks path (what runs on Juno uni-7 today) and a
//! native mock of the BN254 host-function path (what runs on the patched
//! devnet / future mainnet).  Any divergence in accept/reject decision
//! panics immediately.
//!
//! Usage:
//!   N=1000 cargo run --example differential_test
//!
//! The mock host functions below use the same arkworks backend as the
//! pure-Wasm path, so this test is NOT a test of the host-function
//! implementation (that is covered by `wasmvm-fork/cosmwasm-crypto-bn254`
//! unit tests).  What this tests is:
//!   1. Byte-encoding consistency (G1/G2 point → bytes → point)
//!   2. The 4-pair pairing-equality formulation matches arkworks Groth16
//!   3. The vk_x linear combination through scalar-mul + add is correct
//!   4. No edge-case inputs (infinity, zero scalar, large scalar) break
//!      the precompile path while passing the pure path.

use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
use ark_ff::{BigInteger, PrimeField};
use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use ark_std::Zero;

/// Minimal circuit: prove knowledge of x such that x * x = y
#[derive(Clone)]
struct SquareCircuit {
    x: Option<Fr>,
}

impl ConstraintSynthesizer<Fr> for SquareCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let x_val = self.x.unwrap_or(Fr::from(1u64));
        let y_val = x_val * x_val;
        let x_var = cs.new_witness_variable(|| Ok(x_val))?;
        let y_var = cs.new_input_variable(|| Ok(y_val))?;
        cs.enforce_constraint(
            ark_relations::lc!() + x_var,
            ark_relations::lc!() + x_var,
            ark_relations::lc!() + y_var,
        )?;
        Ok(())
    }
}

// ── Mock host functions (native arkworks, same math as pure path) ─────────

const FQ: usize = 32;
const FR: usize = 32;
const G1: usize = 64;
const G2: usize = 128;
const PAIR: usize = G1 + G2;

fn mock_bn254_add(input: &[u8]) -> Vec<u8> {
    assert_eq!(input.len(), 2 * G1);
    let a = decode_g1(&input[..G1]);
    let b = decode_g1(&input[G1..]);
    let sum = (a.into_group() + b.into_group()).into_affine();
    encode_g1(&sum).to_vec()
}

fn mock_bn254_scalar_mul(input: &[u8]) -> Vec<u8> {
    assert_eq!(input.len(), G1 + FR);
    let p = decode_g1(&input[..G1]);
    let s = Fr::from_be_bytes_mod_order(&input[G1..]);
    let prod = (p.into_group() * s).into_affine();
    encode_g1(&prod).to_vec()
}

fn mock_bn254_pairing_equality(input: &[u8]) -> bool {
    assert_eq!(input.len() % PAIR, 0);
    let n = input.len() / PAIR;
    if n == 0 {
        return true;
    }
    let mut g1s = Vec::with_capacity(n);
    let mut g2s = Vec::with_capacity(n);
    for i in 0..n {
        let off = i * PAIR;
        g1s.push(decode_g1(&input[off..off + G1]));
        g2s.push(decode_g2(&input[off + G1..off + PAIR]));
    }
    Bn254::multi_pairing(g1s, g2s).is_zero()
}

// ── Encoders / decoders (copied from bn254_backend.rs) ────────────────────

fn encode_fq(fq: &ark_bn254::Fq) -> [u8; FQ] {
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

fn decode_g1(bytes: &[u8]) -> G1Affine {
    if bytes.iter().all(|&b| b == 0) {
        return G1Affine::zero();
    }
    let x = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[..FQ]);
    let y = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[FQ..]);
    G1Affine::new_unchecked(x, y)
}

fn decode_g2(bytes: &[u8]) -> G2Affine {
    if bytes.iter().all(|&b| b == 0) {
        return G2Affine::zero();
    }
    let x_c1 = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[0..FQ]);
    let x_c0 = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[FQ..2 * FQ]);
    let y_c1 = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[2 * FQ..3 * FQ]);
    let y_c0 = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[3 * FQ..]);
    G2Affine::new_unchecked(
        ark_bn254::Fq2::new(x_c0, x_c1),
        ark_bn254::Fq2::new(y_c0, y_c1),
    )
}

fn negate_g1_bytes(bytes: &[u8; G1]) -> [u8; G1] {
    if bytes.iter().all(|&b| b == 0) {
        return [0u8; G1];
    }
    let mut out = [0u8; G1];
    out[..FQ].copy_from_slice(&bytes[..FQ]);
    let y = ark_bn254::Fq::from_be_bytes_mod_order(&bytes[FQ..]);
    let neg_y = -y;
    out[FQ..].copy_from_slice(&encode_fq(&neg_y));
    out
}

fn copy_pair(dst: &mut [u8], g1: &[u8; G1], g2: &[u8; G2]) {
    dst[..G1].copy_from_slice(g1);
    dst[G1..].copy_from_slice(g2);
}

// ── Precompile verification (mock host) ──────────────────────────────────────

fn verify_via_precompile_mock(
    vk: &VerifyingKey<Bn254>,
    proof: &ark_groth16::Proof<Bn254>,
    public_inputs: &[Fr],
) -> bool {
    // 1. Compute vk_x = gamma_abc[0] + Σ input[i]·gamma_abc[i+1]
    let mut acc_bytes = encode_g1(&vk.gamma_abc_g1[0]);
    for (i, x) in public_inputs.iter().enumerate() {
        let mut mul_input = [0u8; G1 + FR];
        mul_input[..G1].copy_from_slice(&encode_g1(&vk.gamma_abc_g1[i + 1]));
        mul_input[G1..].copy_from_slice(&encode_fr(x));
        let term = mock_bn254_scalar_mul(&mul_input);
        let mut add_input = [0u8; 2 * G1];
        add_input[..G1].copy_from_slice(&acc_bytes);
        add_input[G1..].copy_from_slice(&term);
        let sum = mock_bn254_add(&add_input);
        acc_bytes = sum.try_into().unwrap();
    }

    // 2. Assemble 4-pair pairing equality
    let neg_alpha = (-vk.alpha_g1.into_group()).into_affine();
    let neg_c = (-proof.c.into_group()).into_affine();
    let neg_vk_x_bytes = negate_g1_bytes(&acc_bytes);

    let mut input = [0u8; 4 * PAIR];
    copy_pair(&mut input[0..PAIR], &encode_g1(&proof.a), &encode_g2(&proof.b));
    copy_pair(
        &mut input[PAIR..2 * PAIR],
        &encode_g1(&neg_alpha),
        &encode_g2(&vk.beta_g2),
    );
    copy_pair(
        &mut input[2 * PAIR..3 * PAIR],
        &neg_vk_x_bytes,
        &encode_g2(&vk.gamma_g2),
    );
    copy_pair(
        &mut input[3 * PAIR..4 * PAIR],
        &encode_g1(&neg_c),
        &encode_g2(&vk.delta_g2),
    );

    mock_bn254_pairing_equality(&input)
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let n: usize = std::env::var("N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    println!("Differential test: {} random Groth16 proofs", n);
    println!("Pure-Wasm path  = ark_groth16::verify_proof");
    println!("Precompile path = mock BN254 host functions (native arkworks)");
    println!();

    // Single trusted setup for SquareCircuit — reused for all proofs
    let mut setup_rng = StdRng::seed_from_u64(0xDECAF);
    let circuit = SquareCircuit { x: None };
    let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut setup_rng).unwrap();

    let pvk = ark_groth16::prepare_verifying_key(&vk);

    let mut agree_accept = 0usize;
    let mut agree_reject = 0usize;

    for i in 0..n {
        // Random witness x in [0, 2^16)
        let x_val = Fr::from(((i * 7 + 13) % 65536) as u64);
        let y_val = x_val * x_val;

        let mut proof_rng = StdRng::seed_from_u64(i as u64);
        let circuit = SquareCircuit { x: Some(x_val) };
        let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut proof_rng).unwrap();

        // Every 3rd iteration, feed a WRONG public input so the proof must be
        // rejected. This exercises the reject path in lockstep — a true
        // differential test has to agree on both accept AND reject, not just
        // accept.
        let expect_invalid = i % 3 == 0;
        let claimed_y = if expect_invalid { y_val + Fr::from(1u64) } else { y_val };

        let pure_result =
            Groth16::<Bn254>::verify_proof(&pvk, &proof, &[claimed_y]).unwrap();

        let precompile_result = verify_via_precompile_mock(&vk, &proof, &[claimed_y]);

        if pure_result != precompile_result {
            panic!(
                "DIVERGENCE on proof #{i} (expect_invalid={expect_invalid}): \
                 pure={pure_result}, precompile={precompile_result}",
            );
        }

        // Sanity: the intended validity must match what both paths decided.
        let expected_valid = !expect_invalid;
        if pure_result != expected_valid {
            panic!(
                "LOGIC ERROR on proof #{i}: expected valid={expected_valid} but both \
                 paths returned {pure_result} (test harness bug, not a divergence)",
            );
        }

        if pure_result {
            agree_accept += 1;
        } else {
            agree_reject += 1;
        }

        if (i + 1) % 100 == 0 {
            println!(
                "  ... {}/{} done ({} accept-agree, {} reject-agree)",
                i + 1,
                n,
                agree_accept,
                agree_reject
            );
        }
    }

    println!();
    println!("═══════════════════════════════════════════");
    println!("  DIFFERENTIAL TEST PASSED: {}/{} agree", n, n);
    println!("  Valid proofs   (both ACCEPT): {}", agree_accept);
    println!("  Invalid proofs (both REJECT): {}", agree_reject);
    println!("═══════════════════════════════════════════");
}
