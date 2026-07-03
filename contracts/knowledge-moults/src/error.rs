use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Knowledge Moult not found: {id}")]
    NotFound { id: String },

    #[error("Duplicate Knowledge Moult id: {id}")]
    DuplicateEntry { id: String },

    #[error("agent alias must not be empty")]
    EmptyAgent {},

    #[error("knowledge_summary too long: {len} > max {max}")]
    SummaryTooLong { len: u32, max: u32 },

    #[error("Too many source_moults: {count} > max {max}")]
    TooManySourceMoults { count: u32, max: u32 },
}
