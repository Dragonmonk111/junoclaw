//! Phase 4: J-Lens gate integration test.
//!
//! Tests that the truth gate correctly audits messages and batches:
//! - Clean content passes (green gate)
//! - Deceptive content is blocked (red gate)
//! - Suspicious content gets warning (yellow gate)
//! - Mixed batches filter out red-gated messages
//! - GateResult with attestation_hash is attached to finalized batches
//! - ConsensusEngine with gate filters red messages before finalization

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tracing::info;

use junoclaw_coordination::{
    AgentMessage, Batch, ConsensusConfig, ConsensusEngine, GateConfig, GateVerdict,
    JLensGate,
};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("=== Phase 4: J-Lens Gate Integration Test ===\n");

    let gate = JLensGate::mock_default();

    // ─── 1. Single message audits ────────────────────────────────────
    info!("--- Test: Single message audits ---");

    let clean_verdict = gate.audit(b"hello from agent alpha").await;
    assert_eq!(clean_verdict, GateVerdict::Green);
    info!("  Clean content: Green (pass)");

    let deceptive_verdict = gate.audit(b"this is a deceptive manipulation attempt").await;
    assert!(matches!(deceptive_verdict, GateVerdict::Red { .. }));
    info!("  Deceptive content: Red (blocked)");

    let suspicious_verdict = gate.audit(b"this is suspicious content").await;
    assert!(matches!(suspicious_verdict, GateVerdict::Yellow { .. }));
    info!("  Suspicious content: Yellow (warning)");

    // ─── 2. Batch-level audit — all clean ────────────────────────────
    info!("\n--- Test: Batch audit (all clean) ---");
    let clean_msgs = vec![
        AgentMessage::new(vec![1; 32], vec![], b"proposal vote yes".to_vec(), 1000),
        AgentMessage::new(vec![2; 32], vec![], b"agent coordination message".to_vec(), 2000),
    ];
    let clean_batch = Batch::new(clean_msgs, [0u8; 32], 1, now_ms());
    let clean_result = gate.audit_batch(&clean_batch).await;
    assert_eq!(clean_result.verdict, GateVerdict::Green);
    assert!(clean_result.attestation_hash.is_some(), "attestation hash should be present");
    info!("  Batch verdict: Green");
    info!("  Attestation hash: {}", clean_result.attestation_hash.as_ref().unwrap());

    // ─── 3. Batch-level audit — contains deceptive ───────────────────
    info!("\n--- Test: Batch audit (contains deceptive) ---");
    let mixed_msgs = vec![
        AgentMessage::new(vec![1; 32], vec![], b"clean message here".to_vec(), 1000),
        AgentMessage::new(vec![2; 32], vec![], b"deceptive exploit attempt".to_vec(), 2000),
        AgentMessage::new(vec![3; 32], vec![], b"another clean one".to_vec(), 3000),
    ];
    let mixed_batch = Batch::new(mixed_msgs, [0u8; 32], 1, now_ms());
    let mixed_result = gate.audit_batch(&mixed_batch).await;
    assert!(matches!(mixed_result.verdict, GateVerdict::Red { .. }));
    assert!(mixed_result.separation_score >= 0.9);
    info!("  Batch verdict: Red (separation_score={})", mixed_result.separation_score);

    // ─── 4. Batch-level audit — yellow but no red ────────────────────
    info!("\n--- Test: Batch audit (yellow, no red) ---");
    let yellow_msgs = vec![
        AgentMessage::new(vec![1; 32], vec![], b"clean message".to_vec(), 1000),
        AgentMessage::new(vec![2; 32], vec![], b"unverified claim".to_vec(), 2000),
    ];
    let yellow_batch = Batch::new(yellow_msgs, [0u8; 32], 1, now_ms());
    let yellow_result = gate.audit_batch(&yellow_batch).await;
    assert!(matches!(yellow_result.verdict, GateVerdict::Yellow { .. }));
    info!("  Batch verdict: Yellow (separation_score={})", yellow_result.separation_score);

    // ─── 5. Red overrides yellow in aggregate ────────────────────────
    info!("\n--- Test: Red overrides yellow in aggregate ---");
    let override_msgs = vec![
        AgentMessage::new(vec![1; 32], vec![], b"suspicious content".to_vec(), 1000),
        AgentMessage::new(vec![2; 32], vec![], b"malicious hack attempt".to_vec(), 2000),
    ];
    let override_batch = Batch::new(override_msgs, [0u8; 32], 1, now_ms());
    let override_result = gate.audit_batch(&override_batch).await;
    assert!(matches!(override_result.verdict, GateVerdict::Red { .. }));
    info!("  Batch verdict: Red (yellow overridden)");

    // ─── 6. ConsensusEngine with gate — filters red messages ─────────
    info!("\n--- Test: ConsensusEngine with gate filters red messages ---");
    let engine = ConsensusEngine::new(ConsensusConfig {
        num_validators: 4,
        block_time: Duration::from_millis(50),
        max_messages_per_block: 100,
        validator_index: 0,
    })
    .with_gate(JLensGate::mock_default());

    // Submit a clean and a deceptive message
    let clean_msg = AgentMessage::new(
        engine.validators()[0].clone(),
        vec![],
        b"clean consensus message".to_vec(),
        now_ms(),
    );
    let deceptive_msg = AgentMessage::new(
        engine.validators()[1].clone(),
        vec![],
        b"deceptive hack attempt".to_vec(),
        now_ms(),
    );
    engine.submit(clean_msg).await?;
    engine.submit(deceptive_msg).await?;

    // Produce a block using public API
    let block = engine.produce_block().await.expect("block should be produced");

    // The deceptive message should have been filtered out
    info!("  Block height: {}", block.height);
    info!("  Messages in block: {}", block.batch.len());
    assert_eq!(
        block.batch.len(),
        1,
        "deceptive message should be filtered, only clean message remains"
    );

    // GateResult should be attached
    assert!(
        block.batch.gate_result.is_some(),
        "GateResult should be attached to finalized batch"
    );
    let gate_result = block.batch.gate_result.as_ref().unwrap();
    info!(
        "  Gate verdict: {:?}, separation_score: {}",
        gate_result.verdict, gate_result.separation_score
    );
    info!(
        "  Attestation hash: {}",
        gate_result.attestation_hash.as_deref().unwrap_or("none")
    );
    assert!(gate_result.attestation_hash.is_some(), "attestation hash should be present");

    // ─── 7. ConsensusEngine with gate — all clean passes ─────────────
    info!("\n--- Test: ConsensusEngine with gate — all clean passes ---");
    let engine2 = ConsensusEngine::new(ConsensusConfig {
        num_validators: 4,
        block_time: Duration::from_millis(50),
        max_messages_per_block: 100,
        validator_index: 0,
    })
    .with_gate(JLensGate::mock_default());

    for i in 0..5 {
        let msg = AgentMessage::new(
            engine2.validators()[i % 4].clone(),
            vec![],
            format!("clean agent message {}", i).into_bytes(),
            now_ms(),
        );
        engine2.submit(msg).await?;
    }

    let block2 = engine2.produce_block().await.expect("block should be produced");

    assert_eq!(block2.batch.len(), 5, "all 5 clean messages should be in block");
    let gr = block2.batch.gate_result.as_ref().unwrap();
    assert_eq!(gr.verdict, GateVerdict::Green);
    info!("  All 5 clean messages passed through gate: Green");
    info!("  Attestation hash: {}", gr.attestation_hash.as_deref().unwrap_or("none"));

    // ─── 8. GateConfig custom thresholds ─────────────────────────────
    info!("\n--- Test: Custom gate thresholds ---");
    let custom_config = GateConfig {
        csi_endpoint: "http://localhost:9999".to_string(),
        yellow_threshold: 0.05,
        red_threshold: 0.10,
        timeout: Duration::from_secs(5),
        api_key: Some("test-key".to_string()),
    };
    let custom_gate = JLensGate::mock(custom_config);
    assert_eq!(custom_gate.config().yellow_threshold, 0.05);
    assert_eq!(custom_gate.config().red_threshold, 0.10);
    assert!(custom_gate.is_mock());
    info!("  Custom thresholds: yellow=0.05, red=0.10");

    // ─── Summary ──────────────────────────────────────────────────────
    info!("\n=== Phase 4 Gate Integration Test Summary ===");
    info!("  Single message audits: Green/Red/Yellow verified");
    info!("  Batch audit (all clean): Green with attestation");
    info!("  Batch audit (deceptive): Red blocked");
    info!("  Batch audit (yellow, no red): Yellow warning");
    info!("  Red overrides yellow: verified");
    info!("  ConsensusEngine filters red messages: verified");
    info!("  ConsensusEngine passes all clean: verified");
    info!("  Custom thresholds: verified");
    info!("\n=== Phase 4 Gate Integration Test: PASS ===");

    Ok(())
}
