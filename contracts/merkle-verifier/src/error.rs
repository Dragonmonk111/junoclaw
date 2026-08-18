use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can call this function")]
    Unauthorized {},

    #[error("invalid Merkle proof: {reason}")]
    InvalidProof { reason: String },

    #[error("invalid leaf hash: expected {expected}, got {got}")]
    LeafHashMismatch { expected: String, got: String },

    #[error("batch root not found for robot {robot_id} at batch {batch_height}")]
    RootNotFound { robot_id: String, batch_height: u64 },

    #[error("invalid parameters: {reason}")]
    InvalidParams { reason: String },
}
