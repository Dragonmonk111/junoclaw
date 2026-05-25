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

    #[error("Visibility group too large: {count} > max {max}")]
    GroupTooLarge { count: u32, max: u32 },

    #[error("ZK anonymous publishing requires zk_verifier to be configured")]
    ZkVerifierNotConfigured {},

    #[error("ZK anonymous publishing requires membership_vk_hash to be configured")]
    MembershipVkNotConfigured {},

    #[error("Epoch rate limit exceeded: {count} entries this epoch (max {max})")]
    EpochRateLimited { count: u32, max: u32 },

    #[error("ZK membership proof verification failed")]
    MembershipProofInvalid {},

    #[error("ZK verification sub-message failed: {reason}")]
    ZkVerificationFailed { reason: String },

    #[error("Entry {id} was not authored by sender")]
    NotEntryAuthor { id: String },

    #[error("Entry {id} already has a disclosure")]
    AlreadyDisclosed { id: String },

    #[error("Derivation proof verification failed")]
    DerivationProofInvalid {},
}
