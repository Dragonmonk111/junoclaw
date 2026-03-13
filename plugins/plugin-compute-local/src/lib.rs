use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

pub struct LocalComputePlugin {
    max_concurrent: u32,
}

impl LocalComputePlugin {
    pub fn new(max_concurrent: u32) -> Self {
        Self { max_concurrent }
    }
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
        }
        tracing::info!("Local compute plugin initialized (max_concurrent={})", self.max_concurrent);
        Ok(())
    }

    async fn execute(&self, _task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        // TODO: Phase 1 — implement local task execution
        Err(junoclaw_core::error::JunoClawError::TaskExecution(
            "Local compute execution not yet implemented".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Local compute plugin shutting down");
        Ok(())
    }
}
