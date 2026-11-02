//! J-Lens thought attestation — before-act enforcement (Phase 3) and
//! chain anchoring of thought records (Phase 4).
//!
//! A `ThoughtRef` is the compact, chain-relevant summary of a
//! `jlens.thought.v1` snapshot produced by `jlens-serve` while the model
//! was generating the decision. It travels inside an `IntentMessage`
//! next to `execution_proof_ref`:
//!
//!   execution_proof_ref → proves WHAT the robot/agent did
//!   thought             → proves HOW its model was thinking when it decided
//!
//! Enforcement (per DAO policy v0): for gated action classes a missing,
//! unverifiable, mismatched or non-Green thought blocks the action
//! (fail-closed). Raw activations never travel — only hashes, the
//! activation merkle root and the integrity score.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::gate::SCORE_SEMANTICS;
use crate::message::{GateVerdict, IntentMessage};

pub const THOUGHT_SCHEMA: &str = "jlens.thought.v1";

/// Trust tier of a thought attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tier {
    /// Operator-signed only. Bootstrap / dev; never sufficient for
    /// gated classes once the DAO flips `bootstrap_dev_gated` off.
    #[serde(rename = "t0-operator")]
    T0Operator,
    /// TEE-attested: the enclave measured the weights hash and signed
    /// the snapshot (WAVS / tee-attestation-verifier).
    #[serde(rename = "t2-tee")]
    T2Tee,
}

/// Compact reference to a jlens.thought.v1 snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThoughtRef {
    pub schema: String,
    pub score_semantics: String,
    pub snapshot_hash: String,
    pub model_id: String,
    pub weights_hash: String,
    pub bank_hash: String,
    /// Binds the thought to exactly this action (see [`action_hash`]).
    pub action_hash: String,
    /// Merkle root over the int8-quantized activations (off-chain blobs).
    pub activation_root: String,
    /// jlens.integrity.v1 — higher = healthier.
    pub integrity: f64,
    pub tier: Tier,
    /// TEE attestation reference (quote hash / tx) — required for T2.
    #[serde(default)]
    pub tee_attestation_ref: Option<String>,
    /// Where the off-chain activation blobs can be fetched for re-audit.
    #[serde(default)]
    pub storage_locator: Option<String>,
}

/// Action classes with distinct enforcement policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionClass {
    Chat,
    Governance,
    FundMovement,
    ContractToolCall,
    PhysicalActuation,
}

impl ActionClass {
    /// Classify an intent's `action` string. Unknown actions on an
    /// IntentMessage are treated as physical actuation — the strictest
    /// class — because intents are robot/agent actions by definition.
    pub fn classify(action: &str) -> Self {
        let a = action.to_ascii_lowercase();
        let has = |keys: &[&str]| keys.iter().any(|k| a.contains(k));
        if has(&["chat", "post", "message", "reply", "announce"]) {
            ActionClass::Chat
        } else if has(&["vote", "propose", "governance"]) {
            ActionClass::Governance
        } else if has(&["transfer", "escrow", "pay", "send_funds", "withdraw", "swap"]) {
            ActionClass::FundMovement
        } else if has(&["execute_contract", "tool_call", "contract", "wasm"]) {
            ActionClass::ContractToolCall
        } else {
            ActionClass::PhysicalActuation
        }
    }
}

/// Enforcement rule for one action class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassRule {
    /// true = missing/failed attestation blocks (fail-closed)
    pub fail_closed: bool,
    pub min_tier: Tier,
    /// Gated classes require Green; Yellow is block-with-warning.
    pub require_green: bool,
}

/// DAO-governed thought policy (initial policy v0, 2026-11-02).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThoughtPolicy {
    pub chat: ClassRule,
    pub governance: ClassRule,
    pub fund_movement: ClassRule,
    pub contract_tool_call: ClassRule,
    pub physical_actuation: ClassRule,
    pub green_threshold: f64,
    pub yellow_threshold: f64,
    /// Bootstrap: while TEE capacity is being wired, T0 thoughts are
    /// accepted for T2-required classes (recorded as dev-gated, still
    /// enforced on integrity). Flipped off by DAO param — no code change.
    pub bootstrap_dev_gated: bool,
}

impl Default for ThoughtPolicy {
    /// DAO policy v0 — see drafts/PLAN_JLENS_V1_THOUGHT_ATTESTATION.md §3.2
    fn default() -> Self {
        let gated = ClassRule {
            fail_closed: true,
            min_tier: Tier::T2Tee,
            require_green: true,
        };
        Self {
            chat: ClassRule {
                fail_closed: false,
                min_tier: Tier::T0Operator,
                require_green: false,
            },
            governance: gated.clone(),
            fund_movement: gated.clone(),
            contract_tool_call: gated.clone(),
            physical_actuation: gated,
            green_threshold: 0.30,
            yellow_threshold: 0.15,
            bootstrap_dev_gated: true,
        }
    }
}

impl ThoughtPolicy {
    pub fn rule(&self, class: ActionClass) -> &ClassRule {
        match class {
            ActionClass::Chat => &self.chat,
            ActionClass::Governance => &self.governance,
            ActionClass::FundMovement => &self.fund_movement,
            ActionClass::ContractToolCall => &self.contract_tool_call,
            ActionClass::PhysicalActuation => &self.physical_actuation,
        }
    }
}

/// Source of truth for which probe banks are active. Production: the
/// probe-registry contract; tests: a static set.
pub trait BankRegistry: Send + Sync {
    fn is_active(&self, model_id: &str, weights_hash: &str, bank_hash: &str) -> bool;
}

/// Static registry — for tests and air-gapped operators mirroring the
/// chain registry locally.
#[derive(Default, Clone)]
pub struct StaticBankRegistry {
    active: HashSet<(String, String, String)>,
}

impl StaticBankRegistry {
    pub fn with(mut self, model_id: &str, weights_hash: &str, bank_hash: &str) -> Self {
        self.active.insert((
            model_id.to_string(),
            weights_hash.to_string(),
            bank_hash.to_string(),
        ));
        self
    }
}

impl BankRegistry for StaticBankRegistry {
    fn is_active(&self, model_id: &str, weights_hash: &str, bank_hash: &str) -> bool {
        self.active.contains(&(
            model_id.to_string(),
            weights_hash.to_string(),
            bank_hash.to_string(),
        ))
    }
}

/// Chain-backed bank registry — mirrors the probe-registry contract's
/// Active banks into a local set via [`ChainBankRegistry::refresh`].
/// `is_active` stays synchronous so the gate never blocks a robot's
/// control loop on RPC. Call `refresh` at startup and whenever
/// BankActivated/BankDeprecated/BankRevoked events are observed.
pub struct ChainBankRegistry {
    rpc: String,
    contract: String,
    client: reqwest::Client,
    /// (model_id, weights_hash, bank_hash) currently Active
    active: RwLock<HashSet<(String, String, String)>>,
    /// (model_id, weights_hash) → active bank_hash
    active_for: RwLock<HashMap<(String, String), String>>,
}

#[derive(Deserialize)]
struct ChainBankEntry {
    model_id: String,
    weights_hash: String,
    bank_hash: String,
}

#[derive(Deserialize)]
struct ChainBankList {
    banks: Vec<ChainBankEntry>,
}

impl ChainBankRegistry {
    pub fn new(rpc: impl Into<String>, contract: impl Into<String>) -> Self {
        Self {
            rpc: rpc.into(),
            contract: contract.into(),
            client: reqwest::Client::new(),
            active: RwLock::new(HashSet::new()),
            active_for: RwLock::new(HashMap::new()),
        }
    }

    /// The Active bank hash for an exact (model_id, weights_hash) pair,
    /// from the last refresh — mirrors the contract's `ActiveBank` query.
    pub fn active_bank(&self, model_id: &str, weights_hash: &str) -> Option<String> {
        self.active_for
            .read()
            .ok()?
            .get(&(model_id.to_string(), weights_hash.to_string()))
            .cloned()
    }

    /// Pull every Active bank from the contract (paginated ListBanks)
    /// and atomically rebuild the local set. Returns the bank count.
    pub async fn refresh(&self) -> anyhow::Result<usize> {
        let mut set = HashSet::new();
        let mut for_map = HashMap::new();
        let mut start_after: Option<String> = None;
        loop {
            let mut q = serde_json::json!({
                "list_banks": { "status": "active", "limit": 100 }
            });
            if let Some(s) = &start_after {
                q["list_banks"]["start_after"] = serde_json::json!(s);
            }
            let page: ChainBankList = self.smart_query(&q).await?;
            if page.banks.is_empty() {
                break;
            }
            let last = page.banks.last().map(|b| b.bank_hash.clone());
            let page_len = page.banks.len();
            for b in page.banks {
                for_map.insert(
                    (b.model_id.clone(), b.weights_hash.clone()),
                    b.bank_hash.clone(),
                );
                set.insert((b.model_id, b.weights_hash, b.bank_hash));
            }
            if page_len < 100 {
                break;
            }
            start_after = last;
        }
        let n = set.len();
        if let Ok(mut a) = self.active.write() {
            *a = set;
        }
        if let Ok(mut m) = self.active_for.write() {
            *m = for_map;
        }
        Ok(n)
    }

    /// abci_query smart-contract call — same envelope as the gate's
    /// OnChainProofVerifier: base64 query inside the SmartContractState path.
    async fn smart_query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let query_bytes = serde_json::to_vec(query)?;
        let query_b64 = base64_encode(&query_bytes);
        let url = format!(
            "{}/abci_query?path=\"/cosmwasm.wasm.v1.Query/SmartContractState/{}%2F{}\"",
            self.rpc.trim_end_matches('/'),
            self.contract,
            query_b64,
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("abci_query HTTP {}", resp.status());
        }
        let body: serde_json::Value = resp.json().await?;
        let value = body
            .get("result")
            .and_then(|r| r.get("response"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("abci_query missing response.value"))?;
        let decoded = base64_decode(value)
            .ok_or_else(|| anyhow::anyhow!("abci_query response.value not base64"))?;
        Ok(serde_json::from_slice(&decoded)?)
    }
}

impl BankRegistry for ChainBankRegistry {
    fn is_active(&self, model_id: &str, weights_hash: &str, bank_hash: &str) -> bool {
        self.active.read().map_or(false, |a| {
            a.contains(&(
                model_id.to_string(),
                weights_hash.to_string(),
                bank_hash.to_string(),
            ))
        })
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let t = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((t >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((t >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            CHARS[((t >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[(t & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let s = s.trim_end_matches('=');
    let mut out = Vec::new();
    for chunk in s.as_bytes().chunks(4) {
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = CHARS.iter().position(|&c| c == b)? as u32;
        }
        let quad = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out.push((quad >> 16) as u8);
        if chunk.len() > 2 {
            out.push((quad >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(quad as u8);
        }
    }
    Some(out)
}

/// TEE attestation verification for T2 thoughts.
pub trait TeeVerifier: Send + Sync {
    fn verify(&self, thought: &ThoughtRef) -> bool;
}

/// Accepts a T2 thought iff it carries a non-empty attestation ref.
/// Production wires the tee-attestation-verifier contract here.
pub struct RefPresentTeeVerifier;

impl TeeVerifier for RefPresentTeeVerifier {
    fn verify(&self, thought: &ThoughtRef) -> bool {
        thought
            .tee_attestation_ref
            .as_deref()
            .map_or(false, |r| !r.is_empty())
    }
}

/// Outcome of checking an intent's thought against policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ThoughtDecision {
    pub verdict: GateVerdict,
    pub class: ActionClass,
    /// Human-readable reason (recorded in BreakerAction / trace).
    pub reason: String,
    /// true = the thought was verified and is authoritative for the
    /// J-Lens verdict (no post-hoc content re-audit needed).
    pub authoritative: bool,
    /// true = accepted under bootstrap dev-gating (T0 for a T2 class).
    pub dev_gated: bool,
}

/// Deterministic action hash — binds a thought to exactly one action.
///
/// sha256 over compact JSON with sorted keys of
/// {action, controller_timestamp, params, robot_id, sensor_snapshot_hash}.
/// jlens-serve computes the same value (thought.py `action_hash`); the
/// cross-language golden lives in j-lens/fixtures/action_hash.v1.json.
pub fn action_hash(intent: &IntentMessage) -> String {
    let v = serde_json::json!({
        "action": intent.action,
        "controller_timestamp": intent.controller_timestamp,
        "params": intent.params,
        "robot_id": intent.robot_id,
        "sensor_snapshot_hash": intent.sensor_snapshot_hash,
    });
    // serde_json::Value objects are BTreeMap-backed → keys sorted.
    let s = serde_json::to_string(&v).unwrap_or_default();
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Check an intent's thought against policy. `expected_model` is the
/// agent's registered (model_id, weights_hash) from agent-registry
/// `BindModel`, if known.
pub fn check_thought(
    intent: &IntentMessage,
    policy: &ThoughtPolicy,
    banks: &dyn BankRegistry,
    tee: &dyn TeeVerifier,
    expected_model: Option<(&str, &str)>,
) -> ThoughtDecision {
    let class = ActionClass::classify(&intent.action);
    let rule = policy.rule(class);

    let fail = |reason: String| ThoughtDecision {
        verdict: if rule.fail_closed {
            GateVerdict::Red { integrity: None }
        } else {
            GateVerdict::Yellow { integrity: None }
        },
        class,
        reason,
        authoritative: false,
        dev_gated: false,
    };

    let t = match &intent.thought {
        Some(t) => t,
        None => return fail("no thought attestation".into()),
    };

    if t.schema != THOUGHT_SCHEMA || t.score_semantics != SCORE_SEMANTICS {
        return fail(format!(
            "thought schema/semantics mismatch ({}, {})",
            t.schema, t.score_semantics
        ));
    }
    if !t.integrity.is_finite() || !(0.0..=1.0).contains(&t.integrity) {
        return fail(format!("thought integrity out of range: {}", t.integrity));
    }
    let expected_action = action_hash(intent);
    if t.action_hash != expected_action {
        return fail("thought not bound to this action (action_hash mismatch)".into());
    }
    if let Some((mid, wh)) = expected_model {
        if t.model_id != mid || t.weights_hash != wh {
            return fail(format!(
                "thought model {}@{} != agent's registered model {}@{}",
                t.model_id,
                short(&t.weights_hash),
                mid,
                short(wh)
            ));
        }
    }
    if !banks.is_active(&t.model_id, &t.weights_hash, &t.bank_hash) {
        return fail(format!(
            "probe bank {} not active for {}",
            short(&t.bank_hash),
            t.model_id
        ));
    }

    let mut dev_gated = false;
    match (t.tier, rule.min_tier) {
        (Tier::T2Tee, _) => {
            if !tee.verify(t) {
                return fail("T2 thought without valid TEE attestation".into());
            }
        }
        (Tier::T0Operator, Tier::T2Tee) => {
            if !policy.bootstrap_dev_gated {
                return fail("T0 thought below required tier T2".into());
            }
            dev_gated = true;
        }
        (Tier::T0Operator, Tier::T0Operator) => {}
    }

    let i = t.integrity;
    let verdict = if i < policy.yellow_threshold {
        GateVerdict::Red { integrity: Some(i) }
    } else if i < policy.green_threshold {
        if rule.require_green {
            GateVerdict::Red { integrity: Some(i) }
        } else {
            GateVerdict::Yellow { integrity: Some(i) }
        }
    } else {
        GateVerdict::Green
    };

    let reason = match &verdict {
        GateVerdict::Green => format!("thought verified (integrity={:.3})", i),
        GateVerdict::Yellow { .. } => format!("thought degraded (integrity={:.3})", i),
        GateVerdict::Red { .. } if i >= policy.yellow_threshold => format!(
            "thought degraded (integrity={:.3}) — gated class requires Green",
            i
        ),
        GateVerdict::Red { .. } => format!("thought red (integrity={:.3})", i),
    };

    ThoughtDecision {
        verdict,
        class,
        reason,
        authoritative: true,
        dev_gated,
    }
}

fn short(h: &str) -> &str {
    &h[..h.len().min(12)]
}

// ════════════════════════════════════════════════════════════════
// Phase 4 — ThoughtRecord batch anchoring
// ════════════════════════════════════════════════════════════════

/// One thought outcome, recorded in the batch and anchored on-chain via
/// the batch's thought merkle root (merkle-verifier `AnchorRoot`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThoughtRecord {
    pub agent_id: String,
    pub action_hash: String,
    pub snapshot_hash: String,
    pub bank_hash: String,
    pub activation_root: String,
    pub integrity: f64,
    pub gate: String,
    pub tier: Tier,
    pub dev_gated: bool,
    pub batch_height: u64,
}

impl ThoughtRecord {
    pub fn from_decision(
        intent: &IntentMessage,
        decision: &ThoughtDecision,
        batch_height: u64,
    ) -> Option<Self> {
        let t = intent.thought.as_ref()?;
        if !decision.authoritative {
            return None;
        }
        let gate = match decision.verdict {
            GateVerdict::Green => "green",
            GateVerdict::Yellow { .. } => "yellow",
            GateVerdict::Red { .. } => "red",
        };
        Some(Self {
            agent_id: intent.robot_id.clone(),
            action_hash: t.action_hash.clone(),
            snapshot_hash: t.snapshot_hash.clone(),
            bank_hash: t.bank_hash.clone(),
            activation_root: t.activation_root.clone(),
            integrity: t.integrity,
            gate: gate.to_string(),
            tier: t.tier,
            dev_gated: decision.dev_gated,
            batch_height,
        })
    }

    /// Leaf hash: sha256 over canonical compact JSON (sorted keys,
    /// integrity as integer micro-units for cross-language determinism).
    pub fn leaf_hash(&self) -> [u8; 32] {
        let v = serde_json::json!({
            "action_hash": self.action_hash,
            "activation_root": self.activation_root,
            "agent_id": self.agent_id,
            "bank_hash": self.bank_hash,
            "batch_height": self.batch_height,
            "dev_gated": self.dev_gated,
            "gate": self.gate,
            "integrity_micro": (self.integrity * 1_000_000.0).round() as i64,
            "snapshot_hash": self.snapshot_hash,
            "tier": self.tier,
        });
        Sha256::digest(serde_json::to_string(&v).unwrap_or_default().as_bytes()).into()
    }
}

/// Merkle root over thought record leaves — same rule as the on-chain
/// merkle-verifier: node = sha256(left || right) over raw 32-byte
/// digests, odd level duplicates the last node.
pub fn thought_merkle_root(leaves: &[[u8; 32]]) -> Option<String> {
    if leaves.is_empty() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            level.push(*level.last().unwrap());
        }
        level = level
            .chunks(2)
            .map(|p| {
                let mut h = Sha256::new();
                h.update(p[0]);
                h.update(p[1]);
                h.finalize().into()
            })
            .collect();
    }
    Some(hex::encode(level[0]))
}

/// Sibling path for leaf `index` — submit with merkle-verifier
/// `VerifyProof { leaf_hash, leaf_index, proof }`.
pub fn thought_merkle_proof(leaves: &[[u8; 32]], index: usize) -> Vec<String> {
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    let mut i = index;
    let mut proof = Vec::new();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            level.push(*level.last().unwrap());
        }
        proof.push(hex::encode(level[i ^ 1]));
        level = level
            .chunks(2)
            .map(|p| {
                let mut h = Sha256::new();
                h.update(p[0]);
                h.update(p[1]);
                h.finalize().into()
            })
            .collect();
        i /= 2;
    }
    proof
}

/// The merkle-verifier `AnchorRoot` message for a batch's thoughts.
/// robot_id is the reserved namespace "jlens:thoughts" so thought roots
/// never collide with per-robot reflex batch roots.
pub fn anchor_root_msg(batch_height: u64, root: &str, count: u32) -> serde_json::Value {
    serde_json::json!({
        "anchor_root": {
            "robot_id": "jlens:thoughts",
            "batch_height": batch_height,
            "merkle_root": root,
            "cycle_count": count,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(action: &str) -> IntentMessage {
        IntentMessage {
            robot_id: "robot-1".into(),
            action: action.into(),
            params: serde_json::json!({"speed": 1, "dir": "north"}),
            sensor_snapshot_hash: "abcd".into(),
            controller_timestamp: 1000,
            rationale: None,
            execution_proof_ref: Some("tx1".into()),
            thought: None,
        }
    }

    fn with_thought(mut i: IntentMessage, integrity: f64, tier: Tier) -> IntentMessage {
        let ah = action_hash(&i);
        i.thought = Some(ThoughtRef {
            schema: THOUGHT_SCHEMA.into(),
            score_semantics: SCORE_SEMANTICS.into(),
            snapshot_hash: "snap".into(),
            model_id: "qwen2.5-0.5b-instruct".into(),
            weights_hash: "w1".into(),
            bank_hash: "b1".into(),
            action_hash: ah,
            activation_root: "root".into(),
            integrity,
            tier,
            tee_attestation_ref: if tier == Tier::T2Tee { Some("quote".into()) } else { None },
            storage_locator: None,
        });
        i
    }

    fn banks() -> StaticBankRegistry {
        StaticBankRegistry::default().with("qwen2.5-0.5b-instruct", "w1", "b1")
    }

    #[test]
    fn classify_actions() {
        assert_eq!(ActionClass::classify("post_chat"), ActionClass::Chat);
        assert_eq!(ActionClass::classify("vote"), ActionClass::Governance);
        assert_eq!(ActionClass::classify("escrow_release"), ActionClass::FundMovement);
        assert_eq!(ActionClass::classify("execute_contract"), ActionClass::ContractToolCall);
        assert_eq!(ActionClass::classify("navigate"), ActionClass::PhysicalActuation);
        assert_eq!(ActionClass::classify("totally_unknown"), ActionClass::PhysicalActuation);
    }

    #[test]
    fn missing_thought_fail_closed_for_robots() {
        let d = check_thought(&intent("navigate"), &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
        assert!(!d.authoritative);
    }

    #[test]
    fn missing_thought_warns_for_chat() {
        let d = check_thought(&intent("post_chat"), &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Yellow { integrity: None });
    }

    #[test]
    fn green_t2_thought_passes() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Green);
        assert!(d.authoritative);
        assert!(!d.dev_gated);
    }

    #[test]
    fn yellow_blocks_gated_class() {
        let i = with_thought(intent("navigate"), 0.2, Tier::T2Tee);
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: Some(0.2) });
        assert!(d.reason.contains("requires Green"));
    }

    #[test]
    fn yellow_warns_for_chat() {
        let i = with_thought(intent("post_chat"), 0.2, Tier::T0Operator);
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Yellow { integrity: Some(0.2) });
    }

    #[test]
    fn t0_for_t2_class_dev_gated_then_blocked() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T0Operator);
        let mut p = ThoughtPolicy::default();
        let d = check_thought(&i, &p, &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Green);
        assert!(d.dev_gated);

        p.bootstrap_dev_gated = false;
        let d = check_thought(&i, &p, &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
    }

    #[test]
    fn t2_without_quote_blocked() {
        let mut i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        i.thought.as_mut().unwrap().tee_attestation_ref = None;
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
    }

    #[test]
    fn inactive_bank_blocked() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        let empty = StaticBankRegistry::default();
        let d = check_thought(&i, &ThoughtPolicy::default(), &empty, &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
        assert!(d.reason.contains("not active"));
    }

    #[test]
    fn thought_replayed_on_other_action_blocked() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        let mut replay = i.clone();
        replay.params = serde_json::json!({"speed": 9, "dir": "south"});
        let d = check_thought(&replay, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
        assert!(d.reason.contains("action_hash"));
    }

    #[test]
    fn model_swap_blocked() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        let d = check_thought(
            &i,
            &ThoughtPolicy::default(),
            &banks(),
            &RefPresentTeeVerifier,
            Some(("qwen2.5-0.5b-instruct", "w-other")),
        );
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
        assert!(d.reason.contains("registered model"));
    }

    #[test]
    fn red_integrity_blocks() {
        let i = with_thought(intent("navigate"), 0.05, Tier::T2Tee);
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: Some(0.05) });
        assert!(d.authoritative);
    }

    #[test]
    fn bad_semantics_blocked() {
        let mut i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        i.thought.as_mut().unwrap().score_semantics = "legacy.risk.v0".into();
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        assert_eq!(d.verdict, GateVerdict::Red { integrity: None });
    }

    #[test]
    fn action_hash_cross_language_golden() {
        // Golden shared with jlens-serve/thought.py action_hash()
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/brainmaxx/j-lens/fixtures/action_hash.v1.json"
        );
        let raw = std::fs::read_to_string(path).expect("golden fixture");
        let g: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let i: IntentMessage = serde_json::from_value(g["intent"].clone()).unwrap();
        assert_eq!(action_hash(&i), g["action_hash"].as_str().unwrap());
    }

    #[test]
    fn merkle_root_and_proofs_verify() {
        let leaves: Vec<[u8; 32]> = (0..5u8).map(|b| Sha256::digest([b]).into()).collect();
        let root = thought_merkle_root(&leaves).unwrap();
        for (idx, leaf) in leaves.iter().enumerate() {
            let proof = thought_merkle_proof(&leaves, idx);
            // merkle-verifier algorithm
            let mut cur = leaf.to_vec();
            for (level, sib_hex) in proof.iter().enumerate() {
                let sib = hex::decode(sib_hex).unwrap();
                let mut h = Sha256::new();
                if (idx >> level) & 1 == 0 {
                    h.update(&cur);
                    h.update(&sib);
                } else {
                    h.update(&sib);
                    h.update(&cur);
                }
                cur = h.finalize().to_vec();
            }
            assert_eq!(hex::encode(cur), root, "proof for leaf {}", idx);
        }
        assert!(thought_merkle_root(&[]).is_none());
    }

    // Chain-backed registry: axum stub serves a canned abci_query
    // SmartContractState response; refresh() must populate the active set
    // and is_active must stay synchronous.

    use axum::{routing::get, Json, Router};

    fn b64(data: &[u8]) -> String { base64_encode(data) }

    #[tokio::test]
    async fn chain_registry_refresh_populates_active() {
        let bank_json = serde_json::json!({
            "banks": [{
                "model_id": "qwen2.5-0.5b-instruct",
                "weights_hash": "w-chain",
                "bank_hash": "b-chain"
            }]
        });
        let value = b64(serde_json::to_string(&bank_json).unwrap().as_bytes());
        let app = Router::new().route(
            "/abci_query",
            get(move || {
                let value = value.clone();
                async move {
                    Json(serde_json::json!({
                        "result": { "response": { "value": value } }
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let reg = ChainBankRegistry::new(format!("http://{}", addr), "juno1contract");
        // Before refresh: nothing is active
        assert!(!reg.is_active("qwen2.5-0.5b-instruct", "w-chain", "b-chain"));
        let n = reg.refresh().await.unwrap();
        assert_eq!(n, 1);
        assert!(reg.is_active("qwen2.5-0.5b-instruct", "w-chain", "b-chain"));
        assert!(!reg.is_active("qwen2.5-0.5b-instruct", "w-chain", "other-bank"));
        assert_eq!(
            reg.active_bank("qwen2.5-0.5b-instruct", "w-chain").as_deref(),
            Some("b-chain")
        );
    }

    #[test]
    fn record_leaf_deterministic() {
        let i = with_thought(intent("navigate"), 0.9, Tier::T2Tee);
        let d = check_thought(&i, &ThoughtPolicy::default(), &banks(), &RefPresentTeeVerifier, None);
        let r = ThoughtRecord::from_decision(&i, &d, 7).unwrap();
        assert_eq!(r.leaf_hash(), r.clone().leaf_hash());
        let msg = anchor_root_msg(7, "aa", 1);
        assert_eq!(msg["anchor_root"]["robot_id"], "jlens:thoughts");
    }
}
