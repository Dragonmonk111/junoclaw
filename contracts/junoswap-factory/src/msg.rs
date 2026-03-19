use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use junoclaw_common::AssetInfo;

#[cw_serde]
pub struct InstantiateMsg {
    pub pair_code_id: u64,
    pub default_fee_bps: u16,
    pub junoclaw_contract: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreatePair {
        token_a: AssetInfo,
        token_b: AssetInfo,
        fee_bps: Option<u16>,
    },
    UpdateConfig {
        pair_code_id: Option<u64>,
        default_fee_bps: Option<u16>,
        junoclaw_contract: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(PairResponse)]
    Pair { token_a: AssetInfo, token_b: AssetInfo },
    #[returns(PairsResponse)]
    AllPairs { start_after: Option<u64>, limit: Option<u32> },
    #[returns(u64)]
    PairCount {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: Addr,
    pub pair_code_id: u64,
    pub default_fee_bps: u16,
    pub junoclaw_contract: Option<Addr>,
    pub pair_count: u64,
}

#[cw_serde]
pub struct PairResponse {
    pub pair_addr: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
}

#[cw_serde]
pub struct PairsResponse {
    pub pairs: Vec<PairResponse>,
}

/// Message sent to instantiate a new pair contract
#[cw_serde]
pub struct PairInstantiateMsg {
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub fee_bps: u16,
    pub factory: String,
    pub junoclaw_contract: Option<String>,
}
