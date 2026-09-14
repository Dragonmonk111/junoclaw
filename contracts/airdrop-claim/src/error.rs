use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Claim window not yet open")]
    ClaimNotOpen {},

    #[error("Claim window has closed")]
    ClaimClosed {},

    #[error("Address has already claimed")]
    AlreadyClaimed {},

    #[error("Invalid merkle proof")]
    InvalidProof {},

    #[error("Insufficient contract balance for claim")]
    InsufficientBalance {},

    #[error("Claim window still open, cannot sweep")]
    SweepTooEarly {},

    #[error("No unclaimed tokens to sweep")]
    NothingToSweep {},

    #[error("Invalid params: {reason}")]
    InvalidParams { reason: String },
}
