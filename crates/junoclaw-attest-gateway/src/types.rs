//! Request/response/record types for the /attest API.

use serde::{Deserialize, Serialize};

// ── inbound ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AttestRequest {
    pub idempotency_key: String,
    pub action: Action,
    #[serde(default)]
    pub context: Option<Context>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub callback_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub kind: String,
    /// Raw JSON payload — hashed server-side, discarded unless retained.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    /// Pre-hashed payload ("sha256:<hex>") — preferred privacy tier.
    #[serde(default)]
    pub payload_hash: Option<String>,
    #[serde(default)]
    pub canonicalization: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<i64>,
    #[serde(default)]
    pub policy_ref: Option<String>,
    #[serde(default)]
    pub jlens: Option<JlensRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JlensRef {
    #[serde(default)]
    pub attestation_id: Option<String>,
    #[serde(default)]
    pub bank_hash: Option<String>,
    #[serde(default)]
    pub integrity: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchRequest {
    pub items: Vec<AttestRequest>,
}

// ── stored record ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub attestation_id: String,
    pub customer: String,
    pub idempotency_key: String,
    /// sha256 of the canonical request body — detects key reuse w/ diff body.
    pub body_hash: String,
    pub leaf_hash: String, // hex
    pub kind: String,
    pub agent_id: Option<String>,
    pub occurred_at: Option<i64>,
    pub jlens: Option<JlensRef>,
    pub tier: String,
    pub callback_url: Option<String>,
    pub status: AttStatus,
    pub submitted_at: i64, // unix secs
    #[serde(default)]
    pub batch: Option<BatchInfo>,
    /// Set only if the customer asked us to retain the payload.
    #[serde(default)]
    pub retained_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttStatus {
    Queued,
    Anchoring,
    Anchored,
    AnchorFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub robot_id: String,
    pub batch_height: u64,
    pub merkle_root: String,
    pub leaf_index: u32,
    pub proof: Vec<String>,
    pub anchor_tx: String,
    pub anchored_at_height: u64,
}

// ── outbound ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AttestResponse {
    pub attestation_id: String,
    pub status: &'static str,
    pub est_anchor_secs: u64,
    pub deduplicated: bool,
}

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub customer: String,
    pub submitted: u64,
    pub anchored: u64,
    pub failed: u64,
    pub queued: u64,
}

// ── spool entries ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum SpoolEntry {
    #[serde(rename = "submit")]
    Submit { att: Attestation },
    #[serde(rename = "batch")]
    Batch {
        batch_height: u64,
        robot_id: String,
        merkle_root: String,
        anchor_tx: String,
        anchored_at_height: u64,
        /// (attestation_id, leaf_index) in this batch.
        items: Vec<(String, u32)>,
    },
    #[serde(rename = "batch_failed")]
    BatchFailed {
        batch_height: u64,
        items: Vec<String>,
        error: String,
    },
    #[serde(rename = "purge")]
    Purge { attestation_id: String },
}
