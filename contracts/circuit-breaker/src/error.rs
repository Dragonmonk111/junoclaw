use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can call this function")]
    Unauthorized {},

    #[error("circuit breaker not found for robot {robot_id}")]
    BreakerNotFound { robot_id: String },

    #[error("circuit breaker already tripped for robot {robot_id}")]
    AlreadyTripped { robot_id: String },

    #[error("circuit breaker is not tripped for robot {robot_id}, cannot reset")]
    NotTripped { robot_id: String },

    #[error("invalid parameters: {reason}")]
    InvalidParams { reason: String },
}
