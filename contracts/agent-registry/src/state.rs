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
/// agent_id → exact model/weights (and optional pinned probe bank) the
/// agent's decisions are probed under. This is the adjacency link to the
/// probe-registry contract: probe-registry governs which banks are
/// ACTIVE; this binding pins WHICH model the agent claims to run. The
/// gate rejects thought attestations whose model/weights don't match.
pub const MODEL_BINDINGS: Map<u64, ModelBinding> = Map::new("model_bindings");

#[cw_serde]
pub struct ModelBinding {
    pub model_id: String,
    /// sha256 of the model weights the agent runs (hex).
    pub weights_hash: String,
    /// Optional pinned probe bank. None = any bank active for these
    /// weights is acceptable (probe-registry decides activation).
    pub bank_hash: Option<String>,
    pub bound_at: u64,
    /// Attestation history feeding credibility alongside trust_score:
    /// accepted probed decisions vs rejected/blocked ones.
    pub thoughts_verified: u64,
    pub thoughts_rejected: u64,
}
