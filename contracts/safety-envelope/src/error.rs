use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can call this function")]
    Unauthorized {},
    #[error("safety envelope not found for robot {robot_id}")]
    EnvelopeNotFound { robot_id: String },
    #[error("invalid parameters: {reason}")]
    InvalidParams { reason: String },
    #[error("envelope version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },
}
