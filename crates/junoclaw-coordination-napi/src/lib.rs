//! JunoClaw Coordination NAPI Bridge
//!
//! Exposes the core coordination types (AgentMessage, Batch, GateVerdict,
//! GateResult) to JavaScript/TypeScript via napi-rs.
//!
//! Usage from Node.js:
//! ```js
//! const { createAgentMessage, decodeAgentMessage, createBatch } = require('@junoclaw/coordination');
//!
//! const msg = createAgentMessage({
//!   from: Buffer.from([1,2,3]),
//!   to: Buffer.from([4,5,6]),
//!   content: Buffer.from('hello'),
//!   timestamp: Date.now(),
//! });
//! console.log(msg.contentHash); // Uint8Array(32)
//! ```

use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;

use junoclaw_coordination::thought::{self, ThoughtRecord, ThoughtRef, Tier};
use junoclaw_coordination::{AgentMessage, Batch, GateResult, GateVerdict, IntentMessage};

// ─── GateVerdict ───────────────────────────────────────────────────────

#[napi]
pub enum JsGateVerdict {
  Green,
  Yellow,
  Red,
}

impl From<JsGateVerdict> for GateVerdict {
  fn from(v: JsGateVerdict) -> Self {
    match v {
      JsGateVerdict::Green => GateVerdict::Green,
      JsGateVerdict::Yellow => GateVerdict::Yellow { integrity: None },
      JsGateVerdict::Red => GateVerdict::Red { integrity: None },
    }
  }
}

impl From<GateVerdict> for JsGateVerdict {
  fn from(v: GateVerdict) -> Self {
    match v {
      GateVerdict::Green => JsGateVerdict::Green,
      GateVerdict::Yellow { .. } => JsGateVerdict::Yellow,
      GateVerdict::Red { .. } => JsGateVerdict::Red,
    }
  }
}

// ─── AgentMessage ──────────────────────────────────────────────────────

#[napi(object)]
pub struct JsAgentMessage {
  pub from: Uint8Array,
  pub to: Uint8Array,
  pub content: Uint8Array,
  pub content_hash: Uint8Array,
  pub timestamp: i64,
  pub j_lens_gate: Option<JsGateVerdict>,
  pub proposal_ref: Option<i64>,
}

#[napi]
pub fn create_agent_message(
  from: Uint8Array,
  to: Uint8Array,
  content: Uint8Array,
  timestamp: i64,
) -> JsAgentMessage {
  let msg = AgentMessage::new(from.to_vec(), to.to_vec(), content.to_vec(), timestamp as u64);
  msg.into()
}

impl From<AgentMessage> for JsAgentMessage {
  fn from(msg: AgentMessage) -> Self {
    JsAgentMessage {
      from: Uint8Array::from(msg.from),
      to: Uint8Array::from(msg.to),
      content: Uint8Array::from(msg.content),
      content_hash: Uint8Array::from(msg.content_hash.to_vec()),
      timestamp: msg.timestamp as i64,
      j_lens_gate: msg.j_lens_gate.map(|v| v.into()),
      proposal_ref: msg.proposal_ref.map(|v| v as i64),
    }
  }
}

impl From<JsAgentMessage> for AgentMessage {
  fn from(msg: JsAgentMessage) -> Self {
    let mut hash = [0u8; 32];
    let ch = msg.content_hash.to_vec();
    if ch.len() == 32 {
      hash.copy_from_slice(&ch);
    }
    AgentMessage {
      from: msg.from.to_vec(),
      to: msg.to.to_vec(),
      content: msg.content.to_vec(),
      content_hash: hash,
      timestamp: msg.timestamp as u64,
      j_lens_gate: msg.j_lens_gate.map(|v| v.into()),
      proposal_ref: msg.proposal_ref.map(|v| v as u64),
    }
  }
}

#[napi]
pub fn encode_agent_message(msg: JsAgentMessage) -> Uint8Array {
  let rust_msg: AgentMessage = msg.into();
  let encoded = rust_msg.encode().unwrap_or_default();
  Uint8Array::from(encoded)
}

#[napi]
pub fn decode_agent_message(data: Uint8Array) -> JsAgentMessage {
  let msg = AgentMessage::decode(&data).unwrap_or_else(|_| AgentMessage::new(vec![], vec![], vec![], 0));
  msg.into()
}

#[napi]
pub fn verify_message_hash(msg: JsAgentMessage) -> bool {
  let rust_msg: AgentMessage = msg.into();
  rust_msg.verify_hash()
}

#[napi]
pub fn is_broadcast_message(msg: JsAgentMessage) -> bool {
  let rust_msg: AgentMessage = msg.into();
  rust_msg.is_broadcast()
}

// ─── Batch ─────────────────────────────────────────────────────────────

#[napi(object)]
pub struct JsBatch {
  pub messages: Vec<JsAgentMessage>,
  pub prev_hash: Uint8Array,
  pub height: i64,
  pub timestamp: i64,
  /// Thought records anchored in this batch (merkle-committed).
  pub thought_records: Vec<JsThoughtRecord>,
  /// Merkle root over thought_records leaf hashes (hex).
  pub thought_root: Option<String>,
}

#[napi]
pub fn create_batch(
  messages: Vec<JsAgentMessage>,
  prev_hash: Uint8Array,
  height: i64,
  timestamp: i64,
) -> JsBatch {
  let mut hash = [0u8; 32];
  let ph = prev_hash.to_vec();
  if ph.len() == 32 {
    hash.copy_from_slice(&ph);
  }
  let rust_messages: Vec<AgentMessage> = messages.into_iter().map(|m| m.into()).collect();
  let batch = Batch::new(rust_messages, hash, height as u64, timestamp as u64);
  batch.into()
}

impl From<Batch> for JsBatch {
  fn from(batch: Batch) -> Self {
    JsBatch {
      messages: batch.messages.into_iter().map(|m| m.into()).collect(),
      prev_hash: Uint8Array::from(batch.prev_hash.to_vec()),
      height: batch.height as i64,
      timestamp: batch.timestamp as i64,
      thought_records: batch.thought_records.into_iter().map(|r| r.into()).collect(),
      thought_root: batch.thought_root,
    }
  }
}

impl From<JsBatch> for Batch {
  fn from(batch: JsBatch) -> Self {
    let mut hash = [0u8; 32];
    let ph = batch.prev_hash.to_vec();
    if ph.len() == 32 {
      hash.copy_from_slice(&ph);
    }
    let messages: Vec<AgentMessage> = batch.messages.into_iter().map(|m| m.into()).collect();
    let mut b = Batch::new(messages, hash, batch.height as u64, batch.timestamp as u64);
    let records: Vec<ThoughtRecord> =
      batch.thought_records.into_iter().filter_map(|r| r.try_into().ok()).collect();
    if !records.is_empty() {
      b = b.with_thought_records(records);
    }
    if b.thought_root.is_none() {
      b.thought_root = batch.thought_root;
    }
    b
  }
}

#[napi]
pub fn hash_batch(batch: JsBatch) -> Uint8Array {
  let rust_batch: Batch = batch.into();
  let hash = rust_batch.hash();
  Uint8Array::from(hash.to_vec())
}

#[napi]
pub fn batch_has_blocked_message(batch: JsBatch) -> bool {
  let rust_batch: Batch = batch.into();
  rust_batch.has_blocked_message()
}

#[napi]
pub fn batch_len(batch: JsBatch) -> i64 {
  let rust_batch: Batch = batch.into();
  rust_batch.len() as i64
}

// ─── GateResult ────────────────────────────────────────────────────────

#[napi(object)]
pub struct JsGateResult {
  pub verdict: JsGateVerdict,
  pub attestation_hash: Option<String>,
  /// Worst audited integrity (jlens.integrity.v1, higher = healthier);
  /// absent when nothing was audited.
  pub integrity: Option<f64>,
  pub model_id: Option<String>,
}

#[napi]
pub fn create_gate_result(
  verdict: JsGateVerdict,
  integrity: Option<f64>,
  attestation_hash: Option<String>,
  model_id: Option<String>,
) -> JsGateResult {
  JsGateResult {
    verdict,
    attestation_hash,
    integrity,
    model_id,
  }
}

impl From<GateResult> for JsGateResult {
  fn from(result: GateResult) -> Self {
    JsGateResult {
      verdict: result.verdict.into(),
      attestation_hash: result.attestation_hash,
      integrity: result.integrity,
      model_id: result.model_id,
    }
  }
}

// ─── Thought attestations (jlens.thought.v1) ────────────────────────

/// Compact reference to a jlens.thought.v1 snapshot — travels inside
/// an IntentMessage next to execution_proof_ref.
#[napi(object)]
pub struct JsThoughtRef {
  pub schema: String,
  pub score_semantics: String,
  pub snapshot_hash: String,
  pub model_id: String,
  pub weights_hash: String,
  pub bank_hash: String,
  pub action_hash: String,
  pub activation_root: String,
  /// jlens.integrity.v1 — higher = healthier.
  pub integrity: f64,
  /// "t0-operator" | "t2-tee"
  pub tier: String,
  pub tee_attestation_ref: Option<String>,
  pub storage_locator: Option<String>,
}

fn tier_to_string(t: Tier) -> String {
  serde_json::to_value(t)
    .ok()
    .and_then(|v| v.as_str().map(str::to_string))
    .unwrap_or_default()
}

fn tier_from_string(s: &str) -> Option<Tier> {
  serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

impl From<ThoughtRef> for JsThoughtRef {
  fn from(t: ThoughtRef) -> Self {
    JsThoughtRef {
      schema: t.schema,
      score_semantics: t.score_semantics,
      snapshot_hash: t.snapshot_hash,
      model_id: t.model_id,
      weights_hash: t.weights_hash,
      bank_hash: t.bank_hash,
      action_hash: t.action_hash,
      activation_root: t.activation_root,
      integrity: t.integrity,
      tier: tier_to_string(t.tier),
      tee_attestation_ref: t.tee_attestation_ref,
      storage_locator: t.storage_locator,
    }
  }
}

impl TryFrom<JsThoughtRef> for ThoughtRef {
  type Error = String;
  fn try_from(t: JsThoughtRef) -> Result<Self, Self::Error> {
    Ok(ThoughtRef {
      schema: t.schema,
      score_semantics: t.score_semantics,
      snapshot_hash: t.snapshot_hash,
      model_id: t.model_id,
      weights_hash: t.weights_hash,
      bank_hash: t.bank_hash,
      action_hash: t.action_hash,
      activation_root: t.activation_root,
      integrity: t.integrity,
      tier: tier_from_string(&t.tier).ok_or_else(|| format!("unknown tier: {}", t.tier))?,
      tee_attestation_ref: t.tee_attestation_ref,
      storage_locator: t.storage_locator,
    })
  }
}

/// Robot/agent intent — the auditable action envelope.
/// `params_json` is the structured params object serialized as JSON.
#[napi(object)]
pub struct JsIntentMessage {
  pub robot_id: String,
  pub action: String,
  pub params_json: String,
  pub sensor_snapshot_hash: String,
  pub controller_timestamp: i64,
  pub rationale: Option<String>,
  pub execution_proof_ref: Option<String>,
  /// Generation-time J-Lens thought attestation (fail-closed for
  /// gated action classes under DAO thought policy).
  pub thought: Option<JsThoughtRef>,
}

impl TryFrom<JsIntentMessage> for IntentMessage {
  type Error = String;
  fn try_from(m: JsIntentMessage) -> Result<Self, Self::Error> {
    Ok(IntentMessage {
      robot_id: m.robot_id,
      action: m.action,
      params: serde_json::from_str(&m.params_json)
        .map_err(|e| format!("params_json is not valid JSON: {}", e))?,
      sensor_snapshot_hash: m.sensor_snapshot_hash,
      controller_timestamp: m.controller_timestamp as u64,
      rationale: m.rationale,
      execution_proof_ref: m.execution_proof_ref,
      thought: m.thought.map(TryInto::try_into).transpose()?,
    })
  }
}

impl From<IntentMessage> for JsIntentMessage {
  fn from(m: IntentMessage) -> Self {
    JsIntentMessage {
      robot_id: m.robot_id,
      action: m.action,
      params_json: serde_json::to_string(&m.params).unwrap_or_else(|_| "null".into()),
      sensor_snapshot_hash: m.sensor_snapshot_hash,
      controller_timestamp: m.controller_timestamp as i64,
      rationale: m.rationale,
      execution_proof_ref: m.execution_proof_ref,
      thought: m.thought.map(Into::into),
    }
  }
}

#[napi]
pub fn encode_intent_message(msg: JsIntentMessage) -> Uint8Array {
  let intent: Result<IntentMessage, _> = msg.try_into();
  match intent {
    Ok(i) => Uint8Array::from(i.encode().unwrap_or_default()),
    Err(_) => Uint8Array::from(vec![]),
  }
}

#[napi]
pub fn decode_intent_message(data: Uint8Array) -> JsIntentMessage {
  let fallback = IntentMessage {
    robot_id: String::new(),
    action: String::new(),
    params: serde_json::Value::Null,
    sensor_snapshot_hash: String::new(),
    controller_timestamp: 0,
    rationale: None,
    execution_proof_ref: None,
    thought: None,
  };
  IntentMessage::decode(&data).unwrap_or(fallback).into()
}

/// Deterministic action hash — binds a thought to exactly one action.
/// Same value as jlens-serve thought.py `action_hash` (cross-language
/// golden: j-lens/fixtures/action_hash.v1.json).
#[napi]
pub fn action_hash_for_intent(msg: JsIntentMessage) -> String {
  let intent: Result<IntentMessage, _> = msg.try_into();
  match intent {
    Ok(i) => thought::action_hash(&i),
    Err(_) => String::new(),
  }
}

/// One thought outcome anchored in a batch (merkle leaf).
#[napi(object)]
pub struct JsThoughtRecord {
  pub agent_id: String,
  pub action_hash: String,
  pub snapshot_hash: String,
  pub bank_hash: String,
  pub activation_root: String,
  pub integrity: f64,
  /// "green" | "yellow" | "red"
  pub gate: String,
  /// "t0-operator" | "t2-tee"
  pub tier: String,
  pub dev_gated: bool,
  pub batch_height: i64,
}

impl From<ThoughtRecord> for JsThoughtRecord {
  fn from(r: ThoughtRecord) -> Self {
    JsThoughtRecord {
      agent_id: r.agent_id,
      action_hash: r.action_hash,
      snapshot_hash: r.snapshot_hash,
      bank_hash: r.bank_hash,
      activation_root: r.activation_root,
      integrity: r.integrity,
      gate: r.gate,
      tier: tier_to_string(r.tier),
      dev_gated: r.dev_gated,
      batch_height: r.batch_height as i64,
    }
  }
}

impl TryFrom<JsThoughtRecord> for ThoughtRecord {
  type Error = String;
  fn try_from(r: JsThoughtRecord) -> Result<Self, Self::Error> {
    Ok(ThoughtRecord {
      agent_id: r.agent_id,
      action_hash: r.action_hash,
      snapshot_hash: r.snapshot_hash,
      bank_hash: r.bank_hash,
      activation_root: r.activation_root,
      integrity: r.integrity,
      gate: r.gate,
      tier: tier_from_string(&r.tier).ok_or_else(|| format!("unknown tier: {}", r.tier))?,
      dev_gated: r.dev_gated,
      batch_height: r.batch_height as u64,
    })
  }
}
