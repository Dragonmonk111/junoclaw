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

    #[error("Sender must own active agent {agent_id} to submit a public task")]
    AgentNotOwned { agent_id: u64 },

    #[error("agent_id 0 is reserved for authorized system tasks")]
    ReservedAgentId {},

    #[error("Proposal-linked task already exists for proposal {proposal_id}")]
    ProposalTaskAlreadyExists { proposal_id: u64 },
}
