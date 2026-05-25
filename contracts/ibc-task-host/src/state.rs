use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;

/// Host configuration — tracks which JunoClaw contracts this host dispatches to.
#[cw_serde]
pub struct HostConfig {
    /// Admin who can update config (typically agent-company DAO)
    pub admin: Addr,
    /// task-ledger contract for AcceptTask dispatch
    pub task_ledger: Option<Addr>,
    /// escrow contract for ReclaimExpired dispatch
    pub escrow: Option<Addr>,
    /// zk-verifier contract for SubmitProof dispatch
    pub zk_verifier: Option<Addr>,
    /// Whitelist of allowed junoswap-pair contracts for Swap dispatch
    pub allowed_pairs: Vec<Addr>,
}

/// Running tally of operations dispatched (for monitoring / rate limiting)
#[cw_serde]
pub struct HostStats {
    pub total_accept_task: u64,
    pub total_submit_proof: u64,
    pub total_reclaim: u64,
    pub total_swap: u64,
}

impl Default for HostStats {
    fn default() -> Self {
        Self {
            total_accept_task: 0,
            total_submit_proof: 0,
            total_reclaim: 0,
            total_swap: 0,
        }
    }
}

pub const HOST_CONFIG: Item<HostConfig> = Item::new("host_config");
pub const HOST_STATS: Item<HostStats> = Item::new("host_stats");
