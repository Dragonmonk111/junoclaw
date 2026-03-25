use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    /// Amount to dispense per claim in base denom (e.g. 100_000_000 = 100 JUNOX)
    pub drip_amount: u128,
    /// Native denom (e.g. "ujunox")
    pub denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// First-time user claims their 100 JUNOX
    Claim {},
    /// Admin funds the faucet (send native tokens with this msg)
    Fund {},
    /// Admin withdraws remaining balance
    Withdraw { amount: Option<u128> },
    /// Admin pauses or resumes the faucet
    SetActive { active: bool },
    /// Admin updates drip amount
    UpdateDrip { drip_amount: u128 },
    /// Admin transfers admin role
    TransferAdmin { new_admin: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns faucet config and balance
    #[returns(ConfigResponse)]
    GetConfig {},
    /// Check if an address has already claimed
    #[returns(ClaimStatusResponse)]
    HasClaimed { address: String },
    /// Get total stats
    #[returns(StatsResponse)]
    GetStats {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: Addr,
    pub drip_amount: u128,
    pub denom: String,
    pub active: bool,
    pub balance: u128,
}

#[cw_serde]
pub struct ClaimStatusResponse {
    pub address: Addr,
    pub claimed: bool,
    pub claimed_at_block: Option<u64>,
}

#[cw_serde]
pub struct StatsResponse {
    pub total_claims: u64,
    pub drip_amount: u128,
    pub denom: String,
    pub balance: u128,
}
