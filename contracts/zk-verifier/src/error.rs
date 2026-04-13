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
}
