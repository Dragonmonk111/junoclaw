//! ML-DSA (FIPS 204) post-quantum signature host-function implementations for
//! the CosmWasm VM.
//!
//! This crate wraps the pure-Rust, `#![no_std]`, integer-only `fips204`
//! verifier and exposes a single host-function entry point: [`ml_dsa_verify`].
//! Verification is deterministic and RNG-free (the `default-rng`/`getrandom`
//! feature of `fips204` is deliberately disabled), making it suitable for an
//! optional `cosmwasm-vm` capability alongside the existing BLS12-381, BN254,
//! and MAYO host functions.
//!
//! # Supported variants
//!
//! | Code | Variant   | NIST cat | PK bytes | Sig bytes |
//! |------|-----------|----------|----------|-----------|
//! | 44   | ML-DSA-44 | 2        | 1 312    | 2 420     |
//! | 65   | ML-DSA-65 | 3        | 1 952    | 3 309     |
//! | 87   | ML-DSA-87 | 5        | 2 592    | 4 627     |
//!
//! # Determinism
//!
//! - No randomness, no wall-clock reads, no threading in the verify path.
//! - ML-DSA verification is **integer-only** — no floating point — so it
//!   verifies bit-for-bit identically across validator hardware. This is the
//!   property that makes ML-DSA (not Falcon) the right choice at a consensus
//!   trust root (see `docs/PROJECT_AEGIS_JUNO_FULL_PQC.md` §6).
//!
//! # Context string
//!
//! This entry point verifies with an **empty** FIPS 204 context string. Any
//! domain separation is handled at the application layer (e.g. by the message
//! the contract constructs), mirroring how MAYO attestations are bound.
//!
//! # Gas semantics
//!
//! See [`gas`] for per-variant cost constants (currently pre-measurement
//! estimates pending the Phase B devnet benchmark).

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod errors;
pub mod gas;

pub use errors::MlDsaError;

use fips204::traits::{SerDes, Verifier};

/// Verify an ML-DSA signature via the host-function precompile.
///
/// * `variant` — one of `44` (ML-DSA-44), `65` (ML-DSA-65), or `87`
///   (ML-DSA-87). Other values return [`MlDsaError::UnknownVariant`].
/// * `pk` — public-key bytes (length validated per variant).
/// * `msg` — arbitrary-length message bytes.
/// * `sig` — signature bytes (length validated per variant).
///
/// Returns `Ok(true)` for a valid signature, `Ok(false)` for a well-formed but
/// invalid signature, and `Err(MlDsaError)` for malformed inputs.
pub fn ml_dsa_verify(variant: u32, pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<bool, MlDsaError> {
    match variant {
        44 => verify_44(pk, msg, sig),
        65 => verify_65(pk, msg, sig),
        87 => verify_87(pk, msg, sig),
        v => Err(MlDsaError::UnknownVariant(v)),
    }
}

macro_rules! impl_ml_dsa_verify {
    ($fn_name:ident, $module:path, $name:literal) => {
        fn $fn_name(pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<bool, MlDsaError> {
            use $module as m;
            let pk_arr: [u8; m::PK_LEN] = pk.try_into().map_err(|_| {
                MlDsaError::InvalidInputLength {
                    variant: $name,
                    field: "public_key",
                    expected: m::PK_LEN,
                    actual: pk.len(),
                }
            })?;
            let sig_arr: [u8; m::SIG_LEN] = sig.try_into().map_err(|_| {
                MlDsaError::InvalidInputLength {
                    variant: $name,
                    field: "signature",
                    expected: m::SIG_LEN,
                    actual: sig.len(),
                }
            })?;
            let public_key =
                m::PublicKey::try_from_bytes(pk_arr).map_err(|_| MlDsaError::InvalidPublicKey)?;
            Ok(public_key.verify(msg, &sig_arr, &[]))
        }
    };
}

impl_ml_dsa_verify!(verify_44, fips204::ml_dsa_44, "ML-DSA-44");
impl_ml_dsa_verify!(verify_65, fips204::ml_dsa_65, "ML-DSA-65");
impl_ml_dsa_verify!(verify_87, fips204::ml_dsa_87, "ML-DSA-87");

#[cfg(test)]
mod tests {
    use super::*;
    use fips204::traits::{KeyGen, Signer};

    /// Round-trip a deterministic keygen+sign and check the host function
    /// accepts the valid signature and rejects a tampered message.
    fn roundtrip(variant: u32, pk_bytes: &[u8], sig: &[u8], msg: &[u8]) {
        assert!(ml_dsa_verify(variant, pk_bytes, msg, sig).unwrap());
        assert!(!ml_dsa_verify(variant, pk_bytes, b"tampered message", sig).unwrap());
    }

    #[test]
    fn verifies_ml_dsa_44() {
        let (pk, sk) = fips204::ml_dsa_44::KG::keygen_from_seed(&[7u8; 32]);
        let msg = b"aegis phase B :: ml-dsa-44";
        let sig = sk.try_sign_with_seed(&[9u8; 32], msg, &[]).unwrap();
        roundtrip(44, &pk.into_bytes(), &sig, msg);
    }

    #[test]
    fn verifies_ml_dsa_65() {
        let (pk, sk) = fips204::ml_dsa_65::KG::keygen_from_seed(&[11u8; 32]);
        let msg = b"aegis phase B :: ml-dsa-65";
        let sig = sk.try_sign_with_seed(&[13u8; 32], msg, &[]).unwrap();
        roundtrip(65, &pk.into_bytes(), &sig, msg);
    }

    #[test]
    fn verifies_ml_dsa_87() {
        let (pk, sk) = fips204::ml_dsa_87::KG::keygen_from_seed(&[17u8; 32]);
        let msg = b"aegis phase B :: ml-dsa-87";
        let sig = sk.try_sign_with_seed(&[19u8; 32], msg, &[]).unwrap();
        roundtrip(87, &pk.into_bytes(), &sig, msg);
    }

    #[test]
    fn rejects_unknown_variant() {
        let err = ml_dsa_verify(7, &[], &[], &[]).unwrap_err();
        assert!(matches!(err, MlDsaError::UnknownVariant(7)));
    }

    #[test]
    fn rejects_bad_pk_length() {
        let err = ml_dsa_verify(44, &[0u8; 10], b"x", &[0u8; 2420]).unwrap_err();
        assert!(matches!(
            err,
            MlDsaError::InvalidInputLength {
                variant: "ML-DSA-44",
                field: "public_key",
                expected: 1312,
                actual: 10,
            }
        ));
    }

    #[test]
    fn rejects_garbage_signature() {
        // Valid-length but all-zero pk+sig: must fail verification, not error.
        // (A zero pk may fail to decode; both InvalidPublicKey and Ok(false)
        // are acceptable "not verified" outcomes — assert it is never Ok(true).)
        let pk = [0u8; 1312];
        let sig = [0u8; 2420];
        let result = ml_dsa_verify(44, &pk, b"msg", &sig);
        assert!(!matches!(result, Ok(true)));
    }
}
