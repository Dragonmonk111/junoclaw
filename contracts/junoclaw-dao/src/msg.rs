use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (initially deployer)
    pub admin: String,
    /// Token denom for voting weight (ujclaw)
    pub voting_denom: String,
    /// Community pool address
    pub community_pool: String,
    /// Total token supply (for quorum calculation)
    pub total_supply: u128,
    /// Voting period in blocks
    pub voting_period: u64,
    /// Quorum percentage (e.g. 10 = 10%)
    pub quorum: u32,
    /// Threshold percentage (e.g. 50 = 50% of cast votes must be Yes)
    pub threshold: u32,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Submit a new proposal
    SubmitProposal {
        title: String,
        description: String,
        proposal_type: ProposalTypeInput,
    },
    /// Vote on a proposal
    Vote {
        proposal_id: u64,
        vote: VoteChoiceInput,
    },
    /// Execute a passed proposal (spend from community pool, etc.)
    ExecuteProposal {
        proposal_id: u64,
    },
    /// Update DAO parameters (admin only)
    UpdateParams {
        voting_period: Option<u64>,
        quorum: Option<u32>,
        threshold: Option<u32>,
    },
    /// Transfer admin
    TransferAdmin {
        new_admin: String,
    },
}

/// Input version of ProposalType (same structure, separate for clarity)
#[cw_serde]
pub enum ProposalTypeInput {
    Text,
    Spend {
        recipient: String,
        amount: u128,
        denom: String,
    },
    ParameterChange {
        key: String,
        value: String,
    },
}

#[cw_serde]
pub enum VoteChoiceInput {
    Yes,
    No,
    Abstain,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get DAO config
    #[returns(ConfigResponse)]
    GetConfig {},
    /// Get a proposal by ID
    #[returns(ProposalResponse)]
    GetProposal { proposal_id: u64 },
    /// List proposals (paginated)
    #[returns(ProposalsResponse)]
    ListProposals {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Get a user's vote on a proposal
    #[returns(VoteResponse)]
    GetVote { proposal_id: u64, voter: String },
    /// Get proposal tally
    #[returns(TallyResponse)]
    GetTally { proposal_id: u64 },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub voting_denom: String,
    pub community_pool: String,
    pub total_supply: u128,
    pub voting_period: u64,
    pub quorum: u32,
    pub threshold: u32,
    pub next_proposal_id: u64,
}

#[cw_serde]
pub struct ProposalResponse {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub proposer: String,
    pub proposal_type: ProposalTypeOutput,
    pub status: String,
    pub yes_votes: u128,
    pub no_votes: u128,
    pub abstain_votes: u128,
    pub created_height: u64,
    pub voting_end_height: u64,
    pub executed: bool,
}

#[cw_serde]
pub enum ProposalTypeOutput {
    Text,
    Spend {
        recipient: String,
        amount: u128,
        denom: String,
    },
    ParameterChange {
        key: String,
        value: String,
    },
}

#[cw_serde]
pub struct ProposalsResponse {
    pub proposals: Vec<ProposalResponse>,
}

#[cw_serde]
pub struct VoteResponse {
    pub voter: String,
    pub vote: String,
    pub weight: u128,
    pub height: u64,
}

#[cw_serde]
pub struct TallyResponse {
    pub proposal_id: u64,
    pub yes_votes: u128,
    pub no_votes: u128,
    pub abstain_votes: u128,
    pub total_votes: u128,
    pub status: String,
}
