use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

use crate::state::{GrantTier, SubmissionStatus, WorkSubmission};

#[cw_serde]
pub struct InstantiateMsg {
    /// Native denom (e.g. "ujunox")
    pub denom: String,
    /// Initial TEE operator addresses
    pub operators: Vec<String>,
    /// Optional agent-company contract for governance integration
    pub agent_company: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Builder submits proof of ecosystem work
    SubmitWork {
        tier: GrantTier,
        /// On-chain evidence (TX hash, contract addr, etc.)
        evidence: String,
        /// SHA-256 hash of work output
        work_hash: String,
    },

    /// TEE operator verifies work and attests
    VerifyWork {
        submission_id: u64,
        /// TEE attestation hash proving verification ran in enclave
        attestation_hash: String,
        /// true = verified, false = rejected
        approved: bool,
    },

    /// Builder claims tokens after verification
    ClaimGrant {
        submission_id: u64,
    },

    /// Admin funds the grant pool (send native tokens with this msg)
    Fund {},

    /// Admin adds a TEE operator
    AddOperator { address: String },

    /// Admin removes a TEE operator
    RemoveOperator { address: String },

    /// Admin pauses/resumes submissions
    SetActive { active: bool },

    /// Admin transfers admin role
    TransferAdmin { new_admin: String },

    /// Admin withdraws funds
    Withdraw { amount: Option<u128> },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    GetConfig {},

    #[returns(WorkSubmission)]
    GetSubmission { id: u64 },

    #[returns(SubmissionsResponse)]
    ListSubmissions {
        status: Option<SubmissionStatus>,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    #[returns(BuilderStatsResponse)]
    GetBuilderStats { address: String },

    #[returns(StatsResponse)]
    GetStats {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: Addr,
    pub operators: Vec<Addr>,
    pub agent_company: Option<Addr>,
    pub denom: String,
    pub active: bool,
    pub balance: u128,
}

#[cw_serde]
pub struct SubmissionsResponse {
    pub submissions: Vec<WorkSubmission>,
}

#[cw_serde]
pub struct BuilderStatsResponse {
    pub address: Addr,
    pub total_granted: u128,
    pub submissions: Vec<WorkSubmission>,
}

#[cw_serde]
pub struct StatsResponse {
    pub total_granted: u128,
    pub total_submissions: u64,
    pub balance: u128,
    pub pending_count: u64,
}
