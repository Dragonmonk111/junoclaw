//! MAYO post-quantum signature host-function implementations for the
//! CosmWasm VM.
//!
//! This crate wraps the pure-Rust `junoclaw-mayo-verify` verifier and
//! exposes a single host-function entry point: [`mayo_verify`]. The
//! underlying verifier is `#![no_std]` + `alloc`, deterministic, and
//! has zero C dependencies, making it suitable for upstreaming into
//! `cosmwasm-vm` as an optional capability.
//!
//! # Supported variants
//!
//! | Code | Variant | NIST Level | PK bytes | Sig bytes |
//! |------|---------|------------|----------|-----------|
//! | 1    | MAYO-1  | L1         | 1 420    | 454       |
//! | 2    | MAYO-2  | L1         | 4 912    | 186       |
//! | 3    | MAYO-3  | L3         | 2 986    | 681       |
//! | 5    | MAYO-5  | L5         | 5 554    | 964       |
//!
//! # Determinism
//!
//! - No randomness, no wall-clock reads, no threading.
//! - All field arithmetic is constant-time in the verifier path.
//! - The AES-128-CTR expansion of the public key is fully deterministic.
//!
//! # Gas semantics
//!
//! See [`gas`] for per-variant cost constants. Costs are expressed in
//! CosmWasm VM gas (100× SDK gas) to match the existing BLS12-381 and
//! BN254 host-function conventions.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(unsafe_code)]

extern crate alloc;

pub mod errors;
pub mod gas;

pub use errors::MayoError;

use junoclaw_mayo_verify::{Mayo1, Mayo2, Mayo3, Mayo5, ParameterSet};

/// Verify a MAYO signature via the host-function precompile.
///
/// * `variant` — one of `1` (MAYO-1), `2` (MAYO-2), `3` (MAYO-3), or `5`
///   (MAYO-5). Other values return [`MayoError::UnknownVariant`].
/// * `pk` — compact public key bytes (length validated per variant).
/// * `msg` — arbitrary-length message bytes.
/// * `sig` — signature bytes (length validated per variant).
///
/// Returns `Ok(true)` for a valid signature, `Ok(false)` for an invalid
/// signature, and `Err(MayoError)` for malformed inputs or internal errors.
pub fn mayo_verify(variant: u32, pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<bool, MayoError> {
    match variant {
        1 => verify_checked::<Mayo1>(pk, msg, sig),
        2 => verify_checked::<Mayo2>(pk, msg, sig),
        3 => verify_checked::<Mayo3>(pk, msg, sig),
        5 => verify_checked::<Mayo5>(pk, msg, sig),
        v => Err(MayoError::UnknownVariant(v)),
    }
}

/// Verify after checking lengths so the underlying `verify` receives
/// well-sized slices and returns `VerifyFailed` rather than `InvalidLength`
/// on bad data.
fn verify_checked<P: ParameterSet>(
    pk: &[u8],
    msg: &[u8],
    sig: &[u8],
) -> Result<bool, MayoError> {
    if pk.len() != P::PK_BYTES {
        return Err(MayoError::InvalidInputLength {
            variant: P::NAME,
            field: "public_key",
            expected: P::PK_BYTES,
            actual: pk.len(),
        });
    }
    if sig.len() != P::SIG_BYTES {
        return Err(MayoError::InvalidInputLength {
            variant: P::NAME,
            field: "signature",
            expected: P::SIG_BYTES,
            actual: sig.len(),
        });
    }
    junoclaw_mayo_verify::verify::<P>(msg, sig, pk)
        .map_err(|e| map_error::<P>(e))
}

fn map_error<P: ParameterSet>(e: junoclaw_mayo_verify::Error) -> MayoError {
    match e {
        junoclaw_mayo_verify::Error::InvalidLength { expected, actual } => {
            MayoError::InvalidInputLength {
                variant: P::NAME,
                field: "input",
                expected,
                actual,
            }
        }
        junoclaw_mayo_verify::Error::VerifyFailed => MayoError::InvalidSignature,
        junoclaw_mayo_verify::Error::AesError => MayoError::InternalError,
        junoclaw_mayo_verify::Error::ShakeError => MayoError::InternalError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn rejects_unknown_variant() {
        let err = mayo_verify(4, &[], &[], &[]).unwrap_err();
        assert!(matches!(err, MayoError::UnknownVariant(4)));
    }

    #[test]
    fn rejects_bad_lengths() {
        // MAYO-2 expects 4912 B PK and 186 B sig
        let err = mayo_verify(2, &[0u8; 100], b"msg", &[0u8; 100]).unwrap_err();
        assert!(matches!(
            err,
            MayoError::InvalidInputLength {
                variant: "MAYO-2",
                field: "public_key",
                expected: 4912,
                actual: 100,
            }
        ));
    }

    #[test]
    fn rejects_garbage_signature() {
        // Zeroed PK + sig should fail verification (not error)
        let pk = vec![0u8; Mayo2::PK_BYTES];
        let sig = vec![0u8; Mayo2::SIG_BYTES];
        let valid = mayo_verify(2, &pk, b"msg", &sig).unwrap();
        assert!(!valid);
    }
}
