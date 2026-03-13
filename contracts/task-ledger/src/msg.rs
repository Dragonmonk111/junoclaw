use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{Config, LedgerStats};
#[allow(unused_imports)]
use junoclaw_common::{ExecutionTier, TaskRecord};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub agent_registry: String,
    /// Optional initial operator wallets (e.g. daemon address).
    pub operators: Option<Vec<String>>,
}

#[cw_serde]
pub enum ExecuteMsg {
    SubmitTask {
        agent_id: u64,
        input_hash: String,
        execution_tier: ExecutionTier,
    },
    CompleteTask {
        task_id: u64,
        output_hash: String,
        cost_ujuno: Option<Uint128>,
    },
    FailTask {
        task_id: u64,
    },
    CancelTask {
        task_id: u64,
    },
    /// Admin-only: grant operator rights to a wallet.
    AddOperator { operator: String },
    /// Admin-only: revoke operator rights.
    RemoveOperator { operator: String },
    UpdateConfig {
        admin: Option<String>,
        agent_registry: Option<String>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(TaskRecord)]
    GetTask { task_id: u64 },
    #[returns(Vec<TaskRecord>)]
    GetTasksByAgent { agent_id: u64, limit: Option<u32> },
    #[returns(Vec<TaskRecord>)]
    GetTasksBySubmitter { submitter: String, limit: Option<u32> },
    #[returns(LedgerStats)]
    GetStats {},
    #[returns(Vec<TaskRecord>)]
    ListTasks { start_after: Option<u64>, limit: Option<u32> },
}
