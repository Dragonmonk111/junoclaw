use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub denom: String,
    /// Hard cap on `max_cost` per lease — the guardrail preventing an edge
    /// agent from autonomously committing unbounded spend on burst compute.
    pub max_cost_per_lease: Uint128,
    pub min_timeout_secs: u64,
    pub max_timeout_secs: u64,
    /// Optional Moultbook address — lease lifecycle events can be logged
    /// there for audit, since this is an autonomous spend decision made
    /// without a human in the loop at request time.
    pub moultbook: Option<Addr>,
    /// Optional task-ledger address — leases may be tied to a specific
    /// task for traceability.
    pub task_ledger: Option<Addr>,
}

#[cw_serde]
pub enum LeaseStatus {
    /// Escrowed, awaiting the provider (or an authorized relayer/oracle)
    /// to confirm the Akash lease is actually active.
    Pending,
    /// Provider confirmed — compute is running.
    Active,
    /// Provider was paid `actual_cost`; remainder refunded to requester.
    Completed,
    /// Requester cancelled before the provider confirmed activation.
    Cancelled,
    /// Deadline passed without confirmation/completion — fail-safe:
    /// full refund to requester, local agent falls back to its own
    /// safe-state policy instead of waiting further.
    Expired,
}

#[cw_serde]
pub struct LeaseRequest {
    pub id: u64,
    pub requester: Addr,
    /// Akash provider identifier — not a Juno address, so stored as a
    /// free-form string (bech32 akash1... or a provider URI).
    pub provider: String,
    pub task_id: Option<u64>,
    /// 0-100 — how confident the local agent's decision was before it
    /// decided burst compute was needed. Purely informational/audit.
    pub confidence_score: u8,
    pub escrowed: Uint128,
    pub max_cost: Uint128,
    pub status: LeaseStatus,
    pub requested_at: u64,
    pub deadline: u64,
    pub actual_cost: Option<Uint128>,
    pub resolved_at: Option<u64>,
}

#[cw_serde]
#[derive(Default)]
pub struct EscrowStats {
    pub total_leases: u64,
    pub total_paid_to_providers: Uint128,
    pub total_refunded: Uint128,
    pub total_expired: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LEASE_COUNT: Item<u64> = Item::new("lease_count");
pub const LEASES: Map<u64, LeaseRequest> = Map::new("leases");
pub const LEASES_BY_REQUESTER: Map<(&Addr, u64), ()> = Map::new("leases_by_requester");
pub const STATS: Item<EscrowStats> = Item::new("stats");
