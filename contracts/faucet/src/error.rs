use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can perform this action")]
    Unauthorized {},

    #[error("already claimed: address {address} has already received tokens")]
    AlreadyClaimed { address: String },

    #[error("faucet is paused")]
    FaucetPaused {},

    #[error("insufficient funds: faucet balance ({balance}) < drip amount ({drip})")]
    InsufficientFunds { balance: u128, drip: u128 },

    #[error("invalid drip amount: must be > 0")]
    InvalidDripAmount {},
}
