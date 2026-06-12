use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Duplicate member: {addr}")]
    DuplicateMember { addr: String },

    #[error("Member not found: {addr}")]
    MemberNotFound { addr: String },

    #[error("Parent not found: {addr}")]
    ParentNotFound { addr: String },

    #[error("Invalid weights: total must equal {expected}, got {got}")]
    InvalidWeights { expected: u64, got: u64 },

    #[error("Cannot break channel on root member")]
    BreakChannelRoot {},

    #[error("Sunset blocked: member {addr} has not passed their bud (has {children} children)")]
    SunsetBlocked { addr: String, children: u32 },

    #[error("Sunset already initiated at height {height}")]
    SunsetAlreadyInitiated { height: u64 },

    #[error("Sunset not initiated")]
    SunsetNotInitiated {},

    #[error("Sunset grace period active: {remaining} blocks remaining")]
    SunsetGracePeriod { remaining: u64 },

    #[error("Contract is sunsetting — no new buds allowed")]
    SunsettingNoBuds {},

    #[error("Contract has been sunset")]
    AlreadySunset {},

    #[error("MAYO public key not registered for member: {addr}")]
    MayoPkNotFound { addr: String },

    #[error("MAYO public key hash mismatch for member: {addr}")]
    MayoPkHashMismatch { addr: String },

    #[error("MAYO signature verification failed for member: {addr}")]
    MayoVerifyFailed { addr: String },

    #[error("Invalid MAYO public key length: expected {expected}, got {actual}")]
    MayoInvalidPkLength { expected: usize, actual: usize },
}
