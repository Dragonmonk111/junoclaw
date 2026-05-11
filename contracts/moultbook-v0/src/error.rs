use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Entry not found: {id}")]
    EntryNotFound { id: String },

    #[error("Entry already redacted: {id}")]
    AlreadyRedacted { id: String },

    #[error("Commitment must be exactly 32 bytes (got {got})")]
    InvalidCommitmentLength { got: usize },

    #[error("Off-chain blob size {size} exceeds max {max}")]
    SizeTooLarge { size: u64, max: u64 },

    #[error("Too many refs: {count} > max {max}")]
    TooManyRefs { count: u32, max: u32 },

    #[error("content_type too long: {len} > max {max}")]
    ContentTypeTooLong { len: u32, max: u32 },

    #[error("Sender holds no DENS identity in whoami contract")]
    NoIdentity {},

    #[error("Visibility cannot be widened to Public from a narrower scope")]
    CannotWidenVisibility {},

    #[error("Duplicate entry: {id}")]
    DuplicateEntry { id: String },

    #[error("Invalid ref id: {id}")]
    InvalidRef { id: String },
}
