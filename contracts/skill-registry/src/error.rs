use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Skill not found for dApp: {dapp_name}")]
    SkillNotFound { dapp_name: String },

    #[error("dApp name already claimed by another publisher: {dapp_name}")]
    NameAlreadyClaimed { dapp_name: String },

    #[error("Insufficient registration fee: required {required}, sent {sent}")]
    InsufficientFee { required: Uint128, sent: Uint128 },

    #[error("dApp name must not be empty")]
    EmptyName {},

    #[error("skill_uri must not be empty")]
    EmptyUri {},

    #[error("skill_hash must not be empty")]
    EmptyHash {},
}
