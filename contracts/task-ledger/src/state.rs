use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use junoclaw_common::ContractRegistry;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub agent_registry: Addr,
    /// Wallets allowed to call CompleteTask/FailTask (e.g. the daemon wallet).
    pub operators: Vec<Addr>,
    #[serde(default)]
    pub agent_company: Option<Addr>,
    pub registry: ContractRegistry,
}

#[cw_serde]
pub struct LedgerStats {
    pub total_tasks: u64,
    pub total_completed: u64,
    pub total_failed: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const TASKS: Map<u64, junoclaw_common::TaskRecord> = Map::new("tasks");
pub const TASKS_BY_AGENT: Map<u64, Vec<u64>> = Map::new("tasks_by_agent");
pub const TASKS_BY_SUBMITTER: Map<&Addr, Vec<u64>> = Map::new("tasks_by_submitter");
/// Reverse index: governance proposal_id → task_id. Populated only for tasks
/// spawned by `agent-company::WavsPush`. Enables downstream contracts to
/// address the task by the correlation id the governance layer used.
pub const TASKS_BY_PROPOSAL: Map<u64, u64> = Map::new("tasks_by_proposal");
pub const NEXT_TASK_ID: Item<u64> = Item::new("next_task_id");
pub const LEDGER_STATS: Item<LedgerStats> = Item::new("ledger_stats");
