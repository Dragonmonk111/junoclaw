use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Obligation not found: {obligation_id}")]
    ObligationNotFound { obligation_id: u64 },

    #[error("No obligation for task: {task_id}")]
    NoObligationForTask { task_id: u64 },

    #[error("Obligation not pending: {obligation_id}")]
    NotPending { obligation_id: u64 },

    #[error("Task already has an obligation")]
    AlreadyAuthorized { task_id: u64 },

    #[error("Invalid payee address")]
    InvalidPayee {},

    #[error("Amount must be greater than zero")]
    ZeroAmount {},

    #[error("No task in task-ledger carries escrow key {task_id}")]
    TaskNotFound { task_id: u64 },

    #[error("Obligation does not satisfy the escrow pin on task {task_id}")]
    PinMismatch { task_id: u64 },
}
