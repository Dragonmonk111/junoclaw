use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use junoclaw_common::AssetInfo;

#[cw_serde]
pub struct PairConfig {
    pub factory: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub fee_bps: u16,
    pub junoclaw_contract: Option<Addr>,
}

#[cw_serde]
pub struct PoolState {
    pub reserve_a: Uint128,
    pub reserve_b: Uint128,
    pub total_lp_shares: Uint128,
    pub total_swaps: u64,
    pub total_volume_a: Uint128,
    pub total_volume_b: Uint128,
    pub last_swap_block: u64,
}

impl Default for PoolState {
    fn default() -> Self {
        Self {
            reserve_a: Uint128::zero(),
            reserve_b: Uint128::zero(),
            total_lp_shares: Uint128::zero(),
            total_swaps: 0,
            total_volume_a: Uint128::zero(),
            total_volume_b: Uint128::zero(),
            last_swap_block: 0,
        }
    }
}

pub const PAIR_CONFIG: Item<PairConfig> = Item::new("pair_config");
pub const POOL_STATE: Item<PoolState> = Item::new("pool_state");
pub const LP_SHARES: cw_storage_plus::Map<&Addr, Uint128> = cw_storage_plus::Map::new("lp_shares");
