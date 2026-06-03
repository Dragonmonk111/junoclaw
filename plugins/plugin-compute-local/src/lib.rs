use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult, TokenUsage};

pub struct LocalComputePlugin {
    max_concurrent: u32,
    /// Bounds the number of concurrently executing local tasks.
    semaphore: Arc<Semaphore>,
}

impl LocalComputePlugin {
    pub fn new(max_concurrent: u32) -> Self {
        let permits = max_concurrent.max(1) as usize;
        Self {
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(permits)),
        }
    }
}

/// Deterministic local-compute core.
///
/// The task input may carry an optional `op:arg` directive. All operations are
/// pure functions of the input, so the same input always yields the same output
/// (and therefore the same `output_hash`) — the property the verification layer
/// relies on. Unknown/absent directives fall back to an identity echo.
pub fn run_local_compute(input: &str) -> String {
    let trimmed = input.trim();
    match trimmed.split_once(':') {
        Some(("hash", arg)) => {
            format!("sha256:{}", hex::encode(Sha256::digest(arg.as_bytes())))
        }
        Some(("reverse", arg)) => arg.chars().rev().collect(),
        Some(("upper", arg)) => arg.to_uppercase(),
        Some(("lower", arg)) => arg.to_lowercase(),
        Some(("wordcount", arg)) => arg.split_whitespace().count().to_string(),
        _ => trimmed.to_string(),
    }
}

/// `sha256:<hex>` digest of an output string — the canonical JunoClaw
/// content-hash convention (matches `junoclaw-common`).
pub fn output_hash(output: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(output.as_bytes())))
}

#[async_trait]
impl Plugin for LocalComputePlugin {
    fn name(&self) -> &str {
        "plugin-compute-local"
    }

    fn description(&self) -> &str {
        "Execute tasks on the local machine's CPU/GPU"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn capabilities(&self) -> Vec<PluginCapability> {
        vec![PluginCapability::ComputeLocal]
    }

    fn is_available(&self) -> bool {
        true
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "max_concurrent_tasks": {
                    "type": "integer",
                    "default": 4,
                    "description": "Maximum concurrent local tasks"
                }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        if let Some(max) = config.get("max_concurrent_tasks").and_then(|v| v.as_u64()) {
            self.max_concurrent = max as u32;
            self.semaphore = Arc::new(Semaphore::new(self.max_concurrent.max(1) as usize));
        }
        tracing::info!("Local compute plugin initialized (max_concurrent={})", self.max_concurrent);
        Ok(())
    }

    async fn execute(&self, task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        // Bound concurrency: never run more than `max_concurrent` local tasks.
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            junoclaw_core::error::JunoClawError::TaskExecution(format!(
                "compute-local semaphore closed: {e}"
            ))
        })?;

        tracing::info!("compute-local executing task {} (tier={:?})", task.id, task.tier);

        // Deterministic, verifiable local execution.
        let output = run_local_compute(&task.input);
        let output_hash = output_hash(&output);

        Ok(TaskResult {
            output,
            output_hash,
            tool_calls: vec![],
            tokens_used: TokenUsage::default(),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Local compute plugin shutting down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use junoclaw_core::plugin::PluginContext;
    use junoclaw_core::types::{ExecutionTier, Task};

    fn ctx() -> PluginContext {
        PluginContext {
            agent_id: "agent-1".to_string(),
            session_id: "sess-1".to_string(),
            workspace_dir: std::env::temp_dir(),
            budget_remaining_usd: 0.0,
        }
    }

    #[test]
    fn run_local_compute_directives() {
        assert_eq!(run_local_compute("reverse:abc"), "cba");
        assert_eq!(run_local_compute("upper:abc"), "ABC");
        assert_eq!(run_local_compute("lower:ABC"), "abc");
        assert_eq!(run_local_compute("wordcount:a b c"), "3");
        assert_eq!(
            run_local_compute("hash:abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Unknown/absent directive echoes the trimmed input.
        assert_eq!(run_local_compute("  plain text  "), "plain text");
        assert_eq!(run_local_compute("unknownop:data"), "unknownop:data");
    }

    #[test]
    fn output_hash_is_deterministic_and_prefixed() {
        let a = output_hash("hello");
        let b = output_hash("hello");
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:"));
        assert_eq!(a.len(), "sha256:".len() + 64);
    }

    #[tokio::test]
    async fn execute_produces_deterministic_result() {
        let plugin = LocalComputePlugin::new(2);
        let task = Task::new("agent-1", "upper:hello", ExecutionTier::Local);
        let r1 = plugin.execute(&task, &ctx()).await.unwrap();
        assert_eq!(r1.output, "HELLO");
        assert_eq!(r1.output_hash, output_hash("HELLO"));
        assert_eq!(r1.tokens_used.total_tokens, 0);
        assert!(r1.tool_calls.is_empty());

        // Same input → identical hash (verifiability).
        let r2 = plugin.execute(&task, &ctx()).await.unwrap();
        assert_eq!(r1.output_hash, r2.output_hash);
    }
}
