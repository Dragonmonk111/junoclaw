//! Pure-Rust MAYO signature verifier for CosmWasm (`no_std` + `alloc`).
//!
//! This crate implements MAYO signature verification without any C dependencies,
//! making it suitable for `wasm32-unknown-unknown` targets (e.g. CosmWasm contracts).
//!
//! Supports all NIST Round 4 parameter sets — **MAYO-1/2/3/5** — each
//! cross-checked bit-for-bit against the MAYO-C reference implementation.
//! MAYO-2 (186-byte signatures) is recommended for on-chain use at Level 1;
//! MAYO-5 (964-byte signatures) provides NIST Level 5.
//!
//! # Usage
//! ```ignore
//! use junoclaw_mayo_verify::Mayo2;
//!
//! let valid = Mayo2::verify(message, signature, public_key).unwrap();
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "test-c"), forbid(unsafe_code))]

extern crate alloc;

pub mod error;
pub mod gf16;
pub mod params;
pub mod verify;

pub use error::{Error, Result};
pub use params::{Mayo1, Mayo2, Mayo3, Mayo5, ParameterSet};
pub use verify::verify;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mayo2_verify_rejects_garbage() {
        let msg = b"test";
        let sig = [0u8; Mayo2::SIG_BYTES];
        let pk = [0u8; Mayo2::PK_BYTES];
        assert!(!verify::<Mayo2>(msg, &sig, &pk).unwrap());
    }

    /// Cross-check harness against the reference C implementation
    /// (sriracha-mayo). Generates a keypair with the C crate, signs a message,
    /// and asserts that our pure-Rust verifier accepts the signature and
    /// rejects a tampered message. Macro-based so each expansion uses concrete
    /// types (sriracha-mayo's variant trait is not publicly nameable).
    #[cfg(feature = "test-c")]
    macro_rules! cross_check_test {
        ($name:ident, $p:ty, $c:ty) => {
            #[test]
            fn $name() {
                use rand_chacha::ChaCha20Rng;
                use rand_core::SeedableRng;

                let mut rng = ChaCha20Rng::from_seed([42; 32]);
                let msg = b"hello mayo pure-rust verifier";

                let (pk, sk) = sriracha_mayo::SecretKey::<$c>::random(&mut rng).unwrap();
                let sig = sk.sign(msg).unwrap();

                // C impl should accept
                assert!(
                    sig.verify(&pk, msg),
                    "{}: C verify rejected own sig",
                    <$p as ParameterSet>::NAME
                );

                // Our verifier should accept the signature
                assert!(
                    verify::<$p>(msg, sig.as_ref(), pk.as_ref()).unwrap(),
                    "{}: pure-Rust verifier rejected valid sig",
                    <$p as ParameterSet>::NAME
                );

                // Tampered message should fail
                let bad_msg = b"tampered message";
                assert!(
                    !verify::<$p>(bad_msg, sig.as_ref(), pk.as_ref()).unwrap(),
                    "{}: pure-Rust verifier accepted tampered msg",
                    <$p as ParameterSet>::NAME
                );
            }
        };
    }

    #[cfg(feature = "test-c")]
    cross_check_test!(test_mayo1_cross_check_sriracha, Mayo1, sriracha_mayo::Mayo1);
    #[cfg(feature = "test-c")]
    cross_check_test!(test_mayo2_cross_check_sriracha, Mayo2, sriracha_mayo::Mayo2);
    #[cfg(feature = "test-c")]
    cross_check_test!(test_mayo3_cross_check_sriracha, Mayo3, sriracha_mayo::Mayo3);
    #[cfg(feature = "test-c")]
    cross_check_test!(test_mayo5_cross_check_sriracha, Mayo5, sriracha_mayo::Mayo5);

    /// Verify our public-key expansion matches the C implementation bit-for-bit.
    #[cfg(feature = "test-c")]
    #[test]
    fn test_mayo2_expand_pk_matches_c() {
        use sriracha_mayo::Mayo2 as CMayo2;
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;
        use core::ffi::c_int;

        extern "C" {
            fn pqmayo_MAYO_2_opt_mayo_expand_pk(pk: *mut u64, cpk: *const u8) -> c_int;
        }

        let mut rng = ChaCha20Rng::from_seed([42; 32]);
        let (cpk_obj, _sk) = sriracha_mayo::SecretKey::<CMayo2>::random(&mut rng).unwrap();
        let cpk = cpk_obj.as_ref();

        // C expansion
        let mut c_epk = vec![0u64; Mayo2::EPK_LIMBS];
        let ret = unsafe { pqmayo_MAYO_2_opt_mayo_expand_pk(c_epk.as_mut_ptr(), cpk.as_ptr()) };
        assert_eq!(ret, 0, "C expand_pk failed");

        // Rust expansion
        let (rust_p1, rust_p2, rust_p3) = verify::expand_pk::<Mayo2>(cpk).unwrap();
        let mut rust_epk = Vec::new();
        rust_epk.extend_from_slice(&rust_p1);
        rust_epk.extend_from_slice(&rust_p2);
        rust_epk.extend_from_slice(&rust_p3);

        assert_eq!(c_epk, rust_epk, "expanded PK mismatch");
    }
}
