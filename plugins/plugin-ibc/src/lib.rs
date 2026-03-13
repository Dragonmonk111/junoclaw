use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

pub struct IbcPlugin {
    enabled: bool,
}

impl IbcPlugin {
    pub fn new() -> Self {
        Self { enabled: false }
    }
}

#[async_trait]
impl Plugin for IbcPlugin {
    fn name(&self) -> &str { "plugin-ibc" }
    fn description(&self) -> &str { "Cross-chain messaging and token transfers via IBC" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn capabilities(&self) -> Vec<PluginCapability> { vec![PluginCapability::IbcMessaging] }
    fn is_available(&self) -> bool { self.enabled }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": false }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.enabled = config.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        tracing::info!("IBC plugin initialized (enabled={})", self.enabled);
        Ok(())
    }

    async fn execute(&self, _task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        Err(junoclaw_core::error::JunoClawError::TaskExecution(
            "IBC plugin execution not yet implemented".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }
}
