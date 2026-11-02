use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Probe bank already registered: {bank_hash}")]
    BankExists { bank_hash: String },

    #[error("Probe bank not found: {bank_hash}")]
    BankNotFound { bank_hash: String },

    #[error("Bank is not in proposed state: {bank_hash}")]
    BankNotProposed { bank_hash: String },

    #[error("Bank is not in active state: {bank_hash}")]
    BankNotActive { bank_hash: String },

    #[error("Bank covers too few concepts: {got}, need at least {min}")]
    TooFewConcepts { got: u32, min: u32 },

    #[error("Calibration report required before activation")]
    CalibrationRequired {},
}
