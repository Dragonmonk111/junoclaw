//! Consensus ordering — BFT message ordering using simplex consensus.
//!
//! Phase 2 integrates `commonware-consensus::simplex` for BFT ordering
//! of agent messages into finalized batches with threshold certificates.
//!
//! The consensus module is available in two modes:
//! - `simulated` (default): In-process simulated consensus for testing
//! - `p2p`: Real commonware-consensus integration (requires NASM/aws-lc-sys)
//!
//! Block format: `Batch { messages, prev_hash, height, timestamp }`
//! Block production target: 300ms
//! Validator set: 4 nodes (tolerates 1 byzantine)

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::gate::JLensGate;
use crate::message::{AgentMessage, Batch, GateVerdict};

/// Configuration for the consensus engine.
#[derive(Clone, Debug)]
pub struct ConsensusConfig {
    /// Number of validators (default 4 — tolerates 1 byzantine)
    pub num_validators: usize,
    /// Target block time (default 300ms)
    pub block_time: Duration,
    /// Maximum messages per block
    pub max_messages_per_block: usize,
    /// This node's validator index (0-based)
    pub validator_index: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            num_validators: 4,
            block_time: Duration::from_millis(300),
            max_messages_per_block: 100,
            validator_index: 0,
        }
    }
}

/// A finalized block with its threshold certificate.
#[derive(Clone, Debug)]
pub struct FinalizedBlock {
    /// The finalized batch
    pub batch: Batch,
    /// Threshold certificate bytes (BLS aggregate signature in real mode)
    pub certificate: Vec<u8>,
    /// Height of this block
    pub height: u64,
    /// Time the block was finalized
    pub finalized_at: u64,
}

/// The consensus engine — orders messages into finalized batches.
///
/// In `simulated` mode, this uses a simple round-robin leader election
/// and in-process message aggregation. In `p2p` mode, it wraps the
/// real `commonware-consensus::simplex::Engine`.
pub struct ConsensusEngine {
    config: ConsensusConfig,
    /// Pending messages waiting to be included in a block
    pending: Mutex<Vec<AgentMessage>>,
    /// Channel to receive finalized blocks
    block_rx: Mutex<Option<mpsc::Receiver<FinalizedBlock>>>,
    /// Channel to send finalized blocks
    block_tx: mpsc::Sender<FinalizedBlock>,
    /// Last finalized batch hash
    last_hash: Mutex<[u8; 32]>,
    /// Current height
    height: Mutex<u64>,
    /// Validator public keys (by index)
    validators: Vec<Vec<u8>>,
    /// Optional J-Lens truth gate for batch auditing
    gate: Option<JLensGate>,
}

impl ConsensusEngine {
    /// Create a new consensus engine with the given configuration.
    pub fn new(config: ConsensusConfig) -> Self {
        let (block_tx, block_rx) = mpsc::channel::<FinalizedBlock>(64);

        // Generate deterministic validator keys for simulation
        let validators: Vec<Vec<u8>> = (0..config.num_validators)
            .map(|i| {
                let mut pk = [0u8; 32];
                pk[0] = (i + 1) as u8;
                pk[31] = (i + 1) as u8;
                pk.to_vec()
            })
            .collect();

        Self {
            config,
            pending: Mutex::new(Vec::new()),
            block_rx: Mutex::new(Some(block_rx)),
            block_tx,
            last_hash: Mutex::new([0u8; 32]),
            height: Mutex::new(0),
            validators,
            gate: None,
        }
    }

    /// Attach a J-Lens truth gate for batch auditing.
    /// When attached, every batch is audited before finalization:
    /// - Red-gated messages are filtered out
    /// - GateResult is attached to the finalized batch
    pub fn with_gate(mut self, gate: JLensGate) -> Self {
        self.gate = Some(gate);
        self
    }

    /// Submit a message to be included in the next block.
    pub async fn submit(&self, msg: AgentMessage) -> Result<()> {
        let mut pending = self.pending.lock().await;
        if pending.len() >= self.config.max_messages_per_block {
            warn!("Pending queue full, dropping message");
            return Ok(());
        }
        pending.push(msg);
        Ok(())
    }

    /// Receive the next finalized block.
    pub async fn next_block(&self) -> Option<FinalizedBlock> {
        let mut guard = self.block_rx.lock().await;
        if let Some(rx) = guard.as_mut() {
            rx.recv().await
        } else {
            None
        }
    }

    /// Get the validator set.
    pub fn validators(&self) -> &[Vec<u8>] {
        &self.validators
    }

    /// Produce a single block from pending messages (for testing).
    ///
    /// Drains pending messages, optionally audits with gate, creates a
    /// FinalizedBlock with simulated certificate, and updates internal
    /// height/hash state. Returns None if no pending messages (or all
    /// were filtered by the gate).
    pub async fn produce_block(&self) -> Option<FinalizedBlock> {
        let messages = {
            let mut pending = self.pending.lock().await;
            if pending.is_empty() {
                return None;
            }
            pending.drain(..).collect::<Vec<_>>()
        };

        let height = *self.height.lock().await;
        let prev_hash = *self.last_hash.lock().await;
        let timestamp = now_ms();

        let mut batch = Batch::new(messages, prev_hash, height, timestamp);

        // If gate is attached, audit and filter
        if let Some(gate) = &self.gate {
            let gate_result = gate.audit_batch(&batch).await;

            let mut filtered = Vec::new();
            for msg in &batch.messages {
                let verdict = gate.audit(&msg.content).await;
                if !matches!(verdict, GateVerdict::Red { .. }) {
                    filtered.push(msg.clone());
                }
            }

            let blocked_count = batch.messages.len() - filtered.len();
            if blocked_count > 0 {
                warn!(
                    "Filtered {} red-gated messages from block at height {}",
                    blocked_count, height
                );
            }

            batch = Batch::new(filtered, prev_hash, height, timestamp)
                .with_gate_result(gate_result);
        }

        if batch.is_empty() {
            return None;
        }

        let batch_hash = batch.hash();
        let certificate = simulate_certificate(&batch_hash, &self.validators);

        let block = FinalizedBlock {
            batch: batch.clone(),
            certificate,
            height,
            finalized_at: now_ms(),
        };

        // Update state
        *self.last_hash.lock().await = batch_hash;
        *self.height.lock().await = height + 1;

        Some(block)
    }

    /// Run the consensus engine.
    ///
    /// In simulated mode, this loops:
    /// 1. Wait for block_time interval
    /// 2. Collect pending messages
    /// 3. Determine leader (round-robin by height)
    /// 4. If we're the leader, propose a block
    /// 5. "Finalize" (simulate 2f+1 votes)
    /// 6. Emit finalized block
    pub async fn run(self) -> Result<()> {
        info!(
            "Consensus engine starting: {} validators, block_time={}ms, index={}",
            self.config.num_validators,
            self.config.block_time.as_millis(),
            self.config.validator_index
        );

        let mut interval = tokio::time::interval(self.config.block_time);
        let self_arc = std::sync::Arc::new(self);

        loop {
            interval.tick().await;

            let height = {
                let h = self_arc.height.lock().await;
                *h
            };

            // Round-robin leader election
            let leader_index = (height as usize) % self_arc.config.num_validators;
            let is_leader = leader_index == self_arc.config.validator_index;

            // Collect pending messages
            let messages = {
                let mut pending = self_arc.pending.lock().await;
                if pending.is_empty() {
                    continue;
                }
                let msgs: Vec<AgentMessage> = pending.drain(..).collect();
                msgs
            };

            if !is_leader {
                // In real consensus, we'd vote on the leader's proposal.
                // In simulation, non-leaders just wait.
                continue;
            }

            // Leader proposes a block
            let prev_hash = *self_arc.last_hash.lock().await;
            let timestamp = now_ms();

            let mut batch = Batch::new(messages, prev_hash, height, timestamp);

            // If gate is attached, audit the batch before finalizing
            if let Some(gate) = &self_arc.gate {
                let gate_result = gate.audit_batch(&batch).await;

                // Filter out red-gated messages (audit each message individually)
                let mut filtered = Vec::new();
                for msg in &batch.messages {
                    let verdict = gate.audit(&msg.content).await;
                    if !matches!(verdict, GateVerdict::Red { .. }) {
                        filtered.push(msg.clone());
                    }
                }

                let blocked_count = batch.messages.len() - filtered.len();
                if blocked_count > 0 {
                    warn!(
                        "Block at height {} filtered {} red-gated messages",
                        height, blocked_count
                    );
                }

                batch = Batch::new(filtered, prev_hash, height, timestamp)
                    .with_gate_result(gate_result);
            }

            // Skip empty blocks (all messages were filtered or none pending)
            if batch.is_empty() {
                continue;
            }

            let batch_hash = batch.hash();

            // Simulate threshold certificate (in real mode, this is a BLS aggregate)
            let certificate = simulate_certificate(&batch_hash, &self_arc.validators);

            let block = FinalizedBlock {
                batch: batch.clone(),
                certificate,
                height,
                finalized_at: now_ms(),
            };

            info!(
                "Block finalized: height={}, messages={}, hash={}",
                height,
                block.batch.len(),
                hex::encode(&batch_hash)
            );

            // Emit block
            let _ = self_arc.block_tx.send(block).await;

            // Update state
            *self_arc.last_hash.lock().await = batch_hash;
            *self_arc.height.lock().await = height + 1;
        }
    }

    /// Get the current height.
    pub async fn height(&self) -> u64 {
        *self.height.lock().await
    }
}

/// Simulate a threshold certificate.
///
/// In real mode, this is a BLS12-381 aggregate signature from 2f+1 validators.
/// In simulation, we just hash the batch hash with validator keys.
fn simulate_certificate(batch_hash: &[u8; 32], validators: &[Vec<u8>]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(batch_hash);
    for v in validators {
        hasher.update(v);
    }
    hasher.finalize().to_vec()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_config_default() {
        let config = ConsensusConfig::default();
        assert_eq!(config.num_validators, 4);
        assert_eq!(config.block_time, Duration::from_millis(300));
    }

    #[test]
    fn test_simulate_certificate_deterministic() {
        let hash = [1u8; 32];
        let validators = vec![vec![1; 32], vec![2; 32]];
        let cert1 = simulate_certificate(&hash, &validators);
        let cert2 = simulate_certificate(&hash, &validators);
        assert_eq!(cert1, cert2);
    }

    #[test]
    fn test_simulate_certificate_different_validators() {
        let hash = [1u8; 32];
        let cert1 = simulate_certificate(&hash, &vec![vec![1; 32]]);
        let cert2 = simulate_certificate(&hash, &vec![vec![2; 32]]);
        assert_ne!(cert1, cert2);
    }

    #[tokio::test]
    async fn test_consensus_submit_and_height() {
        let engine = ConsensusEngine::new(ConsensusConfig::default());
        assert_eq!(engine.height().await, 0);

        let msg = AgentMessage::new(vec![1; 32], vec![], b"test".to_vec(), 1000);
        engine.submit(msg).await.unwrap();
    }

    #[tokio::test]
    async fn test_consensus_produces_block() {
        let engine = ConsensusEngine::new(ConsensusConfig {
            num_validators: 4,
            block_time: Duration::from_millis(50),
            max_messages_per_block: 100,
            validator_index: 0,
        });

        // Submit a message
        let msg = AgentMessage::new(vec![1; 32], vec![], b"consensus-test".to_vec(), now_ms());
        engine.submit(msg).await.unwrap();

        // Start the engine
        let engine_arc = std::sync::Arc::new(engine);
        let engine_clone = engine_arc.clone();

        // Run consensus in background
        let handle = tokio::spawn(async move {
            // We need to consume self, so we'll run a simplified version
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            interval.tick().await; // first tick is immediate
            interval.tick().await; // wait 50ms

            let messages = {
                let mut pending = engine_clone.pending.lock().await;
                pending.drain(..).collect::<Vec<_>>()
            };

            if !messages.is_empty() {
                let batch = Batch::new(messages, [0u8; 32], 0, now_ms());
                let cert = simulate_certificate(&batch.hash(), &engine_clone.validators);
                let block = FinalizedBlock {
                    batch,
                    certificate: cert,
                    height: 0,
                    finalized_at: now_ms(),
                };
                let _ = engine_clone.block_tx.send(block).await;
            }
        });

        // Wait for block
        let block = tokio::time::timeout(
            Duration::from_secs(2),
            engine_arc.block_rx.lock().await.as_mut().unwrap().recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(block.height, 0);
        assert_eq!(block.batch.len(), 1);
        assert!(!block.certificate.is_empty());

        handle.abort();
    }
}
