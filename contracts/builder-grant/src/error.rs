use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can perform this action")]
    Unauthorized {},

    #[error("unauthorized operator: sender is not a registered TEE operator")]
    UnauthorizedOperator {},

    #[error("submissions paused")]
    SubmissionsPaused {},

    #[error("submission not found: id {id}")]
    SubmissionNotFound { id: u64 },

    #[error("submission not pending: id {id} has status {status}")]
    SubmissionNotPending { id: u64, status: String },

    #[error("submission not verified: id {id}")]
    SubmissionNotVerified { id: u64 },

    #[error("not the builder: only the original submitter can claim")]
    NotBuilder {},

    #[error("insufficient funds: grant balance ({balance}) < reward ({reward})")]
    InsufficientFunds { balance: u128, reward: u128 },

    #[error("invalid attestation hash: must be 64 hex characters")]
    InvalidAttestationHash {},

    #[error("invalid work hash: must be 64 hex characters")]
    InvalidWorkHash {},

    #[error("empty evidence")]
    EmptyEvidence {},
}
