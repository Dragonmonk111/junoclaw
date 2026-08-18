use cosmwasm_schema::{cw_serde, QueryResponses};

#[allow(unused_imports)]
use crate::state::{Config, FractionalOwner, Machine};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// Optional moultbook contract address for work-integrity score queries
    pub moultbook_contract: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Mint a new machine NFT. Caller becomes 100% owner (10000 basis points).
    Mint {
        model: String,
        serial_number: String,
        sensor_suite: String,
        ipfs_metadata: String,
        /// Moultbook author whose work-integrity score backs this machine
        moultbook_author: String,
    },
    /// Transfer full ownership of a machine to a new address.
    /// Caller must own 100% (10000 BP). All fractions are transferred.
    Transfer { token_id: String, to: String },
    /// Split ownership of a machine among multiple recipients.
    /// Caller must own 100%. Sum of basis_points must equal 10000.
    Fractionalize {
        token_id: String,
        recipients: Vec<(String, u32)>,
    },
    /// Transfer a partial fraction to another address.
    TransferFraction {
        token_id: String,
        to: String,
        basis_points: u32,
    },
    /// Admin only: burn a machine NFT (destroy the token).
    Burn { token_id: String },
    /// Admin only: update config (e.g. set moultbook contract)
    UpdateConfig {
        admin: Option<String>,
        moultbook_contract: Option<String>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(Machine)]
    GetMachine { token_id: String },
    /// Returns all fractional owners of a machine
    #[returns(FractionalOwnersResponse)]
    GetOwnership { token_id: String },
    /// Returns a specific owner's fraction (basis points)
    #[returns(OwnerFractionResponse)]
    GetOwnerFraction { token_id: String, owner: String },
    /// List all machines with pagination
    #[returns(MachinesResponse)]
    ListMachines { start_after: Option<String>, limit: Option<u32> },
    /// List machines an address owns fractions of
    #[returns(MachinesResponse)]
    ListByOwner { owner: String, start_after: Option<String>, limit: Option<u32> },
    /// Query moultbook for the machine's work-integrity credit score.
    /// Requires moultbook_contract to be configured.
    #[returns(WorkIntegrityScoreResponse)]
    GetWorkIntegrityScore { token_id: String },
}

#[cw_serde]
pub struct FractionalOwnersResponse {
    pub owners: Vec<FractionalOwner>,
}

#[cw_serde]
pub struct OwnerFractionResponse {
    pub owner: String,
    pub basis_points: u32,
}

#[cw_serde]
pub struct MachinesResponse {
    pub machines: Vec<Machine>,
}

#[cw_serde]
pub struct WorkIntegrityScoreResponse {
    pub token_id: String,
    pub moultbook_author: String,
    pub score: u64,
    pub total_entries: u64,
    pub active_entries: u64,
    pub verified_entries: u64,
}

/// Moultbook query message shape (mirrors moultbook-v0 QueryMsg::QueryCreditScore)
#[cw_serde]
pub struct MoultbookCreditScoreQuery {
    pub query_credit_score: MoultbookCreditScoreInner,
}

#[cw_serde]
pub struct MoultbookCreditScoreInner {
    pub author: String,
}

#[cw_serde]
pub struct MoultbookCreditScoreResponse {
    pub author: String,
    pub score: u64,
    pub total_entries: u64,
    pub active_entries: u64,
    pub redacted_entries: u64,
    pub verified_entries: u64,
}
