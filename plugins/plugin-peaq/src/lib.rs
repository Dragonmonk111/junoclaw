use async_trait::async_trait;
use serde_json::Value;

use junoclaw_core::error::{JunoClawError, Result};
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};

/// peaqOS plug-in adapter.
///
/// Bridges peaq's identity and commerce layer into the JunoClaw trust core:
/// - peaqID DID → jclaw-credential verification (identity plug-in)
/// - MCR credit rating → moultbook work-integrity score (credit plug-in)
/// - Scale/robotic.sh → skill-registry service discovery (commerce plug-in)
///
/// This adapter is **optional** — JunoClaw works standalone without peaq.
/// When configured, it fetches peaq on-chain data and feeds it into JunoClaw's
/// sovereign primitives for cross-verification.
pub struct PeaqPlugin {
    enabled: bool,
    peaq_rpc: String,
    peaq_id_contract: String,
    mcr_contract: String,
}

impl PeaqPlugin {
    pub fn new() -> Self {
        Self {
            enabled: false,
            peaq_rpc: String::new(),
            peaq_id_contract: String::new(),
            mcr_contract: String::new(),
        }
    }
}

#[async_trait]
impl Plugin for PeaqPlugin {
    fn name(&self) -> &str {
        "plugin-peaq"
    }
    fn description(&self) -> &str {
        "peaqOS identity + credit rating + commerce plug-in for JunoClaw trust core"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn capabilities(&self) -> Vec<PluginCapability> {
        vec![PluginCapability::ExternalIdentity]
    }
    fn is_available(&self) -> bool {
        self.enabled
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": false },
                "peaq_rpc": {
                    "type": "string",
                    "description": "peaq chain RPC endpoint (e.g. https://peaq-rpc.publicnode.com:443)"
                },
                "peaq_id_contract": {
                    "type": "string",
                    "description": "peaqID DID contract address on peaq chain"
                },
                "mcr_contract": {
                    "type": "string",
                    "description": "Machine Credit Rating contract address on peaq chain"
                }
            },
            "required": ["peaq_rpc", "peaq_id_contract", "mcr_contract"]
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.enabled = config
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.peaq_rpc = config
            .get("peaq_rpc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.peaq_id_contract = config
            .get("peaq_id_contract")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.mcr_contract = config
            .get("mcr_contract")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if self.enabled && (self.peaq_rpc.is_empty() || self.peaq_id_contract.is_empty()) {
            return Err(JunoClawError::Config(
                "peaq plugin enabled but peaq_rpc or peaq_id_contract not set".to_string(),
            ));
        }

        tracing::info!(
            "peaqOS plugin initialized (enabled={}, rpc={})",
            self.enabled,
            self.peaq_rpc
        );
        Ok(())
    }

    async fn execute(&self, task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        if !self.enabled {
            return Err(JunoClawError::Plugin {
                plugin: "plugin-peaq".to_string(),
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
            "fetch_peaq_id" => {
                let did = input
                    .get("did")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'did' parameter".to_string())
                    })?;

                tracing::info!("Fetching peaqID for DID: {}", did);
                Err(JunoClawError::Plugin {
                    plugin: "plugin-peaq".to_string(),
                    message: format!(
                        "peaqID fetch for {} — wire to jclaw-credential verification at {}",
                        did, self.peaq_id_contract
                    ),
                })
            }
            "fetch_mcr" => {
                let machine_addr = input
                    .get("machine_address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution(
                            "missing 'machine_address' parameter".to_string(),
                        )
                    })?;

                tracing::info!("Fetching MCR for machine: {}", machine_addr);
                Err(JunoClawError::Plugin {
                    plugin: "plugin-peaq".to_string(),
                    message: format!(
                        "MCR fetch for {} — feed result to moultbook credit score at {}",
                        machine_addr, self.mcr_contract
                    ),
                })
            }
            _ => Err(JunoClawError::TaskExecution(format!(
                "unknown peaq plugin action: {}",
                action
            ))),
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("peaqOS plugin shutting down");
        Ok(())
    }
}
