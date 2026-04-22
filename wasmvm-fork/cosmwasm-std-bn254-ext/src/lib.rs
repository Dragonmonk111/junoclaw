//! Guest-side shim exposing the three BN254 host functions to CosmWasm
//! contracts **without** requiring a fork of `cosmwasm-std`.
//!
//! Once `cosmwasm-std` absorbs the `Api::bn254_*` methods (see the patch
//! `../patches/cosmwasm-std.traits.rs.patch`) this crate becomes
//! redundant and can be deleted. Until then it is the unblocking piece
//! that lets `contracts/zk-verifier` compile with
//! `--features bn254-precompile` against a stock `cosmwasm-std` release.
//!
//! # Design notes
//!
//! The FFI convention exactly matches the one used for BLS12-381 in
//! `cosmwasm-std` 2.x: a single `Region` struct is passed to the host
//! containing `(offset, capacity, length)` of the input buffer, the host
//! returns a pointer to a result `Region`, and the guest reclaims that
//! region via the standard Wasm allocator used by `cosmwasm-std`.
//!
//! The `Region` representation and the `build_region` / `consume_region`
//! helpers below are **byte-for-byte copies** of the private
//! `cosmwasm-std::memory` internals (Apache-2.0). The duplication is
//! unfortunate but lets us ship the precompile demo without a vendored
//! cosmwasm-std fork.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

// ── Region FFI plumbing (copied from cosmwasm-std 2.2, Apache-2.0) ─────────
//
// The items in this section are only referenced by the wasm32-specific
// `call_host_bytes` implementation. On native host builds they appear to be
// dead code, so we gate them behind the same cfg that consumes them.

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
use alloc::boxed::Box;

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
use core::mem;

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
#[repr(C)]
#[derive(Debug)]
struct Region {
    offset: u32,
    capacity: u32,
    length: u32,
}

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
fn build_region(data: &[u8]) -> Box<Region> {
    let data_ptr = data.as_ptr() as usize;
    Box::new(Region {
        offset: u32::try_from(data_ptr).expect("pointer doesn't fit in u32"),
        capacity: u32::try_from(data.len()).expect("length doesn't fit in u32"),
        length: u32::try_from(data.len()).expect("length doesn't fit in u32"),
    })
}

/// # Safety
/// The caller must guarantee that `ptr` was produced by the host's
/// allocator in response to one of our `bn254_*` import calls, and that
/// it has not already been consumed.
#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
unsafe fn consume_region(ptr: *mut Region) -> Vec<u8> {
    assert!(!ptr.is_null(), "Region pointer is null");
    let region = unsafe { Box::from_raw(ptr) };
    let region_start = region.offset as *mut u8;
    assert!(!region_start.is_null(), "Region starts at null pointer");
    unsafe {
        Vec::from_raw_parts(
            region_start,
            region.length as usize,
            region.capacity as usize,
        )
    }
}

// ── Host imports (only linked on Wasm, per feature flag) ──────────────────

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
extern "C" {
    fn bn254_add(input_ptr: u32) -> u32;
    fn bn254_scalar_mul(input_ptr: u32) -> u32;
    fn bn254_pairing_equality(input_ptr: u32) -> u32;
}

// ── Error type ────────────────────────────────────────────────────────────

/// Errors returned by the BN254 host-function wrappers. The wrapping
/// contract is expected to convert these into its own error type.
#[derive(Debug, Clone)]
pub enum Bn254ExtError {
    /// Non-Wasm target: the caller is linking this crate without the
    /// `runtime` feature, or on a non-`wasm32` triple. Returned by every
    /// public function so native tests can still link without crashing.
    NotAvailableOffChain,
    /// Input was the wrong size for this operation.
    InvalidInputLength {
        expected: usize,
        actual: usize,
    },
    /// Host returned an unexpected length of bytes.
    InvalidOutputLength {
        expected: usize,
        actual: usize,
    },
    /// Host returned a discriminant other than 0/1 for pairing equality.
    InvalidBooleanDiscriminant(u8),
    /// Host returned a non-zero error code with a message.
    HostError(String),
}

impl core::fmt::Display for Bn254ExtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAvailableOffChain => {
                f.write_str("BN254 precompile is only available in an on-chain Wasm build")
            }
            Self::InvalidInputLength { expected, actual } => write!(
                f,
                "BN254: invalid input length: expected {expected}, got {actual}"
            ),
            Self::InvalidOutputLength { expected, actual } => write!(
                f,
                "BN254: invalid host output length: expected {expected}, got {actual}"
            ),
            Self::InvalidBooleanDiscriminant(d) => {
                write!(f, "BN254: invalid boolean discriminant: {d}")
            }
            Self::HostError(msg) => write!(f, "BN254: host error: {msg}"),
        }
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Calls the `bn254_add` host function.
///
/// * `input` must be exactly 128 bytes (two 64-byte G1 points).
/// * Returns 64 bytes (the sum).
pub fn bn254_add_call(input: &[u8]) -> Result<Vec<u8>, Bn254ExtError> {
    if input.len() != 128 {
        return Err(Bn254ExtError::InvalidInputLength {
            expected: 128,
            actual: input.len(),
        });
    }
    let data = call_host_bytes(input, HostFn::Add)?;
    if data.len() != 64 {
        return Err(Bn254ExtError::InvalidOutputLength {
            expected: 64,
            actual: data.len(),
        });
    }
    Ok(data)
}

/// Calls the `bn254_scalar_mul` host function.
///
/// * `input` must be exactly 96 bytes (`g1_point || scalar_be`).
/// * Returns 64 bytes.
pub fn bn254_scalar_mul_call(input: &[u8]) -> Result<Vec<u8>, Bn254ExtError> {
    if input.len() != 96 {
        return Err(Bn254ExtError::InvalidInputLength {
            expected: 96,
            actual: input.len(),
        });
    }
    let data = call_host_bytes(input, HostFn::ScalarMul)?;
    if data.len() != 64 {
        return Err(Bn254ExtError::InvalidOutputLength {
            expected: 64,
            actual: data.len(),
        });
    }
    Ok(data)
}

/// Calls the `bn254_pairing_equality` host function.
///
/// * `input` length must be a multiple of 192.
/// * Empty input returns `true` (matches EIP-197).
/// * Returns the boolean result of the pairing equality check.
pub fn bn254_pairing_equality_call(input: &[u8]) -> Result<bool, Bn254ExtError> {
    if input.len() % 192 != 0 {
        return Err(Bn254ExtError::InvalidInputLength {
            expected: 192 /* or any multiple */,
            actual: input.len(),
        });
    }
    let data = call_host_bytes(input, HostFn::PairingEquality)?;
    match data.as_slice() {
        [0u8] => Ok(false),
        [1u8] => Ok(true),
        [d] => Err(Bn254ExtError::InvalidBooleanDiscriminant(*d)),
        _ => Err(Bn254ExtError::InvalidOutputLength {
            expected: 1,
            actual: data.len(),
        }),
    }
}

// ── Internals ──────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
enum HostFn {
    Add,
    ScalarMul,
    PairingEquality,
}

fn call_host_bytes(input: &[u8], which: HostFn) -> Result<Vec<u8>, Bn254ExtError> {
    #[cfg(all(feature = "runtime", target_arch = "wasm32"))]
    {
        let input_region = build_region(input);
        let input_region_ptr = &*input_region as *const Region as u32;
        let result_ptr = unsafe {
            match which {
                HostFn::Add => bn254_add(input_region_ptr),
                HostFn::ScalarMul => bn254_scalar_mul(input_region_ptr),
                HostFn::PairingEquality => bn254_pairing_equality(input_region_ptr),
            }
        };
        // Leak the input region via mem::forget — the host consumed it.
        mem::forget(input_region);
        if result_ptr == 0 {
            return Err(Bn254ExtError::HostError(
                "host returned null pointer".into(),
            ));
        }
        let data = unsafe { consume_region(result_ptr as *mut Region) };
        Ok(data)
    }
    #[cfg(not(all(feature = "runtime", target_arch = "wasm32")))]
    {
        let _ = (input, which);
        Err(Bn254ExtError::NotAvailableOffChain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_chain_returns_not_available() {
        // Under `cargo test` on a host triple the runtime feature is off
        // by default, so all three calls surface NotAvailableOffChain.
        let input = [0u8; 128];
        match bn254_add_call(&input) {
            Err(Bn254ExtError::NotAvailableOffChain) => {}
            other => panic!("expected NotAvailableOffChain, got {other:?}"),
        }
    }

    #[test]
    fn length_validation_happens_before_host_call() {
        let err = bn254_add_call(&[0u8; 127]).unwrap_err();
        assert!(matches!(
            err,
            Bn254ExtError::InvalidInputLength {
                expected: 128,
                actual: 127
            }
        ));
    }
}
