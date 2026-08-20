//! Bridge — submits batches to the coordination-settler contract on Juno.
//!
//! This module handles the actual on-chain transaction submission.
//! It builds a CosmWasm ExecuteMsg::SubmitBatch, signs it with the
//! relayer's key, and broadcasts it to the Juno RPC endpoint.
//!
//! Signing approach: the relayer shells out to the cosmos-mcp CLI
//! (`node mcp/dist/index.js wallet exec <wallet_id> <contract> <msg_json>`)
//! which handles wallet decryption, tx signing, and broadcasting via
//! cosmjs. This reuses the MCP wallet store's encrypted key management
//! (AES-GCM at rest, passphrase/keychain backend) without duplicating
//! crypto code in Rust.
//!
//! For dry-run / soak-test mode, the relayer key can be set to "dry-run"
//! which skips actual submission and just logs.

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{info, warn};

use crate::watcher::FinalizedBatch;

/// The `SubmitBatch` message for the coordination-settler contract.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct SubmitBatchMsg {
    certificate: String,
    messages_hash: String,
    commonware_height: u64,
    timestamp: u64,
}

/// Submit a finalized batch to the coordination-settler contract.
///
/// Builds the ExecuteMsg JSON and delegates signing/broadcasting to
/// the cosmos-mcp wallet store via CLI subprocess.
pub async fn submit_batch(
    rpc_endpoint: &str,
    contract_addr: &str,
    relayer_key: &str,
    batch: &FinalizedBatch,
) -> Result<()> {
    let cert_hex = &batch.certificate;
    let msg_hash_hex = &batch.messages_hash;

    info!(
        "Built SubmitBatch tx for contract {} (height={}, cert={}..., msg_hash={})",
        contract_addr, batch.commonware_height, &cert_hex[..cert_hex.len().min(16)], msg_hash_hex
    );

    // Dry-run mode: just log, don't submit
    if relayer_key == "dry-run" || relayer_key.is_empty() {
        info!("[dry-run] Skipping on-chain submission for batch {}", batch.commonware_height);
        return Ok(());
    }

    // Build the ExecuteMsg JSON
    let msg = SubmitBatchMsg {
        certificate: cert_hex.clone(),
        messages_hash: msg_hash_hex.clone(),
        commonware_height: batch.commonware_height,
        timestamp: batch.timestamp,
    };
    let msg_json = serde_json::to_string(&msg)
        .context("failed to serialize SubmitBatch msg")?;

    // Shell out to cosmos-mcp CLI for signing and broadcasting.
    // The wallet_id (relayer_key) references an encrypted wallet in
    // ~/.junoclaw/wallets/ that was enrolled via `cosmos-mcp wallet add`.
    //
    // Command: node mcp/dist/index.js wallet exec <wallet_id> <contract> <msg_json> --rpc <rpc>
    let mcp_path = std::env::var("MCP_CLI_PATH")
        .unwrap_or_else(|_| "node".to_string());

    let output = tokio::process::Command::new(&mcp_path)
        .arg("mcp/dist/index.js")
        .arg("wallet")
        .arg("exec")
        .arg(relayer_key)
        .arg(contract_addr)
        .arg(&msg_json)
        .arg("--rpc")
        .arg(rpc_endpoint)
        .output()
        .await
        .context("failed to spawn cosmos-mcp CLI subprocess")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "cosmos-mcp CLI returned non-zero exit: {}",
            stderr.trim()
        );
        // Don't fail hard — the watcher loop will retry
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    info!(
        "Batch {} submitted on Juno: {}",
        batch.commonware_height,
        stdout.trim()
    );

    Ok(())
}
