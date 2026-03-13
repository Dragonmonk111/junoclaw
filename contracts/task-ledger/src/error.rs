use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: u64 },

    #[error("Task already completed: {task_id}")]
    TaskAlreadyCompleted { task_id: u64 },

    #[error("Task not in running state: {task_id}")]
    TaskNotRunning { task_id: u64 },

    #[error("Only task submitter or admin can modify task")]
    NotSubmitter {},
}
