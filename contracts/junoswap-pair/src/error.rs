use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Insufficient liquidity")]
    InsufficientLiquidity {},

    #[error("Insufficient funds sent")]
    InsufficientFunds {},

    #[error("Slippage exceeded: return {return_amount} < min {min_return}")]
    SlippageExceeded {
        return_amount: String,
        min_return: String,
    },

    #[error("Invalid asset: expected {expected}, got {got}")]
    InvalidAsset { expected: String, got: String },

    #[error("Pool is empty — provide initial liquidity first")]
    EmptyPool {},

    #[error("Zero amount not allowed")]
    ZeroAmount {},
}
