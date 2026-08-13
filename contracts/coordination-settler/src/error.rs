use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized: sender {sender} is not the admin")]
    Unauthorized { sender: String },

    #[error("Invalid certificate: {reason}")]
    InvalidCertificate { reason: String },

    #[error("Validator set not initialized")]
    ValidatorSetNotInitialized {},

    #[error("Validator {public_key} not in active set")]
    ValidatorNotInSet { public_key: String },

    #[error("Batch already settled at height {height}")]
    BatchAlreadySettled { height: u64 },

    #[error("Invalid messages hash: expected {expected}, got {actual}")]
    InvalidMessagesHash { expected: String, actual: String },

    #[error("No validators registered")]
    NoValidators {},

    #[error("Insufficient signatures: got {got}, need {need}")]
    InsufficientSignatures { got: u32, need: u32 },
}
