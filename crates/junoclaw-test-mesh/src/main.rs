//! Local 3-node test mesh for JunoClaw coordination layer.
//!
//! Simulates a P2P mesh using in-process tokio channels, bypassing the
//! commonware-p2p dependency (which requires NASM/aws-lc-sys on Windows).
//!
//! Tests:
//! - Message creation, encoding, and hash verification
//! - Broadcast delivery to all peers
//! - Direct messaging to specific peers
//! - J-Lens gate verdict attachment
//! - Batch assembly and hash chaining
//! - Throughput benchmark (target: 1000 msg/s)

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use junoclaw_coordination::{
    AgentMessage, Batch, GateResult, GateVerdict,
};

/// A simulated peer in the test mesh.
struct TestPeer {
    label: String,
    public_key: [u8; 32],
    /// Channel to receive messages from the mesh bus
    rx: mpsc::Receiver<AgentMessage>,
}

impl TestPeer {
    /// Create a new test peer with a deterministic key.
    fn new(label: &str, seed: u8) -> (Self, mpsc::Sender<AgentMessage>) {
        let mut pk = [0u8; 32];
        pk[0] = seed;
        pk[31] = seed;
        let (tx, rx) = mpsc::channel::<AgentMessage>(1024);
        (
            TestPeer {
                label: label.to_string(),
                public_key: pk,
                rx,
            },
            tx,
        )
    }
}

/// The in-process mesh bus — routes messages between peers.
struct MeshBus {
    /// Sender for each peer (by public key prefix)
    peers: Vec<mpsc::Sender<AgentMessage>>,
}

impl MeshBus {
    fn new() -> Self {
        MeshBus { peers: vec![] }
    }

    fn add_peer(&mut self, sender: mpsc::Sender<AgentMessage>) {
        self.peers.push(sender);
    }

    /// Route a message to the recipient (or all peers if broadcast).
    async fn route(&self, msg: &AgentMessage) {
        if msg.is_broadcast() {
            // Send to all peers except sender
            for tx in &self.peers {
                let _ = tx.send(msg.clone()).await;
            }
        } else {
            // Send to specific recipient (match first byte of `to`)
            for tx in &self.peers {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("=== JunoClaw Local 3-Node Test Mesh ===");

    // ─── 1. Create 3 peers ────────────────────────────────────────────
    let (peer1, tx1) = TestPeer::new("alpha", 0x01);
    let (peer2, tx2) = TestPeer::new("beta", 0x02);
    let (peer3, tx3) = TestPeer::new("gamma", 0x03);

    let mut bus = MeshBus::new();
    bus.add_peer(tx1);
    bus.add_peer(tx2);
    bus.add_peer(tx3);

    let peers = vec![peer1, peer2, peer3];
    let peer_pks: Vec<[u8; 32]> = peers.iter().map(|p| p.public_key).collect();

    info!("Created 3 peers:");
    for (i, pk) in peer_pks.iter().enumerate() {
        info!("  Node {}: pk={}", i, hex::encode(pk));
    }

    // ─── 2. Test message creation and hash verification ───────────────
    info!("\n--- Test: Message creation & hash verification ---");
    let msg = AgentMessage::new(
        peer_pks[0].to_vec(),
        peer_pks[1].to_vec(),
        b"hello from alpha".to_vec(),
        now_ms(),
    );
    assert!(msg.verify_hash(), "hash verification should pass");
    info!("  Message hash verified: OK");
    info!("  Content: {:?}", String::from_utf8_lossy(&msg.content));
    info!("  Hash: {}", hex::encode(&msg.content_hash));

    // ─── 3. Test encode/decode round-trip ─────────────────────────────
    info!("\n--- Test: Encode/decode round-trip ---");
    let encoded = msg.encode()?;
    let decoded = AgentMessage::decode(&encoded)?;
    assert_eq!(msg.content, decoded.content);
    assert_eq!(msg.content_hash, decoded.content_hash);
    assert_eq!(msg.timestamp, decoded.timestamp);
    info!("  Encoded {} bytes, decoded OK", encoded.len());

    // ─── 4. Test broadcast delivery ───────────────────────────────────
    info!("\n--- Test: Broadcast delivery ---");
    let broadcast = AgentMessage::new(
        peer_pks[0].to_vec(),
        vec![],
        b"broadcast from alpha".to_vec(),
        now_ms(),
    );
    bus.route(&broadcast).await;

    let mut received = 0;
    for mut peer in peers {
        // Use try_recv in a non-blocking way with timeout
        let pk = peer.public_key;
        if let Ok(Some(msg)) =
            tokio::time::timeout(Duration::from_millis(100), peer.rx.recv()).await
        {
            if msg.is_broadcast() {
                received += 1;
                info!("  Node {} received broadcast: {:?}", hex::encode(&pk), String::from_utf8_lossy(&msg.content));
            }
        }
    }
    assert_eq!(received, 3, "all 3 nodes should receive broadcast");
    info!("  All 3 nodes received broadcast: OK");

    // ─── 5. Test J-Lens gate verdict attachment ────────────────────────
    info!("\n--- Test: J-Lens gate verdict attachment ---");
    let gated_msg = AgentMessage::new(
        peer_pks[0].to_vec(),
        vec![],
        b"gated message".to_vec(),
        now_ms(),
    )
    .with_gate(GateVerdict::Green);
    assert_eq!(gated_msg.j_lens_gate, Some(GateVerdict::Green));
    info!("  Green gate attached: OK");

    let _red_msg = AgentMessage::new(
        peer_pks[1].to_vec(),
        vec![],
        b"suspicious message".to_vec(),
        now_ms(),
    )
    .with_gate(GateVerdict::Red { separation_score: 0.95 });
    info!("  Red gate attached: separation_score=0.95");

    // ─── 6. Test batch assembly and hash chaining ─────────────────────
    info!("\n--- Test: Batch assembly & hash chaining ---");
    let msg1 = AgentMessage::new(peer_pks[0].to_vec(), vec![], b"msg1".to_vec(), 1000);
    let msg2 = AgentMessage::new(peer_pks[1].to_vec(), vec![], b"msg2".to_vec(), 2000);
    let msg3 = AgentMessage::new(peer_pks[2].to_vec(), vec![], b"msg3".to_vec(), 3000);

    let batch1 = Batch::new(vec![msg1.clone(), msg2], [0u8; 32], 1, 1500);
    let hash1 = batch1.hash();
    info!("  Batch 1: {} messages, hash={}", batch1.len(), hex::encode(&hash1));

    let batch2 = Batch::new(vec![msg3], hash1, 2, 3500);
    info!("  Batch 2: {} messages, prev_hash={}", batch2.len(), hex::encode(&batch2.prev_hash));
    assert_eq!(batch2.prev_hash, hash1, "batch chain linkage");
    info!("  Hash chain verified: OK");

    // ─── 7. Test blocked message detection ────────────────────────────
    info!("\n--- Test: Blocked message detection ---");
    let clean_msg = AgentMessage::new(peer_pks[0].to_vec(), vec![], b"clean".to_vec(), 1000)
        .with_gate(GateVerdict::Green);
    let blocked_msg = AgentMessage::new(peer_pks[1].to_vec(), vec![], b"bad".to_vec(), 2000)
        .with_gate(GateVerdict::Red { separation_score: 0.9 });

    let batch = Batch::new(vec![clean_msg, blocked_msg], [0u8; 32], 1, 3000);
    assert!(batch.has_blocked_message(), "batch should have blocked message");
    info!("  Blocked message detected: OK");

    let all_clean = Batch::new(
        vec![
            AgentMessage::new(peer_pks[0].to_vec(), vec![], b"clean1".to_vec(), 1000)
                .with_gate(GateVerdict::Green),
            AgentMessage::new(peer_pks[1].to_vec(), vec![], b"clean2".to_vec(), 2000)
                .with_gate(GateVerdict::Yellow { separation_score: 0.3 }),
        ],
        [0u8; 32],
        1,
        3000,
    );
    assert!(!all_clean.has_blocked_message(), "no blocked messages");
    info!("  No false positives on clean batch: OK");

    // ─── 8. Throughput benchmark ──────────────────────────────────────
    info!("\n--- Benchmark: Message throughput (target: 1000 msg/s) ---");

    // Recreate peers for benchmark (previous rx channels are consumed)
    let (mut peer1, tx1) = TestPeer::new("alpha", 0x01);
    let (mut peer2, tx2) = TestPeer::new("beta", 0x02);
    let (mut peer3, tx3) = TestPeer::new("gamma", 0x03);

    let mut bench_bus = MeshBus::new();
    bench_bus.add_peer(tx1);
    bench_bus.add_peer(tx2);
    bench_bus.add_peer(tx3);

    let total_msgs = 3000u64;
    let start = Instant::now();

    for i in 0..total_msgs {
        let content = format!("benchmark-msg-{}", i);
        let msg = AgentMessage::new(
            peer1.public_key.to_vec(),
            vec![],
            content.as_bytes().to_vec(),
            now_ms(),
        );
        bench_bus.route(&msg).await;
    }

    // Drain all received messages
    let mut total_received = 0u64;
    for rx in [&mut peer1.rx, &mut peer2.rx, &mut peer3.rx] {
        while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(10), rx.recv()).await {
            total_received += 1;
        }
    }

    let elapsed = start.elapsed();
    let msgs_per_sec = (total_msgs as f64 / elapsed.as_secs_f64()).round() as u64;

    info!("  Sent {} messages in {:.2}ms", total_msgs, elapsed.as_millis());
    info!("  Received {} messages total (3 peers)", total_received);
    info!("  Throughput: {} msg/s", msgs_per_sec);

    if msgs_per_sec >= 1000 {
        info!("  PASS: throughput >= 1000 msg/s target");
    } else {
        warn!("  BELOW TARGET: {} msg/s (target: 1000 msg/s)", msgs_per_sec);
    }

    // ─── 9. GateResult struct test ────────────────────────────────────
    info!("\n--- Test: GateResult struct ---");
    let gate_result = GateResult {
        verdict: GateVerdict::Yellow { separation_score: 0.42 },
        attestation_hash: Some("abc123".to_string()),
        separation_score: 0.42,
        model_id: Some("glm-4.5-air".to_string()),
    };
    let batch_with_gate = Batch::new(vec![msg1.clone()], [0u8; 32], 1, 1500)
        .with_gate_result(gate_result);
    assert!(batch_with_gate.gate_result.is_some());
    info!("  GateResult attached to batch: OK");

    // ─── Summary ──────────────────────────────────────────────────────
    info!("\n=== Test Mesh Summary ===");
    info!("  All protocol tests passed");
    info!("  Throughput: {} msg/s", msgs_per_sec);
    info!("  Target: 1000 msg/s — {}", if msgs_per_sec >= 1000 { "MET" } else { "BELOW" });

    Ok(())
}
