use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// DAO admin (initially deployer, can be transferred)
pub const ADMIN: Item<Addr> = Item::new("admin");

/// Token denom for voting weight (ujclaw)
pub const VOTING_DENOM: Item<String> = Item::new("voting_denom");

/// Community pool address (holds the 30% treasury)
pub const COMMUNITY_POOL: Item<Addr> = Item::new("community_pool");

/// Voting period in blocks
pub const VOTING_PERIOD: Item<u64> = Item::new("voting_period");

/// Quorum as percentage (e.g. 10 = 10% of total supply must vote)
pub const QUORUM: Item<u32> = Item::new("quorum");

/// Threshold as percentage (e.g. 50 = 50% of votes must be Yes)
pub const THRESHOLD: Item<u32> = Item::new("threshold");

/// Total token supply (for quorum calculation)
pub const TOTAL_SUPPLY: Item<u128> = Item::new("total_supply");

/// Next proposal ID
pub const NEXT_PROPOSAL_ID: Item<u64> = Item::new("next_proposal_id");

/// Proposals: proposal_id -> Proposal
pub const PROPOSALS: Map<u64, Proposal> = Map::new("proposals");

/// Votes: (proposal_id, voter) -> Vote
pub const VOTES: Map<(u64, &str), VoteRecord> = Map::new("votes");

#[cw_serde]
pub struct Proposal {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub proposer: String,
    pub proposal_type: ProposalType,
    pub status: ProposalStatus,
    pub yes_votes: u128,
    pub no_votes: u128,
    pub abstain_votes: u128,
    pub created_height: u64,
    pub voting_end_height: u64,
    pub executed: bool,
}

#[cw_serde]
pub enum ProposalType {
    /// Text proposal (signaling, no execution)
    Text,
    /// Spend from community pool
    Spend {
        recipient: String,
        amount: u128,
        denom: String,
    },
    /// Parameter change
    ParameterChange {
        key: String,
        value: String,
    },
}

#[cw_serde]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Failed,
}

#[cw_serde]
pub struct VoteRecord {
    pub voter: String,
    pub vote: VoteChoice,
    pub weight: u128,
    pub height: u64,
}

#[cw_serde]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}
