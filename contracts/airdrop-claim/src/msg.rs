use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (DAO governance)
    pub admin: String,
    /// Token denom (ujclaw)
    pub denom: String,
    /// Community pool address for swept unclaimed tokens
    pub community_pool: String,
    /// Merkle root of the airdrop distribution (hex-encoded)
    pub merkle_root: String,
    /// Total airdrop amount in ujclaw
    pub total_amount: u128,
    /// Block height when claims open
    pub claim_start: u64,
    /// Block height when claims close (unclaimed → community pool)
    pub claim_end: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Claim airdrop tokens with a merkle proof
    Claim {
        amount: u128,
        merkle_proof: Vec<String>,
    },
    /// Sweep unclaimed tokens to community pool (after claim_end)
    SweepUnclaimed {},
    /// Update the merkle root (admin only, for corrections)
    UpdateMerkleRoot {
        merkle_root: String,
    },
    /// Transfer admin
    TransferAdmin {
        new_admin: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get airdrop config
    #[returns(ConfigResponse)]
    GetConfig {},
    /// Check if an address has claimed
    #[returns(ClaimStatusResponse)]
    HasClaimed { address: String },
    /// Get claim statistics
    #[returns(ClaimStatsResponse)]
    GetStats {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub denom: String,
    pub merkle_root: String,
    pub community_pool: String,
    pub total_amount: u128,
    pub claim_start: u64,
    pub claim_end: u64,
}

#[cw_serde]
pub struct ClaimStatusResponse {
    pub address: String,
    pub has_claimed: bool,
    pub claimed_amount: u128,
}

#[cw_serde]
pub struct ClaimStatsResponse {
    pub total_amount: u128,
    pub total_claimed: u128,
    pub total_unclaimed: u128,
    pub num_claimers: u64,
}
