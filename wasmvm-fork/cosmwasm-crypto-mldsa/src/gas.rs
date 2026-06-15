//! Gas schedule constants for ML-DSA host functions.
//!
//! Numbers are expressed in **CosmWasm VM gas** — the internal unit used by
//! `cosmwasm-vm` when instrumenting Wasm execution. The conversion to
//! Cosmos-SDK gas uses `wasmd`'s default multiplier of `100`:
//!
//! ```text
//! sdk_gas = wasm_gas / 100
//! ```
//!
//! # Status: pre-measurement estimates
//!
//! Unlike the MAYO schedule (which was derived from measured pure-Wasm gas),
//! these constants are **initial estimates pending the Phase B devnet
//! benchmark** on `junoclaw-bn254-1`. They are scaled from the measured
//! release-build verify wall-clock (`aegis-bench --features timing`):
//!
//! | Variant | Verify (wall-clock) | Est. precompile (SDK gas) | VM gas |
//! |---------|---------------------|---------------------------|--------|
//! | ML-DSA-44 | ~101 µs           | ~20 000                   | 2 000 000 |
//! | ML-DSA-65 | ~149 µs           | ~30 000                   | 3 000 000 |
//! | ML-DSA-87 | (larger)          | ~40 000                   | 4 000 000 |
//!
//! The 44:65 ratio (~1 : 1.48) matches the measured verify-time ratio. These
//! MUST be re-tuned against on-chain measurements before mainnet use.

/// ML-DSA-44 (NIST category 2) verify cost, in CosmWasm VM gas. **Estimate.**
pub const ML_DSA_44_VERIFY_COST: u64 = 2_000_000;

/// ML-DSA-65 (NIST category 3) verify cost, in CosmWasm VM gas. **Estimate.**
pub const ML_DSA_65_VERIFY_COST: u64 = 3_000_000;

/// ML-DSA-87 (NIST category 5) verify cost, in CosmWasm VM gas. **Estimate.**
pub const ML_DSA_87_VERIFY_COST: u64 = 4_000_000;

/// Cost for an unknown / unmapped variant (saturates so the call fails on gas
/// exhaustion rather than silently succeeding).
pub const ML_DSA_UNKNOWN_VARIANT_COST: u64 = u64::MAX;

/// Returns the precompile cost for the given ML-DSA variant code.
/// `variant` must be one of 44, 65, or 87.
pub const fn ml_dsa_verify_cost(variant: u32) -> u64 {
    match variant {
        44 => ML_DSA_44_VERIFY_COST,
        65 => ML_DSA_65_VERIFY_COST,
        87 => ML_DSA_87_VERIFY_COST,
        _ => ML_DSA_UNKNOWN_VARIANT_COST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_table_matches_targets() {
        assert_eq!(ml_dsa_verify_cost(44), 2_000_000); // ~20k SDK
        assert_eq!(ml_dsa_verify_cost(65), 3_000_000); // ~30k SDK
        assert_eq!(ml_dsa_verify_cost(87), 4_000_000); // ~40k SDK
    }

    #[test]
    fn unknown_variant_saturates() {
        assert_eq!(ml_dsa_verify_cost(0), u64::MAX);
        assert_eq!(ml_dsa_verify_cost(2), u64::MAX);
        assert_eq!(ml_dsa_verify_cost(99), u64::MAX);
    }
}
