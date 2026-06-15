//! Gas schedule constants for MAYO host functions.
//!
//! The numbers here are expressed in **CosmWasm VM gas** — the internal
//! unit used by `cosmwasm-vm` when instrumenting Wasm execution. The
//! conversion to Cosmos-SDK gas uses `wasmd`'s default multiplier of `100`:
//!
//! ```text
//! sdk_gas = wasm_gas / 100
//! ```
//!
//! Costs are derived from the measured pure-Wasm gas on Juno uni-7 testnet
//! divided by a ~7× speedup target (native vs Wasm interpreter overhead).
//!
//! | Variant | Pure-Wasm verify (SDK gas) | Precompile target (SDK gas) | VM gas |
//! |---------|---------------------------|----------------------------|--------|
//! | MAYO-2  | ~356 000                  | ~50 000                    | 5 000 000 |
//! | MAYO-3  | ~457 000                  | ~70 000                    | 7 000 000 |
//! | MAYO-5  | ~799 000                  | ~120 000                   | 12 000 000 |

/// MAYO-2 (NIST Level 1) verify cost, in CosmWasm VM gas.
pub const MAYO2_VERIFY_COST: u64 = 5_000_000;

/// MAYO-3 (NIST Level 3) verify cost, in CosmWasm VM gas.
pub const MAYO3_VERIFY_COST: u64 = 7_000_000;

/// MAYO-5 (NIST Level 5) verify cost, in CosmWasm VM gas.
pub const MAYO5_VERIFY_COST: u64 = 12_000_000;

/// Cost for an unknown / unmapped variant (saturates to a large value so
/// the call fails on gas exhaustion rather than silently succeeding).
pub const MAYO_UNKNOWN_VARIANT_COST: u64 = u64::MAX;

/// Returns the precompile cost for the given MAYO variant code.
/// `variant` must be one of 1, 2, 3, or 5.
pub const fn mayo_verify_cost(variant: u32) -> u64 {
    match variant {
        1 => MAYO2_VERIFY_COST, // MAYO-1 uses same params as MAYO-2 for cost
        2 => MAYO2_VERIFY_COST,
        3 => MAYO3_VERIFY_COST,
        5 => MAYO5_VERIFY_COST,
        _ => MAYO_UNKNOWN_VARIANT_COST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_table_matches_targets() {
        assert_eq!(mayo_verify_cost(2), 5_000_000); // 50k SDK
        assert_eq!(mayo_verify_cost(3), 7_000_000); // 70k SDK
        assert_eq!(mayo_verify_cost(5), 12_000_000); // 120k SDK
    }

    #[test]
    fn unknown_variant_saturates() {
        assert_eq!(mayo_verify_cost(0), u64::MAX);
        assert_eq!(mayo_verify_cost(4), u64::MAX);
        assert_eq!(mayo_verify_cost(99), u64::MAX);
    }
}
