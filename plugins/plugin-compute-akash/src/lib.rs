use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::Result;
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

pub struct AkashComputePlugin {
    enabled: bool,
    default_gpu: String,
    max_hourly_rate_uakt: u64,
    payment_token: String,
}

impl AkashComputePlugin {
    pub fn new() -> Self {
        Self {
            enabled: false,
            default_gpu: "nvidia-h100".to_string(),
            max_hourly_rate_uakt: 1_500_000,
            payment_token: "juno".to_string(),
        }
    }
}

#[async_trait]
impl Plugin for AkashComputePlugin {
    fn name(&self) -> &str {
        "plugin-compute-akash"
    }

    fn description(&self) -> &str {
        "Deploy GPU compute tasks to Akash Network. User signs AKT payment directly via Skip Protocol (non-custodial)."
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn capabilities(&self) -> Vec<PluginCapability> {
        vec![PluginCapability::ComputeAkash]
    }

    fn is_available(&self) -> bool {
        self.enabled
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": false },
                "wallet_mnemonic_env": {
                    "type": "string",
                    "description": "Environment variable holding Akash wallet mnemonic"
                },
                "default_gpu": {
                    "type": "string",
                    "enum": ["nvidia-h100", "nvidia-a100", "nvidia-rtx-4090"],
                    "default": "nvidia-h100"
                },
                "max_hourly_rate_uakt": { "type": "integer", "default": 1500000 },
                "payment_token": {
                    "type": "string",
                    "enum": ["akt", "juno", "usdc", "atom"],
                    "default": "juno",
                    "description": "Token to pay with. Non-AKT tokens are swapped via Skip Protocol."
                },
                "max_slippage_percent": { "type": "number", "default": 0.5 },
                "auto_close_timeout_minutes": { "type": "integer", "default": 60 }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.enabled = config.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(gpu) = config.get("default_gpu").and_then(|v| v.as_str()) {
            self.default_gpu = gpu.to_string();
        }
        if let Some(rate) = config.get("max_hourly_rate_uakt").and_then(|v| v.as_u64()) {
            self.max_hourly_rate_uakt = rate;
        }
        if let Some(token) = config.get("payment_token").and_then(|v| v.as_str()) {
            self.payment_token = token.to_string();
        }
        tracing::info!(
            "Akash compute plugin initialized (enabled={}, gpu={}, payment={})",
            self.enabled, self.default_gpu, self.payment_token
        );
        Ok(())
    }

    async fn execute(&self, _task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        if !self.enabled {
            return Err(junoclaw_core::error::JunoClawError::Plugin {
                plugin: "plugin-compute-akash".to_string(),
                message: "Akash compute is not enabled".to_string(),
            });
        }
        // TODO: Phase 4 — SDL generation, Skip Protocol swap, deployment
        Err(junoclaw_core::error::JunoClawError::TaskExecution(
            "Akash compute execution not yet implemented".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Akash compute plugin shutting down");
        Ok(())
    }
}
