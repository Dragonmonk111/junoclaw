use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("Unknown operation in junoclaw_v1 memo")]
    UnknownOperation {},

    #[error("No funds attached to ICS-20 transfer")]
    NoFunds {},

    #[error("Swap slippage exceeded: got {got}, min {min_return}")]
    SlippageExceeded { got: String, min_return: String },

    #[error("Task ledger not configured")]
    TaskLedgerNotConfigured {},

    #[error("Escrow not configured")]
    EscrowNotConfigured {},

    #[error("Invalid pair contract: {addr}")]
    InvalidPairContract { addr: String },

    #[error("Price impact {impact_bps} bps exceeds max {max_bps} bps")]
    PriceImpactExceeded { impact_bps: u32, max_bps: u32 },
}
