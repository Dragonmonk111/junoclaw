use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (governance or ops)
    pub admin: Option<String>,
    /// Minimum distinct operators that must commit the identical root
    pub required: u32,
    /// Initial operator set (bech32 addresses)
    pub operators: Vec<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Register a new operator (admin only).
    RegisterOperator { addr: String },

    /// Remove an operator (admin only). Does not erase past commits.
    RemoveOperator { addr: String },

    /// An operator commits the deterministic root it computed for a batch.
    /// One commit per operator per (robot_id, batch_height); identical
    /// roots across `required` distinct operators finalize the batch.
    Commit {
        robot_id: String,
        batch_height: u64,
        merkle_root: String,
        cycle_count: u32,
    },

    /// Finalize a batch once quorum of identical commits exists.
    /// Callable by anyone — pure agreement check, no signature needed.
    Finalize {
        robot_id: String,
        batch_height: u64,
    },

    /// Transfer admin.
    TransferAdmin { new_admin: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    GetConfig {},

    #[returns(CommitResponse)]
    GetCommit {
        robot_id: String,
        batch_height: u64,
        operator: String,
    },

    #[returns(CommitsResponse)]
    ListCommits { robot_id: String, batch_height: u64 },

    #[returns(AgreedResponse)]
    GetAgreed { robot_id: String, batch_height: u64 },

    #[returns(OperatorsResponse)]
    ListOperators {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub required: u32,
    pub operator_count: u32,
}

#[cw_serde]
pub struct CommitResponse {
    pub robot_id: String,
    pub batch_height: u64,
    pub operator: String,
    pub merkle_root: String,
    pub cycle_count: u32,
    pub committed_at: u64,
}

#[cw_serde]
pub struct CommitsResponse {
    pub commits: Vec<CommitResponse>,
}

#[cw_serde]
pub struct AgreedResponse {
    pub robot_id: String,
    pub batch_height: u64,
    pub merkle_root: String,
    pub cycle_count: u32,
    pub commits: Vec<String>,
    pub agreed_at: u64,
}

#[cw_serde]
pub struct OperatorsResponse {
    pub operators: Vec<String>,
}
