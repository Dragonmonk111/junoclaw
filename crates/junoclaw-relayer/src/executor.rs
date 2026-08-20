//! Executor — Layer 5: bridges settled coordination batches to the
//! task-ledger contract for on-chain task tracking and plugin execution.
//!
//! After a batch is settled (layer 3) and indexed in moultbook (layer 4),
//! the executor scans the batch's messages for `TaskRequest` payloads and
//! submits them to the task-ledger contract. Each `TaskRequest` becomes a
//! `SubmitTask` on-chain, triggering the task lifecycle:
//!
//!   SubmitTask → (plugin executes off-chain) → CompleteTask/FailTask
//!
//! The executor is best-effort: a failed task submission never stalls
//! settlement of subsequent batches. The actual plugin execution happens
//! off-chain via the junoclaw daemon's plugin system (see
//! `crates/junoclaw-core/src/plugin.rs`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::watcher::FinalizedBatch;

/// Configuration for the executor bridge.
#[derive(Clone, Debug)]
pub struct ExecutorConfig {
    /// Task-ledger contract address on Juno
    pub task_ledger_addr: String,
    /// Agent-registry contract address (for agent validation)
    pub agent_registry_addr: String,
    /// Whether to actually submit on-chain txs (false = dry-run mode)
    pub enabled: bool,
}

/// A task request extracted from a coordination batch message.
///
/// This is the on-chain representation that gets submitted to the
/// task-ledger contract's `SubmitTask` execute msg.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractedTask {
    pub agent_id: u64,
    pub input_hash: String,
    pub execution_tier: String,
    pub proposal_id: Option<u64>,
    pub batch_height: u64,
    pub message_hash: String,
}

/// The `SubmitTask` message format for the task-ledger contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmitTaskMsg {
    pub agent_id: u64,
    pub input_hash: String,
    pub execution_tier: String,
    pub proposal_id: Option<u64>,
    pub pre_hooks: Vec<serde_json::Value>,
    pub post_hooks: Vec<serde_json::Value>,
}

/// Extract task requests from a finalized batch.
///
/// Scans the batch's messages for `TaskRequest` payloads (JSON-encoded
/// in the message content). Non-task messages are skipped silently.
/// The `messages_hash` field links each extracted task back to the
/// settled batch for auditability.
pub fn extract_tasks(batch: &FinalizedBatch) -> Vec<ExtractedTask> {
    // The coordination node exposes the batch payload via its REST API.
    // In the current scaffold, FinalizedBatch only has the hash — we
    // need to fetch the full batch to extract tasks.
    //
    // For now, we return an empty vec. When the coordination node
    // exposes full batch payloads (not just hashes), this will parse
    // each message's content for TaskRequest JSON.
    //
    // TODO: Fetch full batch from coordination endpoint, parse messages.
    let _ = batch;
    Vec::new()
}

/// Extract tasks from a raw batch payload (JSON from coordination node).
///
/// This is the real extraction logic — given the full batch JSON (which
/// includes all messages), parse each message's content as a potential
/// `TaskRequest`.
pub fn extract_tasks_from_payload(
    batch_json: &serde_json::Value,
    batch_height: u64,
    messages_hash: &[u8; 32],
) -> Vec<ExtractedTask> {
    let mut tasks = Vec::new();
    let hash_hex = hex::encode(messages_hash);

    if let Some(messages) = batch_json.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            // Try to parse content as TaskRequest
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                if let Ok(task_req) = serde_json::from_str::<TaskRequestJson>(content) {
                    tasks.push(ExtractedTask {
                        agent_id: task_req.agent_id,
                        input_hash: task_req.input_hash,
                        execution_tier: task_req.execution_tier,
                        proposal_id: task_req.proposal_id,
                        batch_height,
                        message_hash: hash_hex.clone(),
                    });
                }
            }
        }
    }

    tasks
}

/// Internal struct for parsing TaskRequest from message content.
#[derive(Deserialize)]
struct TaskRequestJson {
    agent_id: u64,
    input_hash: String,
    execution_tier: String,
    plugin_hint: Option<String>,
    proposal_id: Option<u64>,
}

/// Build a `SubmitTask` message for the task-ledger contract.
pub fn build_submit_task_msg(task: &ExtractedTask) -> SubmitTaskMsg {
    SubmitTaskMsg {
        agent_id: task.agent_id,
        input_hash: task.input_hash.clone(),
        execution_tier: task.execution_tier.clone(),
        proposal_id: task.proposal_id,
        pre_hooks: Vec::new(),
        post_hooks: Vec::new(),
    }
}

/// Submit extracted tasks to the task-ledger contract.
///
/// Best-effort: logs failures but doesn't propagate errors for individual
/// task submissions. A batch with 5 tasks where 2 fail to submit still
/// counts as successful — the 3 that submitted will execute.
pub async fn submit_tasks(
    rpc_endpoint: &str,
    relayer_key: &str,
    config: &ExecutorConfig,
    tasks: &[ExtractedTask],
) -> Result<()> {
    if tasks.is_empty() {
        return Ok(());
    }

    info!(
        "Submitting {} task(s) to task-ledger {}",
        tasks.len(),
        config.task_ledger_addr
    );

    for task in tasks {
        let msg = build_submit_task_msg(task);

        if !config.enabled {
            info!(
                "[dry-run] Would submit task for agent {} (tier={}, batch={})",
                task.agent_id, task.execution_tier, task.batch_height
            );
            continue;
        }

        match submit_single_task(rpc_endpoint, relayer_key, &config.task_ledger_addr, &msg).await {
            Ok(()) => {
                info!(
                    "Task submitted: agent={} tier={} batch={}",
                    task.agent_id, task.execution_tier, task.batch_height
                );
            }
            Err(e) => {
                warn!(
                    "Task submission failed for agent {} (batch {}): {}",
                    task.agent_id, task.batch_height, e
                );
            }
        }
    }

    Ok(())
}

/// Submit a single task to the task-ledger contract.
///
/// Shells out to the cosmos-mcp CLI wallet store for tx signing,
/// same as bridge.rs. The relayer_key is a wallet_id referencing
/// an encrypted wallet in ~/.junoclaw/wallets/.
async fn submit_single_task(
    rpc_endpoint: &str,
    relayer_key: &str,
    task_ledger_addr: &str,
    msg: &SubmitTaskMsg,
) -> Result<()> {
    let msg_json = serde_json::to_string(msg)
        .context("failed to serialize SubmitTask msg")?;

    info!(
        "Built SubmitTask tx for contract {} (agent={}, tier={})",
        task_ledger_addr, msg.agent_id, msg.execution_tier
    );

    // Dry-run mode: just log
    if relayer_key == "dry-run" || relayer_key.is_empty() {
        info!("[dry-run] Skipping task submission for agent {}", msg.agent_id);
        return Ok(());
    }

    // Shell out to cosmos-mcp CLI for signing and broadcasting
    let mcp_path = std::env::var("MCP_CLI_PATH")
        .unwrap_or_else(|_| "node".to_string());

    let output = tokio::process::Command::new(&mcp_path)
        .arg("mcp/dist/index.js")
        .arg("wallet")
        .arg("exec")
        .arg(relayer_key)
        .arg(task_ledger_addr)
        .arg(&msg_json)
        .arg("--rpc")
        .arg(rpc_endpoint)
        .output()
        .await
        .context("failed to spawn cosmos-mcp CLI subprocess")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "cosmos-mcp CLI returned non-zero exit for task submission: {}",
            stderr.trim()
        );
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    info!(
        "Task submitted for agent {}: {}",
        msg.agent_id,
        stdout.trim()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_tasks_from_empty_batch() {
        let batch = FinalizedBatch {
            commonware_height: 1,
            messages_hash: hex::encode([0u8; 32]),
            certificate: String::new(),
            timestamp: 1000,
            payload_size_bytes: 0,
            breaker_actions: Vec::new(),
            context_digest: None,
            batch_hash: hex::encode([0u8; 32]),
            message_count: 0,
        };
        assert!(extract_tasks(&batch).is_empty());
    }

    #[test]
    fn extract_tasks_from_payload_with_tasks() {
        let batch_json = json!({
            "messages": [
                {
                    "content": serde_json::to_string(&json!({
                        "agent_id": 42,
                        "input_hash": "abc123",
                        "execution_tier": "akash",
                        "plugin_hint": "plugin-compute-akash",
                        "proposal_id": 7
                    })).unwrap()
                },
                {
                    "content": "not a task request"
                },
                {
                    "content": serde_json::to_string(&json!({
                        "agent_id": 1,
                        "input_hash": "deadbeef",
                        "execution_tier": "local",
                        "plugin_hint": null,
                        "proposal_id": null
                    })).unwrap()
                }
            ]
        });

        let tasks = extract_tasks_from_payload(&batch_json, 100, &[0xAB; 32]);
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].agent_id, 42);
        assert_eq!(tasks[0].execution_tier, "akash");
        assert_eq!(tasks[0].proposal_id, Some(7));
        assert_eq!(tasks[0].batch_height, 100);

        assert_eq!(tasks[1].agent_id, 1);
        assert_eq!(tasks[1].execution_tier, "local");
        assert_eq!(tasks[1].proposal_id, None);
    }

    #[test]
    fn extract_tasks_from_payload_no_messages() {
        let batch_json = json!({});
        let tasks = extract_tasks_from_payload(&batch_json, 1, &[0u8; 32]);
        assert!(tasks.is_empty());
    }

    #[test]
    fn build_submit_task_msg_correct() {
        let task = ExtractedTask {
            agent_id: 5,
            input_hash: "hash123".to_string(),
            execution_tier: "local".to_string(),
            proposal_id: Some(3),
            batch_height: 42,
            message_hash: "abcd".to_string(),
        };
        let msg = build_submit_task_msg(&task);
        assert_eq!(msg.agent_id, 5);
        assert_eq!(msg.input_hash, "hash123");
        assert_eq!(msg.execution_tier, "local");
        assert_eq!(msg.proposal_id, Some(3));
        assert!(msg.pre_hooks.is_empty());
        assert!(msg.post_hooks.is_empty());
    }

    #[tokio::test]
    async fn submit_tasks_empty_is_noop() {
        let config = ExecutorConfig {
            task_ledger_addr: "juno1task".to_string(),
            agent_registry_addr: "juno1reg".to_string(),
            enabled: true,
        };
        submit_tasks("http://rpc", "key", &config, &[])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn submit_tasks_dry_run_logs() {
        let config = ExecutorConfig {
            task_ledger_addr: "juno1task".to_string(),
            agent_registry_addr: "juno1reg".to_string(),
            enabled: false,
        };
        let task = ExtractedTask {
            agent_id: 1,
            input_hash: "h".to_string(),
            execution_tier: "local".to_string(),
            proposal_id: None,
            batch_height: 1,
            message_hash: "m".to_string(),
        };
        submit_tasks("http://rpc", "key", &config, &[task])
            .await
            .unwrap();
    }
}
