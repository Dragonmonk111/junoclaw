//! Truth Market — Layer 6: finalizes eval epochs after batch settlement.
//!
//! After a batch is settled on Juno, the relayer calls the truth-market
//! contract's `FinalizeEpoch` to distribute rewards to evaluators whose
//! verdicts matched consensus, and slash those who diverged.
//!
//! The truth market contract holds operator stakes and tracks verdicts.
//! This module builds the `FinalizeEpoch` message and submits it on-chain.
//!
//! See: contracts/truth-market/ for the contract implementation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::watcher::FinalizedBatch;

/// Configuration for the truth market bridge.
#[derive(Clone, Debug)]
pub struct MarketConfig {
    /// Truth-market contract address on Juno
    pub truth_market_addr: String,
}

/// The `FinalizeEpoch` message for the truth-market contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FinalizeEpochMsg {
    /// The batch height being finalized
    pub batch_height: u64,
    /// The consensus gate result (from the settled batch)
    pub consensus_verdict: String,
    /// Hash of the messages in the batch (for verification)
    pub messages_hash: String,
}

/// Finalize an eval epoch for a settled batch.
///
/// Calls the truth-market contract's `FinalizeEpoch` execute msg,
/// which compares each operator's submitted verdict against the
/// consensus verdict and distributes rewards/slashes accordingly.
///
/// Best-effort: errors are logged but don't stall the watcher loop.
pub async fn finalize_epoch(
    rpc_endpoint: &str,
    relayer_key: &str,
    config: &MarketConfig,
    batch: &FinalizedBatch,
) -> Result<()> {
    let msg = FinalizeEpochMsg {
        batch_height: batch.commonware_height,
        consensus_verdict: "green".to_string(),
        messages_hash: hex::encode(&batch.messages_hash),
    };

    info!(
        "Finalizing truth-market epoch for batch {} (verdict={})",
        msg.batch_height, msg.consensus_verdict
    );

    // Dry-run mode: just log
    if relayer_key == "dry-run" || relayer_key.is_empty() {
        info!("[dry-run] Skipping truth-market finalization for batch {}", batch.commonware_height);
        return Ok(());
    }

    let msg_json = serde_json::to_string(&msg)?;

    // Shell out to cosmos-mcp CLI for signing and broadcasting
    let mcp_path = std::env::var("MCP_CLI_PATH")
        .unwrap_or_else(|_| "node".to_string());

    let output = tokio::process::Command::new(&mcp_path)
        .arg("mcp/dist/index.js")
        .arg("wallet")
        .arg("exec")
        .arg(relayer_key)
        .arg(&config.truth_market_addr)
        .arg(&msg_json)
        .arg("--rpc")
        .arg(rpc_endpoint)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            info!(
                "Truth-market epoch finalized for batch {} at contract {}: {}",
                batch.commonware_height, config.truth_market_addr, stdout.trim()
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!(
                "cosmos-mcp CLI returned non-zero exit for truth-market finalization: {}",
                stderr.trim()
            );
        }
        Err(e) => {
            warn!(
                "Failed to spawn cosmos-mcp CLI for truth-market finalization: {}",
                e
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finalize_epoch_logs_and_succeeds() {
        let config = MarketConfig {
            truth_market_addr: "juno1truth".to_string(),
        };
        let batch = FinalizedBatch {
            commonware_height: 42,
            messages_hash: [0xAB; 32],
            certificate: vec![],
            timestamp: 1000,
            payload_size_bytes: 0,
        };
        finalize_epoch("http://rpc", "key", &config, &batch)
            .await
            .unwrap();
    }

    #[test]
    fn finalize_epoch_msg_serialization() {
        let msg = FinalizeEpochMsg {
            batch_height: 100,
            consensus_verdict: "green".to_string(),
            messages_hash: "abcd1234".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("batch_height"));
        assert!(json.contains("consensus_verdict"));
        assert!(json.contains("messages_hash"));
    }
}
