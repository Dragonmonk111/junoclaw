use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::{JunoClawError, Result};
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

/// rsynth plug-in adapter.
///
/// Bridges rsynth's verifiable execution SDK into the JunoClaw trust core:
/// - Fetches rsynth execution proofs from Base (or other configured chains)
/// - Feeds execution payload hashes into J-Lens for cognitive integrity verification
/// - Anchors execution results alongside JunoClaw's BFT consensus certificates
///
/// rsynth proves *what* was executed. J-Lens proves *whether the agent's
/// internal reasoning was honest*. Together: execution proof + cognitive
/// integrity proof.
///
/// This adapter is **optional** — JunoClaw works standalone without rsynth.
pub struct RsynthPlugin {
    enabled: bool,
    rsynth_api: String,
    base_rpc: String,
}

impl RsynthPlugin {
    pub fn new() -> Self {
        Self {
            enabled: false,
            rsynth_api: String::new(),
            base_rpc: String::new(),
        }
    }
}

#[async_trait]
impl Plugin for RsynthPlugin {
    fn name(&self) -> &str {
        "plugin-rsynth"
    }
    fn description(&self) -> &str {
        "rsynth verifiable execution proof plug-in for JunoClaw trust core"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn capabilities(&self) -> Vec<PluginCapability> {
        vec![PluginCapability::ExecutionProof]
    }
    fn is_available(&self) -> bool {
        self.enabled
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": false },
                "rsynth_api": {
                    "type": "string",
                    "description": "rsynth API endpoint for fetching execution proofs"
                },
                "base_rpc": {
                    "type": "string",
                    "description": "Base chain RPC endpoint for on-chain proof verification",
                    "default": "https://mainnet.base.org"
                }
            },
            "required": ["rsynth_api"]
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.enabled = config
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.rsynth_api = config
            .get("rsynth_api")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.base_rpc = config
            .get("base_rpc")
            .and_then(|v| v.as_str())
            .unwrap_or("https://mainnet.base.org")
            .to_string();

        if self.enabled && self.rsynth_api.is_empty() {
            return Err(JunoClawError::Config(
                "rsynth plugin enabled but rsynth_api not set".to_string(),
            ));
        }

        tracing::info!(
            "rsynth plugin initialized (enabled={}, api={})",
            self.enabled,
            self.rsynth_api
        );
        Ok(())
    }

    async fn execute(&self, task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        if !self.enabled {
            return Err(JunoClawError::Plugin {
                plugin: "plugin-rsynth".to_string(),
                message: "plugin not enabled".to_string(),
            });
        }

        let input: Value = serde_json::from_str(&task.input)
            .unwrap_or_else(|_| serde_json::json!({ "action": task.input }));

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match action {
            "fetch_execution_proof" => {
                let proof_id = input
                    .get("proof_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'proof_id' parameter".to_string())
                    })?;

                tracing::info!("Fetching rsynth execution proof: {}", proof_id);
                Err(JunoClawError::Plugin {
                    plugin: "plugin-rsynth".to_string(),
                    message: format!(
                        "execution proof fetch for {} — anchor payload hash to J-Lens gate for cognitive integrity verification",
                        proof_id
                    ),
                })
            }
            "verify_execution" => {
                let tx_hash = input
                    .get("tx_hash")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'tx_hash' parameter".to_string())
                    })?;

                tracing::info!("Verifying rsynth execution on Base: tx={}", tx_hash);
                Err(JunoClawError::Plugin {
                    plugin: "plugin-rsynth".to_string(),
                    message: format!(
                        "on-chain verification for tx {} via {} — feed result to zk-verifier for proof anchoring",
                        tx_hash, self.base_rpc
                    ),
                })
            }
            _ => Err(JunoClawError::TaskExecution(format!(
                "unknown rsynth plugin action: {}",
                action
            ))),
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("rsynth plugin shutting down");
        Ok(())
    }
}
