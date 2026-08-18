use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{Config, EscrowStats, LeaseRequest};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub denom: Option<String>,
    pub max_cost_per_lease: Uint128,
    pub min_timeout_secs: Option<u64>,
    pub max_timeout_secs: Option<u64>,
    pub moultbook: Option<String>,
    pub task_ledger: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Escrow funds and request a burst-compute lease from an Akash
    /// provider. Funds sent must equal `max_cost` in the configured denom
    /// and `max_cost` must not exceed `config.max_cost_per_lease`.
    RequestLease {
        provider: String,
        task_id: Option<u64>,
        confidence_score: u8,
        max_cost: Uint128,
        timeout_secs: u64,
    },

    /// Called by the requester (or an authorized relayer/oracle) once the
    /// Akash provider has confirmed the lease is actually running.
    ConfirmLeaseActive { lease_id: u64 },

    /// Settle a lease: pay the provider's Juno-side payout address
    /// `actual_cost` (<= escrowed amount) and refund the remainder to the
    /// requester.
    CompleteLease {
        lease_id: u64,
        actual_cost: Uint128,
        payout_addr: String,
    },

    /// Requester cancels before the lease is confirmed active — full
    /// refund.
    CancelLease { lease_id: u64 },

    /// Permissionless: once the deadline has passed on a lease still stuck
    /// in Pending or Active, anyone can force expiry — full refund to the
    /// requester. This is the on-chain half of the reflex-tier fail-safe:
    /// the local agent does not block on this transaction to fall back to
    /// its own safe-state, it simply stops waiting once its own timeout
    /// fires; this call reconciles the escrow afterwards.
    ExpireLease { lease_id: u64 },

    UpdateConfig {
        max_cost_per_lease: Option<Uint128>,
        min_timeout_secs: Option<u64>,
        max_timeout_secs: Option<u64>,
        moultbook: Option<String>,
        task_ledger: Option<String>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},

    #[returns(LeaseRequest)]
    GetLease { lease_id: u64 },

    #[returns(Vec<LeaseRequest>)]
    ListLeasesByRequester {
        requester: String,
        limit: Option<u32>,
    },

    #[returns(EscrowStats)]
    GetStats {},
}
