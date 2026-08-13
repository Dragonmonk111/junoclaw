//! Coordination node — the main entry point for an agent joining the network.
//!
//! A coordination node wraps the P2P network and provides a simple API:
//! - `join()` — connect to the mesh
//! - `send(msg)` — send a message
//! - `recv()` — receive the next message
//! - `settle(batch_id)` — submit to Juno (Phase 3+)

use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use crate::gate::JLensGate;
use crate::message::{AgentMessage, GateVerdict};
use crate::network::{CoordinationConfig, CoordinationNetwork};

/// A coordination node — an agent's presence on the coordination network.
pub struct CoordinationNode {
    /// The underlying P2P network
    network: CoordinationNetwork,
    /// J-Lens truth gate (optional — wired in Phase 4)
    gate: Option<JLensGate>,
    /// Node identity label (for logging)
    label: String,
}

impl CoordinationNode {
    /// Create a new coordination node with the given configuration.
    pub fn new(label: impl Into<String>, config: CoordinationConfig) -> Result<Self> {
        let network = CoordinationNetwork::new(config)?;
        Ok(Self {
            network,
            gate: None,
            label: label.into(),
        })
    }

    /// Create a new coordination node with a seed-derived identity.
    pub fn from_seed(
        label: impl Into<String>,
        seed: u64,
        config: CoordinationConfig,
    ) -> Result<Self> {
        let network = CoordinationNetwork::from_seed(seed, config)?;
        Ok(Self {
            network,
            gate: None,
            label: label.into(),
        })
    }

    /// Attach a J-Lens truth gate to this node.
    pub fn with_gate(mut self, gate: JLensGate) -> Self {
        self.gate = Some(gate);
        self
    }

    /// Get this node's public key (ed25519, 32 bytes).
    pub fn public_key(&self) -> &[u8] {
        self.network.public_key()
    }

    /// Get this node's public key as hex.
    pub fn public_key_hex(&self) -> String {
        self.network.public_key_hex()
    }

    /// Get the node label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Send a message to a specific peer.
    ///
    /// If a J-Lens gate is attached, the message content is audited first.
    /// Red-gated messages are blocked (not sent).
    /// Yellow-gated messages are sent with the warning attached.
    /// Green-gated messages are sent normally.
    pub async fn send(&self, to: Vec<u8>, content: Vec<u8>) -> Result<SendResult> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut msg = AgentMessage::new(
            self.network.public_key().to_vec(),
            to,
            content,
            timestamp,
        );

        // If J-Lens gate is attached, audit the message
        if let Some(gate) = &self.gate {
            let verdict = gate.audit(&msg.content).await;
            match &verdict {
                GateVerdict::Red { .. } => {
                    info!(
                        "Node {}: message blocked by J-Lens gate (red)",
                        self.label
                    );
                    return Ok(SendResult::Blocked);
                }
                _ => {
                    msg = msg.with_gate(verdict);
                }
            }
        }

        self.network.send(msg).await?;
        Ok(SendResult::Sent)
    }

    /// Broadcast a message to all peers.
    pub async fn broadcast(&self, content: Vec<u8>) -> Result<SendResult> {
        self.send(vec![], content).await
    }

    /// Receive the next incoming message.
    pub async fn recv(&self) -> Option<AgentMessage> {
        self.network.recv().await
    }

    /// This node's public key, cloned as an owned `Vec<u8>` — useful for
    /// building `AgentMessage`s from a detached sender handle.
    pub fn public_key_vec(&self) -> Vec<u8> {
        self.network.public_key().to_vec()
    }

    /// Get a cloneable sender handle for queuing outgoing messages, usable
    /// after `run()` has consumed this node (e.g. when `run()` is spawned
    /// into a background task).
    pub fn sender_handle(&self) -> tokio::sync::mpsc::Sender<AgentMessage> {
        self.network.sender_handle()
    }

    /// Get a shared handle to the incoming-message receiver, usable after
    /// `run()` has consumed this node.
    pub fn recv_handle(
        &self,
    ) -> std::sync::Arc<tokio::sync::RwLock<Option<tokio::sync::mpsc::Receiver<AgentMessage>>>>
    {
        self.network.recv_handle()
    }

    /// Run the coordination node — starts the P2P network loop.
    pub async fn run(self) -> Result<()> {
        info!(
            "Coordination node '{}' starting (pk={})",
            self.label,
            self.public_key_hex()
        );
        self.network.run().await
    }
}

/// Result of attempting to send a message.
#[derive(Debug, PartialEq, Eq)]
pub enum SendResult {
    /// Message was sent successfully
    Sent,
    /// Message was blocked by the J-Lens truth gate
    Blocked,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = CoordinationNode::new(
            "test-node",
            CoordinationConfig {
                listen_addr: "127.0.0.1:5001".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(node.label(), "test-node");
        assert_eq!(node.public_key().len(), 32);
    }

    #[test]
    fn test_node_from_seed() {
        let node1 = CoordinationNode::from_seed(
            "node1",
            100,
            CoordinationConfig::default(),
        )
        .unwrap();
        let node2 = CoordinationNode::from_seed(
            "node2",
            100,
            CoordinationConfig::default(),
        )
        .unwrap();
        assert_eq!(node1.public_key(), node2.public_key());
    }
}
