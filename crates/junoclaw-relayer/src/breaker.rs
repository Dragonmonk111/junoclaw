//! Circuit breaker bridge — submits TripBreaker transactions to the
//! circuit-breaker contract on Juno when BreakerActions are detected
//! in finalized coordination batches.
//!
//! The relayer's watcher calls `submit_breaker_actions` after each
//! successful batch settlement. Each BreakerAction becomes a
//! `TripBreaker { robot_id, reason, cause_ref }` ExecuteMsg on the
//! circuit-breaker contract.

use anyhow::Result;
use serde::Serialize;
use tracing::{info, warn};

use junoclaw_coordination::BreakerAction;

/// The `TripBreaker` message for the circuit-breaker contract.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct TripBreakerMsg {
    robot_id: String,
    reason: String,
    cause_ref: String,
}

/// Configuration for the breaker bridge.
#[derive(Clone, Debug)]
pub struct BreakerConfig {
    /// Circuit-breaker contract address on Juno
    pub breaker_addr: String,
}

/// Submit breaker actions to the circuit-breaker contract.
///
/// Each action becomes a `TripBreaker` tx. Uses the same MCP CLI
/// subprocess pattern as `bridge::submit_batch`. In dry-run mode,
/// just logs.
pub async fn submit_breaker_actions(
    rpc_endpoint: &str,
    relayer_key: &str,
    config: &BreakerConfig,
    actions: &[BreakerAction],
) -> Result<()> {
    for action in actions {
        let msg = TripBreakerMsg {
            robot_id: action.robot_id.clone(),
            reason: action.reason.clone(),
            cause_ref: action.cause_ref.clone(),
        };
        let msg_json = serde_json::to_string(&msg)?;

        info!(
            "Built TripBreaker tx for robot {} (batch={}, cause={})",
            action.robot_id, action.batch_height, action.cause_ref
        );

        // Dry-run mode: just log, don't submit
        if relayer_key == "dry-run" || relayer_key.is_empty() {
            info!(
                "[dry-run] Skipping TripBreaker submission for robot {} (batch {})",
                action.robot_id, action.batch_height
            );
            continue;
        }

        // Shell out to cosmos-mcp CLI for signing and broadcasting
        let mcp_path = std::env::var("MCP_CLI_PATH")
            .unwrap_or_else(|_| "node".to_string());

        let output = tokio::process::Command::new(&mcp_path)
            .arg("mcp/dist/index.js")
            .arg("wallet")
            .arg("exec")
            .arg(relayer_key)
            .arg(&config.breaker_addr)
            .arg(&msg_json)
            .arg("--rpc")
            .arg(rpc_endpoint)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                info!(
                    "TripBreaker submitted for robot {} (batch {}): {}",
                    action.robot_id, action.batch_height, stdout.trim()
                );
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!(
                    "TripBreaker CLI returned non-zero for robot {}: {}",
                    action.robot_id,
                    stderr.trim()
                );
            }
            Err(e) => {
                warn!(
                    "Failed to spawn CLI for TripBreaker (robot {}): {}",
                    action.robot_id, e
                );
            }
        }
    }

    Ok(())
}
