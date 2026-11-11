use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("unauthorized: only admin can call this function")]
    Unauthorized {},

    #[error("sender {addr} is not a registered operator")]
    NotOperator { addr: String },

    #[error("operator already committed for robot {robot_id} batch {batch_height}")]
    AlreadyCommitted { robot_id: String, batch_height: u64 },

    #[error("batch ({robot_id}, {batch_height}) is already agreed")]
    AlreadyAgreed { robot_id: String, batch_height: u64 },

    #[error("commits disagree: {distinct} distinct roots among {total} commits")]
    CommitsDisagree { distinct: usize, total: usize },

    #[error("quorum not reached: {have}/{need} operator commits")]
    QuorumNotReached { have: usize, need: u32 },

    #[error("no agreed root for robot {robot_id} batch {batch_height}")]
    NotAgreed { robot_id: String, batch_height: u64 },

    #[error("invalid parameters: {reason}")]
    InvalidParams { reason: String },
}
