use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Proposal not found")]
    ProposalNotFound {},

    #[error("Proposal is not active")]
    ProposalNotActive {},

    #[error("Voting period has ended")]
    VotingEnded {},

    #[error("Already voted")]
    AlreadyVoted {},

    #[error("Proposal has already been executed")]
    AlreadyExecuted {},

    #[error("Proposal has not passed")]
    ProposalNotPassed {},

    #[error("Quorum not reached")]
    QuorumNotReached {},

    #[error("Insufficient treasury balance")]
    InsufficientBalance {},

    #[error("No locked voting tokens — lock voting_denom first")]
    NoLockedTokens {},

    #[error("Insufficient locked balance")]
    InsufficientLocked {},

    #[error("Tokens locked until block {until} (voted on an active proposal)")]
    LockedUntil { until: u64 },

    #[error("Invalid funds: must send exactly one coin of the voting denom")]
    InvalidFunds {},

    #[error("Invalid params: {reason}")]
    InvalidParams { reason: String },
}
