use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Member weights must sum to exactly 10,000 basis points (got {got})")]
    InvalidWeights { got: u64 },

    #[error("Member list cannot be empty")]
    EmptyMembers {},

    #[error("Duplicate member address: {addr}")]
    DuplicateMember { addr: String },

    #[error("No pending proposal")]
    NoProposal {},

    #[error("Proposal already executed")]
    AlreadyExecuted {},

    #[error("Proposal timelock not elapsed (executable after block {executable_after}, current {current})")]
    TimelockNotElapsed { executable_after: u64, current: u64 },

    #[error("No funds sent for distribution")]
    NoFunds {},

    #[error("Task {task_id} already has a payment record")]
    AlreadyDistributed { task_id: u64 },

    // ── General governance errors ──

    #[error("Proposal {id} not found")]
    ProposalNotFound { id: u64 },

    #[error("Proposal {id} is not open for voting")]
    ProposalNotOpen { id: u64 },

    #[error("Address {addr} already voted on proposal {id}")]
    AlreadyVoted { id: u64, addr: String },

    #[error("Address {addr} is not a DAO member")]
    NotMember { addr: String },

    #[error("Voting deadline not yet reached (deadline block {deadline}, current {current})")]
    VotingNotEnded { deadline: u64, current: u64 },

    #[error("Proposal {id} did not reach quorum")]
    QuorumNotMet { id: u64 },

    #[error("Proposal {id} did not pass (yes_weight <= no_weight)")]
    ProposalNotPassed { id: u64 },

    #[error("Legacy weight-change messages are disabled; use CreateProposal{{kind: WeightChange}}, CastVote, and ExecuteProposal")]
    LegacyWeightChangeDisabled {},

    #[error("No task-ledger address configured for WavsPush proposals")]
    NoTaskLedger {},

    // ── Sortition / randomness errors ──

    #[error("Sortition count {count} exceeds eligible pool size {pool_size}")]
    SortitionCountExceedsPool { count: u32, pool_size: u32 },

    #[error("No pending sortition for job {job_id}")]
    NoPendingSortition { job_id: String },

    #[error("Invalid randomness: expected 64 hex characters (32 bytes), got {len}")]
    InvalidRandomness { len: usize },

    #[error("Unauthorized randomness submission")]
    UnauthorizedRandomness {},

    // ── v7 zk-sidecar ──────────────────────────────────────────────

    #[error("zk-sidecar: proof supplied but no zk_verifier is configured. Either omit proof fields or configure a zk_verifier via RotateZkVerifier / ConfigChange.")]
    ZkVerifierNotConfigured {},

    #[error("zk-sidecar: incomplete proof bundle — proof_base64 and public_inputs_base64 must both be Some or both be None, got proof_some={proof_some} inputs_some={inputs_some}")]
    IncompleteZkProofBundle { proof_some: bool, inputs_some: bool },
}
