use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Lifecycle of a registered probe bank.
/// proposed → active → deprecated → revoked
#[cw_serde]
#[derive(Eq)]
pub enum BankStatus {
    Proposed,
    Active,
    Deprecated,
    Revoked,
}

/// Per-concept detection threshold (cosine cutoff).
#[cw_serde]
pub struct ConceptThreshold {
    pub concept: String,
    /// Detection threshold in basis points (7000 = 0.70 cosine).
    /// cosmwasm serde has no float support — integers only.
    pub threshold_bp: u64,
}

/// A registered J-Lens probe bank. One bank binds exactly one model
/// (repo + revision + weights hash) at one or more layers.
///
/// `bank_hash` is sha256 over the bank file bytes — the on-chain
/// commitment the audit server verifies before loading.
#[cw_serde]
pub struct ProbeBank {
    /// Stable identifier, e.g. "qwen2.5-0.5b-instruct"
    pub model_id: String,
    /// HF repo + revision commit sha, e.g. "Qwen/Qwen2.5-0.5B-Instruct@abc123"
    pub model_revision: String,
    /// sha256 over the model's safetensors manifest — pinning the exact
    /// weights means a fine-tune gets a different hash and no bank.
    pub weights_hash: String,
    /// Layers this bank probes (e.g. [12])
    pub layers: Vec<u32>,
    /// Watched/forbidden concepts this bank scores
    pub concepts: Vec<String>,
    /// Probe method tag: contrastive-dom-v0.1 | logreg-v1 |
    /// tuned-lens-v1 | jacobian-v1
    pub method: String,
    /// sha256 over the bank file bytes
    pub bank_hash: String,
    /// Where the public bank lives (ipfs:// or https://) — banks are open
    pub uri: String,
    /// sha256 over the held-out calibration report (AUROC/TPR/FPR per
    /// concept). Must be published before activation.
    pub calibration_hash: String,
    /// Per-concept detection thresholds (concept → cosine cutoff)
    pub thresholds: Vec<ConceptThreshold>,
    /// Lifecycle status
    pub status: BankStatus,
    /// Address that proposed the bank
    pub proposer: Addr,
    /// Block time of proposal / activation (seconds)
    pub proposed_at: u64,
    pub activated_at: Option<u64>,
}

#[cw_serde]
pub struct Config {
    /// DAO / governance address — only it can activate/deprecate/revoke
    pub admin: Addr,
    /// Minimum concepts a bank must cover (initial: all 10 J-Lens concepts)
    pub min_concepts: u32,
    /// Whether activation requires a calibration report hash
    pub require_calibration: bool,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// bank_hash → ProbeBank (primary index)
pub const BANKS: Map<&str, ProbeBank> = Map::new("banks");

/// model_id → Vec<bank_hash> (history per model)
pub const BANKS_BY_MODEL: Map<&str, Vec<String>> = Map::new("banks_by_model");

/// (model_id, weights_hash) → bank_hash — the active bank lookup used
/// by agents, gates and the J-Lens server.
pub const ACTIVE_BANK: Map<(&str, &str), String> = Map::new("active_bank");
