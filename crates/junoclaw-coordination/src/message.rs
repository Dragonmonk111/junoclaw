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
        }
    }

    /// Attach a J-Lens gate result to the batch.
    pub fn with_gate_result(mut self, result: GateResult) -> Self {
        self.gate_result = Some(result);
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
}
