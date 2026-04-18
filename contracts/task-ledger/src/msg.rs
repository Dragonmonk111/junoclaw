use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{Config, LedgerStats};
#[allow(unused_imports)]
use junoclaw_common::{ContractRegistry, ExecutionTier, TaskRecord};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub agent_registry: String,
    /// Optional initial operator wallets (e.g. daemon address).
    pub operators: Option<Vec<String>>,
    #[serde(default)]
    pub agent_company: Option<String>,
    /// Optional cross-contract registry. When `None`, registry is initialised
    /// from `agent_registry` alone (task_ledger/escrow remain unset until
    /// `UpdateRegistry` is called by the admin). Supplying a registry here
    /// wires all three pointers at instantiate time — preferred for new
    /// deployments where the address graph is known up-front.
    pub registry: Option<ContractRegistry>,
}

#[cw_serde]
pub enum ExecuteMsg {
    SubmitTask {
        agent_id: u64,
        input_hash: String,
        execution_tier: ExecutionTier,
        /// Optional governance correlation id. Populated by
        /// `agent-company::WavsPush` so that downstream escrow callbacks key
        /// the obligation under the same id the proposal layer used.
        #[serde(default)]
        proposal_id: Option<u64>,
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
        #[serde(default)]
        agent_company: Option<String>,
    },
    /// Admin-only: rewire the cross-contract registry (agent-registry,
    /// task-ledger self-ref, escrow). Any field left as `None` is untouched;
    /// passing an empty-Addr would be rejected by `addr_validate`.
    UpdateRegistry {
        agent_registry: Option<String>,
        task_ledger: Option<String>,
        escrow: Option<String>,
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
    /// Reverse lookup: fetch the task record spawned by a given governance
    /// proposal id. Returns `None` if no task is correlated to that proposal.
    #[returns(Option<TaskRecord>)]
    GetTaskByProposal { proposal_id: u64 },
    #[returns(Vec<TaskRecord>)]
    GetTasksByAgent { agent_id: u64, limit: Option<u32> },
    #[returns(Vec<TaskRecord>)]
    GetTasksBySubmitter { submitter: String, limit: Option<u32> },
    #[returns(LedgerStats)]
    GetStats {},
    #[returns(Vec<TaskRecord>)]
    ListTasks { start_after: Option<u64>, limit: Option<u32> },
}
