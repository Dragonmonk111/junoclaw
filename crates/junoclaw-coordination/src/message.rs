//! AgentMessage protocol — the wire format for agent-to-agent communication
//! over the coordination network.
//!
//! Every message is:
//! - Authenticated: sender identity is verified via P2P signed connections
//! - Content-hashed: SHA-256 of content for integrity verification
//! - J-Lens gated: optional audit verdict attached before relay

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// J-Lens gate verdict for a message.
/// Green = clean, Yellow = warning (suspicious but not blocked), Red = blocked.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GateVerdict {
    Green,
    Yellow { separation_score: f64 },
    Red { separation_score: f64 },
}

/// Result of a J-Lens gate audit on a batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateResult {
    pub verdict: GateVerdict,
    pub attestation_hash: Option<String>,
    pub separation_score: f64,
    pub model_id: Option<String>,
}

/// The core message type exchanged over the coordination network.
///
/// Wire format is CBOR/JSON serializable for storage and relay.
/// Content is opaque bytes — the coordination layer does not interpret content,
/// only hashes it and routes it through the J-Lens gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Sender's public key (ed25519, 32 bytes)
    pub from: Vec<u8>,
    /// Recipient's public key (32 bytes), or empty for broadcast
    pub to: Vec<u8>,
    /// Opaque message content
    pub content: Vec<u8>,
    /// SHA-256 hash of content (computed at send time, verified at receive)
    pub content_hash: [u8; 32],
    /// Unix timestamp (milliseconds) when message was created
    pub timestamp: u64,
    /// Optional J-Lens gate verdict (attached before relay)
    pub j_lens_gate: Option<GateVerdict>,
    /// Optional proposal/batch reference for DAO context
    pub proposal_ref: Option<u64>,
}

impl AgentMessage {
    /// Create a new agent message, computing the content hash.
    pub fn new(
        from: Vec<u8>,
        to: Vec<u8>,
        content: Vec<u8>,
        timestamp: u64,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let content_hash = hasher.finalize().into();
        Self {
            from,
            to,
            content,
            content_hash,
            timestamp,
            j_lens_gate: None,
            proposal_ref: None,
        }
    }

    /// Verify that the content hash matches the content.
    pub fn verify_hash(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.content);
        let computed: [u8; 32] = hasher.finalize().into();
        computed == self.content_hash
    }

    /// Check if this is a broadcast message (no specific recipient).
    pub fn is_broadcast(&self) -> bool {
        self.to.is_empty()
    }

    /// Attach a J-Lens gate verdict to the message.
    pub fn with_gate(mut self, verdict: GateVerdict) -> Self {
        self.j_lens_gate = Some(verdict);
        self
    }

    /// Attach a proposal reference for DAO context.
    pub fn with_proposal_ref(mut self, proposal_id: u64) -> Self {
        self.proposal_ref = Some(proposal_id);
        self
    }

    /// Serialize to bytes for P2P transmission.
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from bytes received over P2P.
    pub fn decode(data: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// A structured task request embedded in an AgentMessage's content field.
///
/// When an agent wants to execute a task (shell command, compute job, browser
/// action), it encodes a TaskRequest as JSON in the message content. The
/// coordination layer orders it, J-Lens gates it, and the relayer extracts
/// it post-settlement to submit to the task-ledger contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    /// Agent's on-chain ID (from agent-registry)
    pub agent_id: u64,
    /// SHA-256 hash of the task input (the actual input lives off-chain)
    pub input_hash: String,
    /// Execution tier: "local" or "akash"
    pub execution_tier: String,
    /// Optional plugin hint (e.g. "plugin-shell", "plugin-compute-akash")
    pub plugin_hint: Option<String>,
    /// Optional proposal ID if this task originated from a DAO vote
    pub proposal_id: Option<u64>,
}

impl TaskRequest {
    /// Encode as JSON bytes for embedding in AgentMessage.content
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode from AgentMessage.content bytes
    pub fn decode(content: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(content)?)
    }
}

/// An evaluator's attestation for the truth market (layer 6).
///
/// Each J-Lens operator independently evaluates a batch and submits a
/// verdict. Multiple attestations are aggregated by the coordination mesh;
/// operators matching consensus earn rewards, diverging operators get slashed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalAttestation {
    /// Evaluator's public key (ed25519, 32 bytes)
    pub operator_pubkey: Vec<u8>,
    /// The evaluator's verdict on the batch
    pub verdict: GateVerdict,
    /// Batch height being attested
    pub batch_height: u64,
    /// Operator's signature over (batch_height || verdict || messages_hash)
    pub signature: Vec<u8>,
}

/// A robot's intent-tier decision, encoded as structured content inside an
/// `AgentMessage`. This is the typed schema that distinguishes a robot's
/// auditable decision ("engage target", "take this route") from a generic
/// agent message.
///
/// The reflex-tier (sub-100ms sensor fusion, balance, collision avoidance)
/// never becomes an `IntentMessage` — it stays on the robot's controller.
/// Only intent-tier decisions that need on-chain audit are wrapped in this
/// schema and fed through the gate.
///
/// The split is architectural: `AgentMessage.content` is opaque bytes, but
/// when a robot plugin (e.g. `plugin-ros2`) encodes an `IntentMessage` into
/// the content field, the gate and Truth Market can interpret the structured
/// intent for verdict evaluation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentMessage {
    /// Robot's unique identifier (matches skill-registry dapp_name)
    pub robot_id: String,
    /// The intent action being audited (e.g. "engage", "navigate", "pick_tool")
    pub action: String,
    /// Structured action parameters (JSON-encoded, action-specific)
    pub params: serde_json::Value,
    /// SHA-256 hash of the robot's sensor snapshot at decision time
    pub sensor_snapshot_hash: String,
    /// Robot's controller timestamp (ms) when the intent was emitted
    pub controller_timestamp: u64,
    /// Optional human-readable rationale (for audit trail)
    pub rationale: Option<String>,
    /// Optional execution proof reference (rosbag path, action server result ID)
    pub execution_proof_ref: Option<String>,
}

impl IntentMessage {
    /// Encode as JSON bytes for embedding in AgentMessage.content
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode from AgentMessage.content bytes
    pub fn decode(content: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(content)?)
    }

    /// Create an AgentMessage wrapping this intent
    pub fn into_agent_message(
        self,
        from: Vec<u8>,
        to: Vec<u8>,
        timestamp: u64,
    ) -> anyhow::Result<AgentMessage> {
        let content = self.encode()?;
        Ok(AgentMessage::new(from, to, content, timestamp))
    }
}

/// The outcome of an intent-tier decision, settled by the Truth Market.
///
/// After the gate audits the `IntentMessage` and the Truth Market reaches
/// consensus, the outcome is recorded for settlement (reward/slash) and
/// for the robot's execution log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentOutcome {
    /// Reference to the original IntentMessage (content hash)
    pub intent_hash: [u8; 32],
    /// Gate verdict for this intent
    pub verdict: GateVerdict,
    /// Truth Market consensus ratio (0.0 - 1.0)
    pub consensus_ratio: f64,
    /// Whether the robot's action matched the audited intent
    pub action_matched: bool,
    /// Batch height where this intent was settled
    pub batch_height: u64,
}

/// A robot's declared safety envelope — the operating parameters that its
/// reflex-tier controller must respect. Stored on-chain (governance-controlled)
/// so that changing safety bounds requires a transaction, not a YAML edit.
///
/// The controller reads these at startup and enforces them in its reflex loops.
/// A `ReflexBatchAttestation` proves the envelope was maintained across a batch
/// of reflex cycles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetyEnvelope {
    /// Robot's unique identifier (matches IntentMessage.robot_id)
    pub robot_id: String,
    /// Maximum linear speed (m/s)
    pub max_speed: f64,
    /// Maximum force exerted by any leg actuator (Newtons)
    pub max_force: f64,
    /// Minimum collision distance (meters)
    pub min_collision_distance: f64,
    /// Maximum tilt angle (degrees)
    pub max_tilt_degrees: f64,
    /// Maximum acceleration (m/s²)
    pub max_acceleration: f64,
    /// Whether the robot is permitted to operate in human-proximity zones
    pub human_proximity_allowed: bool,
    /// Maximum force exerted by the robotic arm (Newtons, 0 = no arm)
    #[serde(default)]
    pub max_arm_force: f64,
    /// Maximum torque per joint (N·m, 0 = unchecked)
    #[serde(default)]
    pub max_joint_torque: f64,
    /// Governance version (incremented on each update)
    pub version: u32,
}

impl SafetyEnvelope {
    /// Encode as JSON bytes for on-chain storage
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode from on-chain storage bytes
    pub fn decode(data: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// A batch attestation from a robot's reflex-tier controller, proving that
/// a sequence of reflex cycles maintained the declared safety envelope.
///
/// Unlike `IntentMessage` (which wraps a single auditable decision), this
/// wraps a *batch* of reflex cycles. The controller periodically hashes:
/// - Sensor readings for each cycle
/// - Safety invariant check results (pass/fail per invariant per cycle)
/// - The Merkle root of all cycle hashes
///
/// And submits the root on-chain. Post-hoc verifiable: if an incident occurs,
/// the full rosbag can be compared against the anchored Merkle root.
///
/// This does NOT gate reflexes in real time (physically impossible at 1000Hz).
/// It provides after-the-fact cryptographic proof that the safety envelope
/// was maintained — or evidence that it was violated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReflexBatchAttestation {
    /// Robot's unique identifier
    pub robot_id: String,
    /// Merkle root of all reflex cycle hashes in this batch
    pub merkle_root: String,
    /// Number of reflex cycles in this batch
    pub cycle_count: u32,
    /// Batch start timestamp (ms, controller clock)
    pub batch_start_timestamp: u64,
    /// Batch end timestamp (ms, controller clock)
    pub batch_end_timestamp: u64,
    /// Safety envelope version that was enforced during this batch
    pub envelope_version: u32,
    /// Whether all safety invariants were maintained (false = violation detected)
    pub all_invariants_maintained: bool,
    /// List of invariant names that were violated (empty if all maintained)
    pub violated_invariants: Vec<String>,
    /// Reference to the rosbag segment containing full reflex data
    pub rosbag_ref: String,
}

impl ReflexBatchAttestation {
    /// Encode as JSON bytes for embedding in AgentMessage.content
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode from AgentMessage.content bytes
    pub fn decode(content: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(content)?)
    }

    /// Create an AgentMessage wrapping this attestation
    pub fn into_agent_message(
        self,
        from: Vec<u8>,
        to: Vec<u8>,
        timestamp: u64,
    ) -> anyhow::Result<AgentMessage> {
        let content = self.encode()?;
        Ok(AgentMessage::new(from, to, content, timestamp))
    }

    /// Returns true if this attestation indicates a safety violation
    pub fn has_violation(&self) -> bool {
        !self.all_invariants_maintained || !self.violated_invariants.is_empty()
    }
}

/// The state of a robot's circuit breaker.
///
/// The circuit breaker is the enforcement layer: if a `ReflexBatchAttestation`
/// reveals a safety violation (or the Truth Market flags one), the breaker
/// trips. `plugin-ros2` checks the breaker before emitting any new
/// `IntentMessage`. When tripped, the robot enters safe-hold: reflexes keep
/// running (physics doesn't stop), but intent-tier decisions are locked.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CircuitBreakerState {
    /// Breaker is closed — robot can emit intent-tier messages normally
    Closed,
    /// Breaker is tripped — intent-tier locked, reflexes still running
    Tripped {
        /// Reason the breaker tripped
        reason: String,
        /// Timestamp (ms) when the breaker tripped
        tripped_at: u64,
        /// Reference to the attestation or verdict that caused the trip
        cause_ref: String,
    },
    /// Breaker was manually reset after resolution
    Reset {
        /// Timestamp (ms) when the breaker was reset
        reset_at: u64,
        /// Who authorized the reset (governance or operator address)
        reset_by: String,
    },
}

impl CircuitBreakerState {
    /// Returns true if the breaker is closed (intent-tier allowed)
    pub fn is_closed(&self) -> bool {
        matches!(self, CircuitBreakerState::Closed)
    }

    /// Returns true if the breaker is tripped (intent-tier locked)
    pub fn is_tripped(&self) -> bool {
        matches!(self, CircuitBreakerState::Tripped { .. })
    }
}

/// An action emitted by the consensus engine when a safety violation is
/// detected. The relayer consumes these and submits `TripBreaker`
/// transactions to the circuit-breaker contract on Juno.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreakerAction {
    /// Robot ID whose breaker should trip
    pub robot_id: String,
    /// Human-readable reason for the trip
    pub reason: String,
    /// Reference to the attestation, verdict, or batch that caused the trip
    pub cause_ref: String,
    /// Batch height where the violation was detected
    pub batch_height: u64,
    /// Timestamp (ms) when the action was emitted
    pub emitted_at: u64,
}

impl BreakerAction {
    /// Create a breaker action from a red gate verdict on a robot intent.
    pub fn from_red_verdict(
        robot_id: String,
        batch_height: u64,
        separation_score: f64,
    ) -> Self {
        Self {
            robot_id,
            reason: format!(
                "J-Lens gate red verdict (separation_score={:.3}) — intent blocked by coordination layer",
                separation_score
            ),
            cause_ref: format!("batch:{}:red-verdict", batch_height),
            batch_height,
            emitted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Create a breaker action from a reflex batch attestation violation.
    pub fn from_attestation_violation(
        robot_id: String,
        batch_height: u64,
        violated_invariants: &[String],
    ) -> Self {
        Self {
            robot_id,
            reason: format!(
                "ReflexBatchAttestation violation: {}",
                violated_invariants.join(", ")
            ),
            cause_ref: format!("batch:{}:attestation-violation", batch_height),
            batch_height,
            emitted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

/// Proof verification context attached to a message during coordination.
/// This is populated by the `ProofAwareGate` when it checks whether a
/// ZK proof verification result accompanies the intent.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProofContext {
    /// Whether a valid ZK proof was verified for this message
    pub proof_verified: bool,
    /// Optional proof hash for attestation reference
    pub proof_hash: Option<String>,
    /// Whether the reflex batch attestation showed any violations
    pub attestation_clean: Option<bool>,
    /// List of violated invariants (if attestation_clean is false)
    pub violated_invariants: Vec<String>,
}

impl ProofContext {
    /// Returns true if the proof context indicates a safety violation.
    pub fn has_violation(&self) -> bool {
        self.attestation_clean == Some(false) || !self.violated_invariants.is_empty()
    }
}

/// A batch of ordered messages — the block format for consensus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    /// Ordered messages in this batch
    pub messages: Vec<AgentMessage>,
    /// Hash of the previous batch (chain linkage)
    pub prev_hash: [u8; 32],
    /// Batch height (incrementing counter)
    pub height: u64,
    /// Unix timestamp (milliseconds) when batch was assembled
    pub timestamp: u64,
    /// J-Lens gate result for the entire batch
    pub gate_result: Option<GateResult>,
    /// Layer 6: evaluator attestations from multiple J-Lens operators
    #[serde(default)]
    pub eval_attestations: Vec<EvalAttestation>,
    /// Breaker actions emitted during this batch's consensus processing
    #[serde(default)]
    pub breaker_actions: Vec<BreakerAction>,
    /// Moultbook context fetched during this batch's consensus processing
    #[serde(default)]
    pub context_digest: Option<String>,
}

impl Batch {
    /// Compute the SHA-256 hash of this batch.
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hasher.finalize().into()
    }

    /// Create a new batch with the given messages and previous hash.
    pub fn new(
        messages: Vec<AgentMessage>,
        prev_hash: [u8; 32],
        height: u64,
        timestamp: u64,
    ) -> Self {
        Self {
            messages,
            prev_hash,
            height,
            timestamp,
            gate_result: None,
            eval_attestations: Vec::new(),
            breaker_actions: Vec::new(),
            context_digest: None,
        }
    }

    /// Attach a J-Lens gate result to the batch.
    pub fn with_gate_result(mut self, result: GateResult) -> Self {
        self.gate_result = Some(result);
        self
    }

    /// Attach breaker actions emitted during consensus processing.
    pub fn with_breaker_actions(mut self, actions: Vec<BreakerAction>) -> Self {
        self.breaker_actions = actions;
        self
    }

    /// Attach a moultbook context digest fetched during consensus processing.
    pub fn with_context_digest(mut self, digest: String) -> Self {
        self.context_digest = Some(digest);
        self
    }

    /// Check if any message in the batch has a red gate verdict.
    pub fn has_blocked_message(&self) -> bool {
        self.messages.iter().any(|m| {
            matches!(m.j_lens_gate, Some(GateVerdict::Red { .. }))
        })
    }

    /// Get the number of messages in the batch.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_hash_verification() {
        let msg = AgentMessage::new(
            vec![1; 32],
            vec![2; 32],
            b"hello world".to_vec(),
            1000,
        );
        assert!(msg.verify_hash());
    }

    #[test]
    fn test_message_tamper_detection() {
        let mut msg = AgentMessage::new(
            vec![1; 32],
            vec![2; 32],
            b"hello world".to_vec(),
            1000,
        );
        msg.content = b"tampered".to_vec();
        assert!(!msg.verify_hash());
    }

    #[test]
    fn test_broadcast_message() {
        let msg = AgentMessage::new(
            vec![1; 32],
            vec![],
            b"broadcast".to_vec(),
            1000,
        );
        assert!(msg.is_broadcast());
    }

    #[test]
    fn test_message_encode_decode() {
        let msg = AgentMessage::new(
            vec![1; 32],
            vec![2; 32],
            b"test payload".to_vec(),
            12345,
        )
        .with_gate(GateVerdict::Green)
        .with_proposal_ref(42);

        let encoded = msg.encode().unwrap();
        let decoded = AgentMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.content, msg.content);
        assert_eq!(decoded.timestamp, msg.timestamp);
        assert_eq!(decoded.j_lens_gate, Some(GateVerdict::Green));
        assert_eq!(decoded.proposal_ref, Some(42));
    }

    #[test]
    fn test_batch_hash_chain() {
        let msg1 = AgentMessage::new(vec![1; 32], vec![], b"msg1".to_vec(), 1000);
        let msg2 = AgentMessage::new(vec![2; 32], vec![], b"msg2".to_vec(), 2000);

        let batch1 = Batch::new(vec![msg1], [0u8; 32], 1, 1500);
        let hash1 = batch1.hash();

        let batch2 = Batch::new(vec![msg2], hash1, 2, 2500);
        assert_eq!(batch2.prev_hash, hash1);
    }

    #[test]
    fn test_batch_blocked_message_detection() {
        let msg_clean = AgentMessage::new(vec![1; 32], vec![], b"clean".to_vec(), 1000)
            .with_gate(GateVerdict::Green);
        let msg_blocked = AgentMessage::new(vec![2; 32], vec![], b"bad".to_vec(), 2000)
            .with_gate(GateVerdict::Red { separation_score: 0.95 });

        let batch = Batch::new(vec![msg_clean, msg_blocked], [0u8; 32], 1, 3000);
        assert!(batch.has_blocked_message());
    }

    #[test]
    fn test_batch_no_blocked_messages() {
        let msg1 = AgentMessage::new(vec![1; 32], vec![], b"clean1".to_vec(), 1000)
            .with_gate(GateVerdict::Green);
        let msg2 = AgentMessage::new(vec![2; 32], vec![], b"clean2".to_vec(), 2000)
            .with_gate(GateVerdict::Yellow { separation_score: 0.3 });

        let batch = Batch::new(vec![msg1, msg2], [0u8; 32], 1, 3000);
        assert!(!batch.has_blocked_message());
    }

    #[test]
    fn test_task_request_encode_decode() {
        let req = TaskRequest {
            agent_id: 42,
            input_hash: "abc123".to_string(),
            execution_tier: "akash".to_string(),
            plugin_hint: Some("plugin-compute-akash".to_string()),
            proposal_id: Some(7),
        };
        let encoded = req.encode().unwrap();
        let decoded = TaskRequest::decode(&encoded).unwrap();
        assert_eq!(decoded.agent_id, 42);
        assert_eq!(decoded.execution_tier, "akash");
        assert_eq!(decoded.plugin_hint, Some("plugin-compute-akash".to_string()));
        assert_eq!(decoded.proposal_id, Some(7));
    }

    #[test]
    fn test_task_request_in_agent_message() {
        let req = TaskRequest {
            agent_id: 1,
            input_hash: "deadbeef".to_string(),
            execution_tier: "local".to_string(),
            plugin_hint: None,
            proposal_id: None,
        };
        let content = req.encode().unwrap();
        let msg = AgentMessage::new(vec![1; 32], vec![], content, 1000);
        assert!(msg.verify_hash());

        let decoded_req = TaskRequest::decode(&msg.content).unwrap();
        assert_eq!(decoded_req.agent_id, 1);
        assert_eq!(decoded_req.input_hash, "deadbeef");
    }

    #[test]
    fn test_eval_attestation_serialization() {
        let attestation = EvalAttestation {
            operator_pubkey: vec![0xAB; 32],
            verdict: GateVerdict::Green,
            batch_height: 4041,
            signature: vec![0xCD; 64],
        };
        let json = serde_json::to_string(&attestation).unwrap();
        let decoded: EvalAttestation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.batch_height, 4041);
        assert_eq!(decoded.verdict, GateVerdict::Green);
        assert_eq!(decoded.operator_pubkey, vec![0xAB; 32]);
    }

    #[test]
    fn test_batch_with_eval_attestations() {
        let msg = AgentMessage::new(vec![1; 32], vec![], b"test".to_vec(), 1000);
        let mut batch = Batch::new(vec![msg], [0u8; 32], 1, 2000);
        batch.eval_attestations.push(EvalAttestation {
            operator_pubkey: vec![0xAB; 32],
            verdict: GateVerdict::Green,
            batch_height: 1,
            signature: vec![0xCD; 64],
        });
        assert_eq!(batch.eval_attestations.len(), 1);
        let hash = batch.hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_intent_message_encode_decode() {
        let intent = IntentMessage {
            robot_id: "robot-001".to_string(),
            action: "navigate".to_string(),
            params: serde_json::json!({"destination": [37.7749, -122.4194], "speed": 1.2}),
            sensor_snapshot_hash: "abc123def456".to_string(),
            controller_timestamp: 1723910400000,
            rationale: Some("Route through sector B, avoiding obstacle at waypoint 3".to_string()),
            execution_proof_ref: Some("rosbag_2026_08_17_001".to_string()),
        };
        let encoded = intent.encode().unwrap();
        let decoded = IntentMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.robot_id, "robot-001");
        assert_eq!(decoded.action, "navigate");
        assert_eq!(decoded.sensor_snapshot_hash, "abc123def456");
        assert_eq!(decoded.controller_timestamp, 1723910400000);
        assert!(decoded.rationale.is_some());
        assert!(decoded.execution_proof_ref.is_some());
    }

    #[test]
    fn test_intent_message_into_agent_message() {
        let intent = IntentMessage {
            robot_id: "combat-bot-7".to_string(),
            action: "engage".to_string(),
            params: serde_json::json!({"target_id": "T-0042", "weapon": "primary"}),
            sensor_snapshot_hash: "deadbeef".to_string(),
            controller_timestamp: 1723910500000,
            rationale: None,
            execution_proof_ref: None,
        };
        let from = vec![0xAB; 32];
        let to = vec![0xCD; 32];
        let msg = intent.into_agent_message(from.clone(), to.clone(), 1723910600000).unwrap();

        assert!(msg.verify_hash());
        assert_eq!(msg.from, from);
        assert_eq!(msg.to, to);

        // Decode the content back into an IntentMessage
        let decoded_intent = IntentMessage::decode(&msg.content).unwrap();
        assert_eq!(decoded_intent.robot_id, "combat-bot-7");
        assert_eq!(decoded_intent.action, "engage");
        assert!(decoded_intent.rationale.is_none());
    }

    #[test]
    fn test_intent_message_minimal() {
        let intent = IntentMessage {
            robot_id: "delivery-bot-3".to_string(),
            action: "pick_tool".to_string(),
            params: serde_json::json!({}),
            sensor_snapshot_hash: "minimal_hash".to_string(),
            controller_timestamp: 1000,
            rationale: None,
            execution_proof_ref: None,
        };
        let encoded = intent.encode().unwrap();
        let decoded = IntentMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.action, "pick_tool");
        assert!(decoded.rationale.is_none());
        assert!(decoded.execution_proof_ref.is_none());
    }

    #[test]
    fn test_intent_outcome_serialization() {
        let outcome = IntentOutcome {
            intent_hash: [0xAA; 32],
            verdict: GateVerdict::Green,
            consensus_ratio: 1.0,
            action_matched: true,
            batch_height: 42,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let decoded: IntentOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.batch_height, 42);
        assert_eq!(decoded.verdict, GateVerdict::Green);
        assert!(decoded.action_matched);
        assert_eq!(decoded.consensus_ratio, 1.0);
    }

    #[test]
    fn test_safety_envelope_encode_decode() {
        let envelope = SafetyEnvelope {
            robot_id: "surgical-bot-1".to_string(),
            max_speed: 0.5,
            max_force: 10.0,
            min_collision_distance: 0.05,
            max_tilt_degrees: 15.0,
            max_acceleration: 2.0,
            human_proximity_allowed: true,
            max_arm_force: 0.0,
            max_joint_torque: 0.0,
            version: 1,
        };
        let encoded = envelope.encode().unwrap();
        let decoded = SafetyEnvelope::decode(&encoded).unwrap();
        assert_eq!(decoded.robot_id, "surgical-bot-1");
        assert_eq!(decoded.max_speed, 0.5);
        assert_eq!(decoded.max_force, 10.0);
        assert_eq!(decoded.min_collision_distance, 0.05);
        assert_eq!(decoded.max_tilt_degrees, 15.0);
        assert!(decoded.human_proximity_allowed);
        assert_eq!(decoded.version, 1);
    }

    #[test]
    fn test_safety_envelope_versioning() {
        let v1 = SafetyEnvelope {
            robot_id: "delivery-bot-3".to_string(),
            max_speed: 5.0,
            max_force: 50.0,
            min_collision_distance: 0.3,
            max_tilt_degrees: 30.0,
            max_acceleration: 3.0,
            human_proximity_allowed: true,
            max_arm_force: 0.0,
            max_joint_torque: 0.0,
            version: 1,
        };
        let v2 = SafetyEnvelope {
            robot_id: "delivery-bot-3".to_string(),
            max_speed: 3.0,  // tightened
            max_force: 50.0,
            min_collision_distance: 0.5,  // tightened
            max_tilt_degrees: 30.0,
            max_acceleration: 3.0,
            human_proximity_allowed: true,
            max_arm_force: 0.0,
            max_joint_torque: 0.0,
            version: 2,
        };
        assert_ne!(v1.version, v2.version);
        assert_ne!(v1.max_speed, v2.max_speed);
        assert_ne!(v1.min_collision_distance, v2.min_collision_distance);
    }

    #[test]
    fn test_reflex_batch_attestation_clean() {
        let attestation = ReflexBatchAttestation {
            robot_id: "combat-bot-7".to_string(),
            merkle_root: "a3f2e1d4b5c6...".to_string(),
            cycle_count: 1000,
            batch_start_timestamp: 1723910400000,
            batch_end_timestamp: 1723910401000,
            envelope_version: 1,
            all_invariants_maintained: true,
            violated_invariants: vec![],
            rosbag_ref: "rosbag_2026_08_17_001".to_string(),
        };
        assert!(!attestation.has_violation());
        let encoded = attestation.encode().unwrap();
        let decoded = ReflexBatchAttestation::decode(&encoded).unwrap();
        assert_eq!(decoded.cycle_count, 1000);
        assert!(decoded.all_invariants_maintained);
        assert!(decoded.violated_invariants.is_empty());
    }

    #[test]
    fn test_reflex_batch_attestation_violation() {
        let attestation = ReflexBatchAttestation {
            robot_id: "delivery-bot-3".to_string(),
            merkle_root: "b4c5d6e7f8...".to_string(),
            cycle_count: 500,
            batch_start_timestamp: 1723910500000,
            batch_end_timestamp: 1723910500500,
            envelope_version: 2,
            all_invariants_maintained: false,
            violated_invariants: vec!["min_collision_distance".to_string(), "max_speed".to_string()],
            rosbag_ref: "rosbag_2026_08_17_002".to_string(),
        };
        assert!(attestation.has_violation());
        let encoded = attestation.encode().unwrap();
        let decoded = ReflexBatchAttestation::decode(&encoded).unwrap();
        assert!(!decoded.all_invariants_maintained);
        assert_eq!(decoded.violated_invariants.len(), 2);
        assert_eq!(decoded.violated_invariants[0], "min_collision_distance");
    }

    #[test]
    fn test_reflex_batch_attestation_into_agent_message() {
        let attestation = ReflexBatchAttestation {
            robot_id: "surgical-bot-1".to_string(),
            merkle_root: "c5d6e7f8a9...".to_string(),
            cycle_count: 2000,
            batch_start_timestamp: 1723910600000,
            batch_end_timestamp: 1723910602000,
            envelope_version: 1,
            all_invariants_maintained: true,
            violated_invariants: vec![],
            rosbag_ref: "rosbag_2026_08_17_003".to_string(),
        };
        let from = vec![0xAB; 32];
        let to = vec![0xCD; 32];
        let msg = attestation.into_agent_message(from.clone(), to.clone(), 1723910603000).unwrap();
        assert!(msg.verify_hash());
        assert_eq!(msg.from, from);
        // Decode content back
        let decoded = ReflexBatchAttestation::decode(&msg.content).unwrap();
        assert_eq!(decoded.robot_id, "surgical-bot-1");
        assert_eq!(decoded.cycle_count, 2000);
    }

    #[test]
    fn test_circuit_breaker_closed() {
        let state = CircuitBreakerState::Closed;
        assert!(state.is_closed());
        assert!(!state.is_tripped());
    }

    #[test]
    fn test_circuit_breaker_tripped() {
        let state = CircuitBreakerState::Tripped {
            reason: "min_collision_distance violated in batch 42".to_string(),
            tripped_at: 1723910700000,
            cause_ref: "rosbag_2026_08_17_002".to_string(),
        };
        assert!(!state.is_closed());
        assert!(state.is_tripped());
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let state = CircuitBreakerState::Reset {
            reset_at: 1723910800000,
            reset_by: "juno1governance...".to_string(),
        };
        assert!(!state.is_closed());
        assert!(!state.is_tripped());
    }

    #[test]
    fn test_circuit_breaker_serialization() {
        let tripped = CircuitBreakerState::Tripped {
            reason: "max_speed exceeded".to_string(),
            tripped_at: 1723910700000,
            cause_ref: "attestation_42".to_string(),
        };
        let json = serde_json::to_string(&tripped).unwrap();
        let decoded: CircuitBreakerState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, tripped);
        assert!(decoded.is_tripped());
    }
}
