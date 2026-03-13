use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use junoclaw_common::ContractRegistry;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub max_agents: u64,
    pub registration_fee_ujuno: Uint128,
    pub denom: String,
    pub registry: ContractRegistry,
}

#[cw_serde]
pub struct AgentStats {
    pub total_registered: u64,
    pub total_active: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const AGENTS: Map<u64, junoclaw_common::AgentProfile> = Map::new("agents");
pub const AGENT_BY_OWNER: Map<&Addr, Vec<u64>> = Map::new("agent_by_owner");
pub const NEXT_AGENT_ID: Item<u64> = Item::new("next_agent_id");
pub const AGENT_STATS: Item<AgentStats> = Item::new("agent_stats");
