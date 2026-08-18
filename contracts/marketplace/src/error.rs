use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Listing not found: {listing_id}")]
    ListingNotFound { listing_id: u64 },

    #[error("Listing not active: {listing_id}")]
    ListingNotActive { listing_id: u64 },

    #[error("Hire not found: {hire_id}")]
    HireNotFound { hire_id: u64 },

    #[error("Hire {hire_id} is not in Escrowed state")]
    NotEscrowed { hire_id: u64 },

    #[error("Price must be greater than zero")]
    ZeroPrice {},

    #[error("Skill reference must not be empty")]
    EmptySkillRef {},

    #[error("Expected exactly one coin of {expected}{denom}, got {got:?}")]
    WrongFunds {
        expected: Uint128,
        denom: String,
        got: Vec<String>,
    },

    #[error("Task {task_id} already funded by hire {hire_id}")]
    TaskAlreadyFunded { task_id: u64, hire_id: u64 },

    #[error("Task {task_id} is not yet completed — nothing to release")]
    TaskNotCompleted { task_id: u64 },

    #[error("Truth Market epoch for batch {batch_height} is not finalized yet")]
    EpochNotFinalized { batch_height: u64 },

    #[error("Unknown Truth Market verdict: {verdict}")]
    UnknownVerdict { verdict: String },

    #[error("Cancel window has not elapsed yet ({elapsed}s < {required}s)")]
    CancelWindowNotElapsed { elapsed: u64, required: u64 },

    #[error("Skill '{skill_ref}' is not registered in skill-registry")]
    SkillNotRegistered { skill_ref: String },
}
