use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Lease not found: {lease_id}")]
    LeaseNotFound { lease_id: u64 },

    #[error("Lease {lease_id} is not in Pending state")]
    NotPending { lease_id: u64 },

    #[error("Lease {lease_id} is not in Active state")]
    NotActive { lease_id: u64 },

    #[error("Lease {lease_id} has already been resolved")]
    AlreadyResolved { lease_id: u64 },

    #[error("Provider must not be empty")]
    EmptyProvider {},

    #[error("Requested cost cap {requested} exceeds the configured max {max}")]
    CostCapExceeded { requested: Uint128, max: Uint128 },

    #[error("Timeout must be between {min}s and {max}s")]
    InvalidTimeout { min: u64, max: u64 },

    #[error("Expected exactly one coin of {denom}, got {got:?}")]
    WrongFunds { denom: String, got: Vec<String> },

    #[error("Sent amount {sent} is below the requested max_cost {max_cost}")]
    InsufficientFunds { sent: Uint128, max_cost: Uint128 },

    #[error("Actual cost {actual} exceeds escrowed amount {escrowed}")]
    ActualCostExceedsEscrow { actual: Uint128, escrowed: Uint128 },

    #[error("Lease {lease_id} has not yet reached its deadline ({now} < {deadline})")]
    DeadlineNotReached { lease_id: u64, now: u64, deadline: u64 },

    #[error("Confidence score must be between 0 and 100")]
    InvalidConfidenceScore {},
}
