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

use junoclaw_coordination::{AgentMessage, Batch, GateResult, GateVerdict};

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
      JsGateVerdict::Yellow => GateVerdict::Yellow { separation_score: 0.0 },
      JsGateVerdict::Red => GateVerdict::Red { separation_score: 0.0 },
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
    Batch::new(messages, hash, batch.height as u64, batch.timestamp as u64)
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
  pub separation_score: f64,
  pub model_id: Option<String>,
}

#[napi]
pub fn create_gate_result(
  verdict: JsGateVerdict,
  separation_score: f64,
  attestation_hash: Option<String>,
  model_id: Option<String>,
) -> JsGateResult {
  JsGateResult {
    verdict,
    attestation_hash,
    separation_score,
    model_id,
  }
}

impl From<GateResult> for JsGateResult {
  fn from(result: GateResult) -> Self {
    JsGateResult {
      verdict: result.verdict.into(),
      attestation_hash: result.attestation_hash,
      separation_score: result.separation_score,
      model_id: result.model_id,
    }
  }
}
