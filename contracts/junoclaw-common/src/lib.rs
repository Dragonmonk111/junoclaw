use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

// ──────────────────────────────────────────────
// Agent Registry Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct AgentProfile {
    pub owner: Addr,
    pub name: String,
    pub description: String,
    pub capabilities_hash: String,
    pub model: String,
    pub registered_at: u64,
    pub is_active: bool,
    pub total_tasks: u64,
    pub successful_tasks: u64,
    /// On-chain reputation score. Incremented on success, decremented on slash.
    pub trust_score: u64,
}

// ──────────────────────────────────────────────
// Task Ledger Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct TaskRecord {
    pub id: u64,
    pub agent_id: u64,
    pub submitter: Addr,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub execution_tier: ExecutionTier,
    pub status: TaskStatus,
    pub submitted_at: u64,
    pub completed_at: Option<u64>,
    pub cost_ujuno: Option<Uint128>,
}

#[cw_serde]
pub enum ExecutionTier {
    Local,
    Akash,
}

#[cw_serde]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

// ──────────────────────────────────────────────
// Payment Ledger Types (non-custodial)
// ──────────────────────────────────────────────

/// A payment obligation records that a payer owes a payee some amount.
/// The contract never holds funds — it only tracks the obligation state.
/// Actual transfers happen via the payer's wallet signature.
#[cw_serde]
pub struct PaymentObligation {
    pub id: u64,
    pub payer: Addr,
    pub payee: Addr,
    pub task_id: u64,
    pub amount: Uint128,
    pub denom: String,
    pub status: ObligationStatus,
    pub created_at: u64,
    pub settled_at: Option<u64>,
    /// WAVS attestation hash proving the obligation is valid
    pub attestation_hash: Option<String>,
}

#[cw_serde]
pub enum ObligationStatus {
    /// Obligation recorded, awaiting payer confirmation
    Pending,
    /// Payer confirmed and sent funds directly to payee
    Confirmed,
    /// Obligation disputed by payer
    Disputed,
    /// Obligation cancelled (mutual or admin)
    Cancelled,
    /// Verified by WAVS attestation
    Verified,
}

// ── Deprecated aliases for migration ──
pub type EscrowDeposit = PaymentObligation;
pub type EscrowStatus = ObligationStatus;

// ──────────────────────────────────────────────
// Junoswap v2 DEX Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct PairInfo {
    pub pair_addr: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub lp_token: Addr,
    pub total_fee_bps: u16,
    pub wavs_verified: bool,
}

#[cw_serde]
pub enum AssetInfo {
    Native(String),
    Cw20(Addr),
}

impl AssetInfo {
    pub fn denom_key(&self) -> String {
        match self {
            AssetInfo::Native(d) => d.clone(),
            AssetInfo::Cw20(a) => a.to_string(),
        }
    }
}

#[cw_serde]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

#[cw_serde]
pub struct SwapEvent {
    pub pair: String,
    pub sender: String,
    pub offer_asset: String,
    pub offer_amount: Uint128,
    pub return_asset: String,
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub fee_amount: Uint128,
    pub block_height: u64,
    pub timestamp: u64,
}

// ──────────────────────────────────────────────
// Contract Registry (V30-safe inter-contract refs)
// ──────────────────────────────────────────────

#[cw_serde]
pub struct ContractRegistry {
    pub agent_registry: Option<Addr>,
    pub task_ledger: Option<Addr>,
    pub escrow: Option<Addr>,
}
