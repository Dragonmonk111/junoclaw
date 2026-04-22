use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("no verification key stored — call StoreVk first")]
    NoVerificationKey {},

    #[error("proof verification failed")]
    ProofInvalid {},

    #[error("deserialization error: {reason}")]
    DeserializationError { reason: String },

    /// Surfaced when the `bn254-precompile` feature is on and the host
    /// rejected a call (malformed input, not-on-curve point, etc.). The
    /// inner string is the host's error message, preserved verbatim so
    /// the caller can distinguish programme errors from proof rejection.
    #[error("bn254 precompile error: {reason}")]
    PrecompileError { reason: String },
}
