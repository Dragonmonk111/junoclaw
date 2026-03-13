use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Agent not found: {agent_id}")]
    AgentNotFound { agent_id: u64 },

    #[error("Agent limit reached: {max}")]
    AgentLimitReached { max: u64 },

    #[error("Insufficient registration fee: required {required}, sent {sent}")]
    InsufficientFee { required: Uint128, sent: Uint128 },

    #[error("Agent not owned by sender")]
    NotOwner {},

    #[error("Agent already deactivated")]
    AlreadyDeactivated {},
}
