use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Grant tiers — how much a builder can claim based on verified work
#[cw_serde]
pub enum GrantTier {
    /// Deployed a contract instance on testnet (500 JUNOX)
    ContractDeploy,
    /// Submitted a passing governance proposal (1000 JUNOX)
    GovernanceParticipation,
    /// TEE-attested computation verified on-chain (2000 JUNOX)
    TeeAttestation,
    /// Custom amount approved by governance (variable)
    Custom { amount: u128, description: String },
}

impl GrantTier {
    pub fn reward_amount(&self) -> u128 {
        match self {
            GrantTier::ContractDeploy => 500_000_000,           // 500 JUNOX
            GrantTier::GovernanceParticipation => 1_000_000_000, // 1000 JUNOX
            GrantTier::TeeAttestation => 2_000_000_000,         // 2000 JUNOX
            GrantTier::Custom { amount, .. } => *amount,
        }
    }
}

/// A submitted work proof awaiting TEE verification
#[cw_serde]
pub struct WorkSubmission {
    pub id: u64,
    pub builder: Addr,
    pub tier: GrantTier,
    /// On-chain evidence: TX hash, contract address, proposal ID, etc.
    pub evidence: String,
    /// SHA-256 hash of the work output (for TEE to verify)
    pub work_hash: String,
    pub status: SubmissionStatus,
    pub submitted_at_block: u64,
    /// TEE attestation hash (filled on verification)
    pub attestation_hash: Option<String>,
    /// Who verified (operator address)
    pub verified_by: Option<Addr>,
}

#[cw_serde]
pub enum SubmissionStatus {
    /// Awaiting TEE verification
    Pending,
    /// TEE verified — tokens released
    Verified,
    /// Rejected by TEE or governance
    Rejected,
    /// Tokens claimed by builder
    Claimed,
}

#[cw_serde]
pub struct Config {
    /// Admin (initially deployer, later DAO governance)
    pub admin: Addr,
    /// Addresses authorized to submit TEE attestations (operators)
    pub operators: Vec<Addr>,
    /// The agent-company contract (for governance integration)
    pub agent_company: Option<Addr>,
    /// Native denom
    pub denom: String,
    /// Whether new submissions are accepted
    pub active: bool,
    /// Total grants disbursed
    pub total_granted: u128,
    /// Total submissions
    pub total_submissions: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const SUBMISSION_SEQ: Item<u64> = Item::new("submission_seq");
pub const SUBMISSIONS: Map<u64, WorkSubmission> = Map::new("submissions");
/// Track total granted per builder address
pub const BUILDER_TOTALS: Map<&Addr, u128> = Map::new("builder_totals");
