use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Operator not found: {address}")]
    OperatorNotFound { address: String },

    #[error("Operator already registered: {address}")]
    AlreadyRegistered { address: String },

    #[error("Insufficient stake: required {required}, sent {sent}")]
    InsufficientStake { required: Uint128, sent: Uint128 },

    #[error("Operator not active")]
    NotActive {},

    #[error("Operator already active")]
    AlreadyActive {},

    #[error("Operator already deactivated")]
    AlreadyDeactivated {},

    #[error("Verdict already submitted for batch {batch_height} by {operator}")]
    DuplicateVerdict { batch_height: u64, operator: String },

    #[error("Epoch already finalized for batch {batch_height}")]
    EpochAlreadyFinalized { batch_height: u64 },

    #[error("Epoch not found for batch {batch_height}")]
    EpochNotFound { batch_height: u64 },

    #[error("No verdicts submitted for batch {batch_height}")]
    NoVerdicts { batch_height: u64 },

    #[error("Invalid verdict: {verdict}")]
    InvalidVerdict { verdict: String },

    #[error("Unstake cooldown not elapsed")]
    CooldownNotElapsed {},

    #[error("No pending unstake request")]
    NoPendingUnstake {},
}
