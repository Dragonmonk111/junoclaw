use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

pub struct LocalStoragePlugin {
    base_path: std::path::PathBuf,
}

impl LocalStoragePlugin {
    pub fn new(base_path: std::path::PathBuf) -> Self {
        Self { base_path }
    }
}

#[async_trait]
impl Plugin for LocalStoragePlugin {
    fn name(&self) -> &str { "plugin-storage-local" }
    fn description(&self) -> &str { "Local filesystem storage for agent workspaces and sessions" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn capabilities(&self) -> Vec<PluginCapability> { vec![PluginCapability::StorageLocal] }
    fn is_available(&self) -> bool { true }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "base_path": { "type": "string", "description": "Root storage directory" },
                "max_session_history": { "type": "integer", "default": 100 }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        if let Some(path) = config.get("base_path").and_then(|v| v.as_str()) {
            self.base_path = std::path::PathBuf::from(path);
        }
        std::fs::create_dir_all(&self.base_path)?;
        tracing::info!("Local storage plugin initialized at {}", self.base_path.display());
        Ok(())
    }

    async fn execute(&self, _task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        Err(junoclaw_core::error::JunoClawError::TaskExecution(
            "Direct execution not applicable for storage plugin".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }
}
