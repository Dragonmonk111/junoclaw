//! Bit-for-bit Ethereum-precompile conformance vectors.
//!
//! Where `vectors.rs` exercises algebraic identities (G+G=2G, k·G via repeated
//! addition, bilinearity), this file pins down **exact byte strings** from
//! canonical Ethereum sources. A reviewer should be able to grep any hex blob
//! below in `go-ethereum/core/vm/contracts_test.go` (or in the EIP-196 / EIP-197
//! spec text) and find an identical fixture.
//!
//! Sources cited per test:
//!
//!   * `EIP-196` — point addition + scalar multiplication on the alt_bn128 curve
//!     <https://eips.ethereum.org/EIPS/eip-196>
//!   * `EIP-197` — pairing checks on alt_bn128
//!     <https://eips.ethereum.org/EIPS/eip-197>
//!   * `EIP-1108` — gas reduction (defines the cost schedule, not new fixtures)
//!     <https://eips.ethereum.org/EIPS/eip-1108>
//!   * `go-ethereum/core/vm/contracts_test.go` — execution-layer reference impl
//!     <https://github.com/ethereum/go-ethereum/blob/master/core/vm/contracts_test.go>
//!
//! Coverage target (plan §0.2 — 24 vectors total):
//!
//!   | Precompile  | Positive | Negative |
//!   |-------------|----------|----------|
//!   | ECADD       | 5        | 3        |
//!   | ECMUL       | 5        | 3        |
//!   | ECPAIRING   | 5        | 3        |
//!
//! This file currently seeds the suite with **4 vectors** drawn directly from
//! the EIP-196 / EIP-197 spec text (the most defensible starting point — these
//! are the bytes the EIP author used to define correctness). The remaining 20
//! vectors arrive in a follow-up commit; tracking issue link in
//! `docs/POST_VOTE_EXECUTION_PLAN.md` §0.2.

use cosmwasm_crypto_bn254::{
    bn254_add, bn254_pairing_equality, bn254_scalar_mul, Bn254Error,
};

/// Decode a hex string into a byte vector. Line comments (`//` to end of line)
/// are stripped first, then any remaining non-hex character is dropped. This
/// lets test fixtures embed inline annotations and free whitespace without
/// distorting the data — but keeps the parser honest about which bytes count.
fn hx(s: &str) -> Vec<u8> {
    let stripped: String = s
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let cleaned: String = stripped.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    assert!(
        cleaned.len() % 2 == 0,
        "hex literal must contain an even number of hex digits after comment-stripping, \
         got {} from input {s:?}",
        cleaned.len()
    );
    (0..cleaned.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).expect("invalid hex"))
        .collect()
}

// ─── ECADD vectors ─────────────────────────────────────────────────────────

/// EIP-196 §"Test Cases" — ECADD example 1.
///
/// Source: <https://eips.ethereum.org/EIPS/eip-196> (search "Example 1").
///
/// Adds the identity element `(0, 0)` to itself; result is the identity.
/// Confirms the identity element is encoded as 64 zero bytes and that the
/// precompile preserves it.
#[test]
fn eip196_ecadd_identity_plus_identity() {
    let input = hx(
        "0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000",
    );
    let expected = hx(
        "0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000",
    );
    let out = bn254_add(&input).expect("identity addition must succeed");
    assert_eq!(out.as_slice(), expected.as_slice());
}

/// EIP-196 §"Test Cases" — ECADD example 2.
///
/// `G1 + (-G1) = O` where `G1 = (1, 2)` is the alt_bn128 generator and
/// `-G1 = (1, p-2)` is its negation. The result must be the identity, encoded
/// as 64 zero bytes.
///
/// `p` (BN254 base-field modulus) =
///   `0x30644E72E131A029B85045B68181585D97816A916871CA8D3C208C16D87CFD47`
/// so `p - 2 =`
///   `0x30644E72E131A029B85045B68181585D97816A916871CA8D3C208C16D87CFD45`.
#[test]
fn eip196_ecadd_g1_plus_neg_g1_is_identity() {
    let input = hx(
        // G1 = (1, 2)
        "0000000000000000000000000000000000000000000000000000000000000001\
         0000000000000000000000000000000000000000000000000000000000000002\
         // -G1 = (1, p-2)
         0000000000000000000000000000000000000000000000000000000000000001\
         30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd45",
    );
    let expected = hx(
        "0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000",
    );
    let out = bn254_add(&input).expect("G + (-G) must succeed");
    assert_eq!(out.as_slice(), expected.as_slice(), "G + (-G) must be O");
}

// ─── ECMUL vectors ─────────────────────────────────────────────────────────

/// EIP-196 §"Test Cases" — ECMUL example 1.
///
/// `0 · G1 = O`. The precompile must return the identity element regardless
/// of the input point (provided it is on-curve).
#[test]
fn eip196_ecmul_zero_scalar_is_identity() {
    let input = hx(
        // G1 = (1, 2)
        "0000000000000000000000000000000000000000000000000000000000000001\
         0000000000000000000000000000000000000000000000000000000000000002\
         // scalar = 0
         0000000000000000000000000000000000000000000000000000000000000000",
    );
    let expected = hx(
        "0000000000000000000000000000000000000000000000000000000000000000\
         0000000000000000000000000000000000000000000000000000000000000000",
    );
    let out = bn254_scalar_mul(&input).expect("0 · G must succeed");
    assert_eq!(out.as_slice(), expected.as_slice(), "0 · G must be O");
}

// ─── ECPAIRING vectors ─────────────────────────────────────────────────────

/// EIP-197 §"Specification" — empty input contract.
///
/// > "If the length of the input is not a multiple of 192, the call fails.
/// >  An empty input is allowed and must return 1 (true)."
///
/// We ratify the second clause here; the first clause is exercised in
/// `vectors.rs::ecpairing_rejects_non_multiple_of_192` (algebraic suite).
#[test]
fn eip197_ecpairing_empty_input_returns_true() {
    let result = bn254_pairing_equality(&[]).expect("empty input must succeed");
    assert!(result, "EIP-197 empty input must yield true (the empty product)");
}

// ─── Negative-case stubs (filled in subsequent commit) ────────────────────

/// Placeholder: ECADD must reject inputs whose length is not exactly 128 bytes.
///
/// The current crate enforces this; the assertion below pins down the
/// behaviour against a canonical "too-short" input. Once the full EIP-1108
/// vector batch lands, this becomes one of the three required negative
/// ECADD cases.
#[test]
fn ecadd_rejects_short_input() {
    let input = hx(
        // 96 bytes — one full G1 plus only x-coord of the second
        "0000000000000000000000000000000000000000000000000000000000000001\
         0000000000000000000000000000000000000000000000000000000000000002\
         0000000000000000000000000000000000000000000000000000000000000001",
    );
    let err = bn254_add(&input).unwrap_err();
    assert!(
        matches!(err, Bn254Error::InvalidInputLength { .. }),
        "expected InvalidInputLength, got {err:?}"
    );
}
