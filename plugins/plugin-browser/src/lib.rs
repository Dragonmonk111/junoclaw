use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

pub struct BrowserPlugin {
    sandbox_mode: bool,
}

impl BrowserPlugin {
    pub fn new() -> Self {
        Self { sandbox_mode: true }
    }
}

#[async_trait]
impl Plugin for BrowserPlugin {
    fn name(&self) -> &str { "plugin-browser" }
    fn description(&self) -> &str { "Headless browser automation for web scraping and interaction" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn capabilities(&self) -> Vec<PluginCapability> { vec![PluginCapability::BrowserAutomation] }
    fn is_available(&self) -> bool { true }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sandbox_mode": { "type": "boolean", "default": true }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.sandbox_mode = config.get("sandbox_mode").and_then(|v| v.as_bool()).unwrap_or(true);
        tracing::info!("Browser plugin initialized (sandbox={})", self.sandbox_mode);
        Ok(())
    }

    async fn execute(&self, _task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        Err(junoclaw_core::error::JunoClawError::TaskExecution(
            "Browser automation not yet implemented".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }
}
