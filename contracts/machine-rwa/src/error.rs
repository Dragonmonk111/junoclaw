use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Machine not found: {token_id}")]
    MachineNotFound { token_id: String },

    #[error("Machine already burned: {token_id}")]
    AlreadyBurned { token_id: String },

    #[error("Caller does not own 100% of machine {token_id} (owns {owned} BP)")]
    NotFullOwner { token_id: String, owned: u32 },

    #[error("Caller owns 0 BP of machine {token_id}")]
    NoFraction { token_id: String },

    #[error("Fractionalize requires sum of basis_points = 10000, got {sum}")]
    InvalidBasisPointsSum { sum: u32 },

    #[error("Basis points cannot exceed 10000 (got {bp})")]
    BasisPointsTooHigh { bp: u32 },

    #[error("Cannot transfer {requested} BP, caller only owns {owned} BP")]
    InsufficientFraction { requested: u32, owned: u32 },

    #[error("Machine model must not be empty")]
    EmptyModel {},

    #[error("Serial number must not be empty")]
    EmptySerial {},

    #[error("Moultbook contract not configured")]
    MoultbookNotConfigured {},

    #[error("Recipient list must not be empty")]
    EmptyRecipients {},
}
