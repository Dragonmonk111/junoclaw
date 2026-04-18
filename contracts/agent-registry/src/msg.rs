use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{AgentStats, Config};
#[allow(unused_imports)]
use junoclaw_common::{AgentProfile, ContractRegistry};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub max_agents: u64,
    pub registration_fee_ujuno: Uint128,
    /// Native token denom (e.g. "ujunox" on testnet, "ujuno" on mainnet). Defaults to "ujunox".
    pub denom: Option<String>,
    /// Optional cross-contract registry snapshot. Required at some point —
    /// `IncrementTasks` authorization depends on `registry.task_ledger` being
    /// wired. When `None` here, admin must later call `UpdateRegistry`.
    pub registry: Option<ContractRegistry>,
}

#[cw_serde]
pub enum ExecuteMsg {
    RegisterAgent {
        name: String,
        description: String,
        capabilities_hash: String,
        model: String,
    },
    UpdateAgent {
        agent_id: u64,
        name: Option<String>,
        description: Option<String>,
        capabilities_hash: Option<String>,
        model: Option<String>,
    },
    DeactivateAgent {
        agent_id: u64,
    },
    IncrementTasks {
        agent_id: u64,
        success: bool,
    },
    /// Called by admin/task-ledger when an agent is slashed — decrements trust_score.
    SlashAgent {
        agent_id: u64,
        reason: String,
    },
    UpdateConfig {
        admin: Option<String>,
        max_agents: Option<u64>,
        registration_fee_ujuno: Option<Uint128>,
    },
    /// Admin-only: rewire the cross-contract registry. This is the only way
    /// to authorise a task-ledger to call `IncrementTasks` after instantiate.
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
    #[returns(AgentProfile)]
    GetAgent { agent_id: u64 },
    #[returns(Vec<AgentProfile>)]
    GetAgentsByOwner { owner: String },
    #[returns(AgentStats)]
    GetStats {},
    #[returns(Vec<AgentProfile>)]
    ListAgents { start_after: Option<u64>, limit: Option<u32> },
}
