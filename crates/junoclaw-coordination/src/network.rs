//! Coordination network — P2P mesh using commonware-p2p.
//!
//! Each coordination node joins the mesh with an ed25519 identity.
//! Messages are sent over authenticated, encrypted P2P channels.
//! The network does NOT order messages — that's consensus (Phase 2).
//!
//! Transport: `commonware_p2p::authenticated::lookup` — a "known addresses"
//! P2P scheme where each peer's public key and socket address are pre-configured.
//! This matches our 4-validator testnet setup perfectly.
//!
//! Architecture:
//! - User calls `send()` / `recv()` on `&self` (via `CoordinationNode`)
//! - `run(self)` consumes the network and starts the commonware runtime
//! - The commonware runtime runs in `spawn_blocking` to avoid nested tokio runtimes
//! - Inside the runtime, two bridge tasks connect P2P <-> user channels:
//!   - send-bridge: `send_rx` (user) -> `p2p_sender` (P2P)
//!   - recv-bridge: `p2p_receiver` (P2P) -> `recv_tx` (user)

use anyhow::Result;
use commonware_cryptography::{ed25519, Signer};
use commonware_math::algebra::Random;
use commonware_p2p::{
    authenticated::lookup::{self, Network as P2pNetwork},
    Address, AddressableManager, Recipients, Sender as P2pSender, Receiver as P2pReceiver,
};
use commonware_runtime::{
    tokio::{Runner as TokioRunner, Config as RuntimeConfig},
    IoBuf, Quota, Runner, Spawner, Supervisor,
};
use commonware_utils::{NZU32, ordered::Map};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::message::AgentMessage;

/// Configuration for a coordination network node.
#[derive(Clone, Debug)]
pub struct CoordinationConfig {
    /// This node's listen address (e.g. "127.0.0.1:4001")
    pub listen_addr: String,
    /// Known peers: list of (public_key_hex, socket_addr) tuples.
    /// All peers must know all other peers for the authenticated lookup mesh.
    pub peers: Vec<(String, String)>,
    /// Max message size in bytes (default 1MB)
    pub max_message_size: usize,
    /// Channel capacity for incoming messages
    pub recv_buffer: usize,
    /// Application namespace for P2P cryptographic isolation
    pub namespace: Vec<u8>,
}

impl Default for CoordinationConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:4001".to_string(),
            peers: vec![],
            max_message_size: 1_048_576,
            recv_buffer: 1024,
            namespace: b"junoclaw-coordination-v1".to_vec(),
        }
    }
}

/// The coordination network — a P2P mesh of agent nodes.
///
/// Provides authenticated, encrypted peer-to-peer connections
/// using commonware-p2p's authenticated lookup transport.
///
/// Channel layout:
/// - `send_tx` / `send_rx`: user writes messages in, P2P send-bridge reads out
/// - `recv_tx` / `recv_rx`: P2P recv-bridge writes messages in, user reads out
/// - `send_tx` stays on the struct for `&self` access via `send()`
/// - `recv_rx` stays on the struct (behind Arc<RwLock>) for `&self` access via `recv()`
/// - `send_rx` and `recv_tx` are `Option`, taken by `run()` into the P2P runtime
pub struct CoordinationNetwork {
    /// This node's signing key
    signer: ed25519::PrivateKey,
    /// This node's public key (32 bytes)
    public_key: Vec<u8>,
    /// Network configuration
    config: CoordinationConfig,
    /// User-facing send channel (stays on struct for `send()`)
    send_tx: mpsc::Sender<AgentMessage>,
    /// Internal: receiver for outgoing messages (taken by `run()`)
    send_rx: Option<mpsc::Receiver<AgentMessage>>,
    /// User-facing receive channel (stays on struct for `recv()`)
    recv_rx: Arc<RwLock<Option<mpsc::Receiver<AgentMessage>>>>,
    /// Internal: sender for incoming messages (taken by `run()`)
    recv_tx: Option<mpsc::Sender<AgentMessage>>,
}

impl CoordinationNetwork {
    /// Create a new coordination network with a random identity.
    pub fn new(config: CoordinationConfig) -> Result<Self> {
        let signer = ed25519::PrivateKey::random(rand::rng());
        let public_key = signer.public_key().to_vec();
        let (send_tx, send_rx) = mpsc::channel::<AgentMessage>(1024);
        let (recv_tx, recv_rx) = mpsc::channel::<AgentMessage>(config.recv_buffer);

        Ok(Self {
            signer,
            public_key,
            config,
            send_tx,
            send_rx: Some(send_rx),
            recv_rx: Arc::new(RwLock::new(Some(recv_rx))),
            recv_tx: Some(recv_tx),
        })
    }

    /// Create a new coordination network with a seed-derived identity.
    pub fn from_seed(seed: u64, config: CoordinationConfig) -> Result<Self> {
        let mut rng = StdRng::seed_from_u64(seed);
        let signer = ed25519::PrivateKey::random(&mut rng);
        let public_key = signer.public_key().to_vec();
        let (send_tx, send_rx) = mpsc::channel::<AgentMessage>(1024);
        let (recv_tx, recv_rx) = mpsc::channel::<AgentMessage>(config.recv_buffer);

        Ok(Self {
            signer,
            public_key,
            config,
            send_tx,
            send_rx: Some(send_rx),
            recv_rx: Arc::new(RwLock::new(Some(recv_rx))),
            recv_tx: Some(recv_tx),
        })
    }

    /// Get this node's public key.
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Get this node's public key as hex string.
    pub fn public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }

    /// Send a message to the mesh (broadcast if `msg.to` is empty, targeted otherwise).
    ///
    /// The message is queued on the internal send channel. The P2P send-bridge
    /// (running inside `run()`) picks it up and sends it over the authenticated mesh.
    pub async fn send(&self, msg: AgentMessage) -> Result<()> {
        self.send_tx.send(msg).await?;
        Ok(())
    }

    /// Receive the next incoming message from the mesh.
    ///
    /// Returns `None` if the network has been shut down.
    pub async fn recv(&self) -> Option<AgentMessage> {
        let mut guard = self.recv_rx.write().await;
        if let Some(rx) = guard.as_mut() {
            rx.recv().await
        } else {
            None
        }
    }

    /// Get the number of configured peers (not live-connected count).
    pub async fn peer_count(&self) -> usize {
        self.config.peers.len()
    }

    /// Get a cloneable sender handle for queuing outgoing messages.
    ///
    /// Useful when `run()` is spawned into a background task (it consumes
    /// `self`) but the caller still needs to send messages via the same
    /// underlying channel. `send_tx` is `mpsc::Sender`, which is `Clone`.
    pub fn sender_handle(&self) -> mpsc::Sender<AgentMessage> {
        self.send_tx.clone()
    }

    /// Get a shared handle to the incoming-message receiver.
    ///
    /// The returned `Arc<RwLock<..>>` is the same allocation used internally
    /// by `recv()`, so it stays valid even after `run()` consumes `self`
    /// (only the `CoordinationNetwork` struct is moved, not the `Arc`'s data).
    pub fn recv_handle(&self) -> Arc<RwLock<Option<mpsc::Receiver<AgentMessage>>>> {
        self.recv_rx.clone()
    }

    /// Run the coordination network with real commonware-p2p transport.
    ///
    /// This starts the commonware tokio runtime in a blocking thread (to avoid
    /// nested tokio runtimes), creates the authenticated lookup P2P network,
    /// registers the coordination message channel, and runs send/receive bridge
    /// loops that connect P2P transport to the user-facing mpsc channels.
    ///
    /// The method blocks indefinitely until the P2P runtime shuts down.
    pub async fn run(mut self) -> Result<()> {
        let listen_addr: SocketAddr = self.config.listen_addr.parse()?;
        let pk_hex = self.public_key_hex();
        let namespace = self.config.namespace.clone();
        let max_msg_size = self.config.max_message_size as u32;

        // Take the internal channel halves (user keeps send_tx and recv_rx)
        let mut send_rx = self.send_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("send_rx already taken — run() called twice?"))?;
        let recv_tx = self.recv_tx
            .take()
            .ok_or_else(|| anyhow::anyhow!("recv_tx already taken — run() called twice?"))?;

        // Parse peer addresses into commonware types
        let mut peer_entries: Vec<(ed25519::PublicKey, Address)> = Vec::new();
        for (pk_hex_str, addr_str) in &self.config.peers {
            let pk_bytes = hex::decode(pk_hex_str)
                .map_err(|e| anyhow::anyhow!("invalid peer public key hex '{}': {}", pk_hex_str, e))?;
            let pk = ed25519::PublicKey::try_from(&pk_bytes[..])
                .map_err(|e| anyhow::anyhow!("invalid peer public key: {:?}", e))?;
            let addr: SocketAddr = addr_str.parse()
                .map_err(|e| anyhow::anyhow!("invalid peer address '{}': {}", addr_str, e))?;
            peer_entries.push((pk, addr.into()));
        }

        // Add ourselves to the peer map
        let my_pk = self.signer.public_key();
        peer_entries.push((my_pk.clone(), listen_addr.into()));

        let peer_count = peer_entries.len();
        let peer_map: Map<ed25519::PublicKey, Address> = Map::from_iter_dedup(peer_entries);

        info!(
            "Coordination network starting: listen={}, pk={}, peers={}",
            listen_addr, pk_hex, peer_count
        );

        let signer = self.signer.clone();

        // Run commonware runtime in a blocking thread to avoid nested tokio runtimes.
        // The commonware tokio runner creates its own tokio runtime internally.
        let handle = tokio::task::spawn_blocking(move || {
            let runner = TokioRunner::new(RuntimeConfig::default());
            runner.start(|context| async move {
                // Configure P2P network using authenticated lookup (known addresses)
                let p2p_cfg = lookup::Config::local(
                    signer.clone(),
                    &namespace,
                    listen_addr,
                    max_msg_size,
                );

                // Initialize network and oracle
                let (mut network, mut oracle) =
                    P2pNetwork::new(context.child("network"), p2p_cfg);

                // Register all peers at peer-set ID 0
                oracle.track(0, peer_map);

                // Register the coordination message channel (channel ID 0)
                // 1000 msg/s per peer, 1024 message backlog
                let quota = Quota::per_second(NZU32!(1000));
                let (mut p2p_sender, mut p2p_receiver) = network.register(
                    0,
                    quota,
                    1024usize,
                );

                // Start the P2P network
                network.start();
                info!("P2P network started, awaiting peer connections...");

                // Spawn send-bridge: read from user send_rx, forward to P2P
                context.child("send_bridge").spawn(|_| async move {
                    while let Some(msg) = send_rx.recv().await {
                        let encoded = match msg.encode() {
                            Ok(data) => data,
                            Err(e) => {
                                warn!("Failed to encode message for P2P: {}", e);
                                continue;
                            }
                        };

                        let recipients = if msg.is_broadcast() {
                            Recipients::All
                        } else {
                            match ed25519::PublicKey::try_from(&msg.to[..]) {
                                Ok(pk) => Recipients::Some(vec![pk]),
                                Err(_) => {
                                    warn!(
                                        "Invalid recipient key ({} bytes), broadcasting",
                                        msg.to.len()
                                    );
                                    Recipients::All
                                }
                            }
                        };

                        let sent_to = p2p_sender.send(
                            recipients,
                            IoBuf::from(encoded),
                            false,
                        );
                        if sent_to.is_empty() {
                            warn!("P2P send returned no recipients (all rate-limited or disconnected?)");
                        }
                    }
                    info!("Send-bridge ended (user send channel closed)");
                });

                // Spawn recv-bridge: read from P2P, forward to user recv_tx
                context.child("recv_bridge").spawn(|_| async move {
                    loop {
                        match p2p_receiver.recv().await {
                            Ok((sender, data)) => {
                                let sender_pk = sender.to_vec();
                                let data_bytes: &[u8] = data.as_ref();
                                match AgentMessage::decode(data_bytes) {
                                    Ok(agent_msg) => {
                                        tracing::debug!(
                                            "Received P2P message from {}: {} bytes",
                                            hex::encode(&sender_pk),
                                            data.len()
                                        );
                                        if recv_tx.send(agent_msg).await.is_err() {
                                            warn!("User recv channel closed, stopping recv-bridge");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to decode P2P message from {}: {}",
                                            hex::encode(&sender_pk),
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("P2P receiver error: {:?}", e);
                                break;
                            }
                        }
                    }
                    info!("Recv-bridge ended (P2P receiver closed)");
                });

                // Keep the runtime alive indefinitely.
                // The bridge tasks will run until channels are closed.
                std::future::pending::<()>().await;
            });
        });

        // Wait for the P2P runtime (runs indefinitely until shutdown)
        handle.await
            .map_err(|e| anyhow::anyhow!("P2P runtime task panicked: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_creation() {
        let config = CoordinationConfig {
            listen_addr: "127.0.0.1:4001".to_string(),
            peers: vec![],
            max_message_size: 1_048_576,
            recv_buffer: 256,
            namespace: b"test-namespace".to_vec(),
        };
        let net = CoordinationNetwork::new(config).unwrap();
        assert_eq!(net.public_key().len(), 32);
        assert!(!net.public_key_hex().is_empty());
    }

    #[test]
    fn test_network_from_seed_deterministic() {
        let config = CoordinationConfig::default();
        let net1 = CoordinationNetwork::from_seed(42, config.clone()).unwrap();
        let net2 = CoordinationNetwork::from_seed(42, config).unwrap();
        assert_eq!(net1.public_key(), net2.public_key());
    }

    #[test]
    fn test_different_seeds_different_keys() {
        let config = CoordinationConfig::default();
        let net1 = CoordinationNetwork::from_seed(1, config.clone()).unwrap();
        let net2 = CoordinationNetwork::from_seed(2, config).unwrap();
        assert_ne!(net1.public_key(), net2.public_key());
    }
}
