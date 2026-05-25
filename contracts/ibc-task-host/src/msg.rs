use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

/// Instantiation — wire up the JunoClaw contracts this host dispatches to.
#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub task_ledger: Option<String>,
    pub escrow: Option<String>,
    pub zk_verifier: Option<String>,
    /// Whitelisted junoswap-pair contracts for cross-chain swap
    pub allowed_pairs: Vec<String>,
}

/// Execute messages — these arrive via ICS-20+PFM wasm memo.
///
/// The PFM wasm middleware deserializes `memo.wasm.msg` and calls
/// `execute()` with the corresponding variant.
#[cw_serde]
pub enum ExecuteMsg {
    /// Dispatch from ICS-20 memo — the `junoclaw_v1` envelope.
    JunoClawV1(JunoClawV1Op),
    /// Admin: update host config
    UpdateConfig {
        task_ledger: Option<String>,
        escrow: Option<String>,
        zk_verifier: Option<String>,
        allowed_pairs: Option<Vec<String>>,
    },
}

/// The four v2.1 operations that arrive via ICS-20 memo.
#[cw_serde]
pub enum JunoClawV1Op {
    AcceptTask {
        task_id: u64,
        agent_addr: String,
        agent_origin_chain: String,
        agent_origin_addr: String,
    },
    SubmitProof {
        task_id: u64,
        proof_b64: String,
        public_inputs_b64: String,
        agent_origin_chain: String,
        agent_origin_addr: String,
    },
    ReclaimExpired {
        task_id: u64,
        dao_origin_chain: String,
        dao_origin_addr: String,
    },
    Swap {
        pair_contract: String,
        offer_denom: String,
        min_return: String,
        agent_origin_chain: String,
        agent_origin_addr: String,
        max_price_impact_bps: Option<u32>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(HostConfigResponse)]
    Config {},
    #[returns(HostStatsResponse)]
    Stats {},
}

#[cw_serde]
pub struct HostConfigResponse {
    pub admin: Addr,
    pub task_ledger: Option<Addr>,
    pub escrow: Option<Addr>,
    pub zk_verifier: Option<Addr>,
    pub allowed_pairs: Vec<Addr>,
}

#[cw_serde]
pub struct HostStatsResponse {
    pub total_accept_task: u64,
    pub total_submit_proof: u64,
    pub total_reclaim: u64,
    pub total_swap: u64,
}
