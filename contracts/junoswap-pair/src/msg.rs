use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use junoclaw_common::AssetInfo;

#[cw_serde]
pub struct InstantiateMsg {
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub fee_bps: u16,
    pub factory: String,
    pub junoclaw_contract: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    ProvideLiquidity {},
    WithdrawLiquidity { lp_amount: Uint128 },
    Swap { offer_asset: AssetInfo, min_return: Option<Uint128> },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(PairInfoResponse)]
    PairInfo {},
    #[returns(PoolStateResponse)]
    Pool {},
    #[returns(SimulateResponse)]
    SimulateSwap { offer_asset: AssetInfo, offer_amount: Uint128 },
    #[returns(Uint128)]
    LpBalance { address: String },
}

#[cw_serde]
pub struct PairInfoResponse {
    pub factory: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub fee_bps: u16,
    pub junoclaw_contract: Option<Addr>,
}

#[cw_serde]
pub struct PoolStateResponse {
    pub reserve_a: Uint128,
    pub reserve_b: Uint128,
    pub total_lp_shares: Uint128,
    pub total_swaps: u64,
    pub total_volume_a: Uint128,
    pub total_volume_b: Uint128,
    pub price_a_per_b: String,
    pub price_b_per_a: String,
}

#[cw_serde]
pub struct SimulateResponse {
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub fee_amount: Uint128,
}
