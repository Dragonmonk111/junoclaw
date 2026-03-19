use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Pair already exists")]
    PairExists {},

    #[error("Invalid fee: must be <= 10000 bps")]
    InvalidFee {},

    #[error("Cannot create pair with identical assets")]
    IdenticalAssets {},
}
