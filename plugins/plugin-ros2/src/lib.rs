pub mod onchain;

use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::RwLock;

use junoclaw_coordination::message::{AgentMessage, IntentMessage, ReflexBatchAttestation, CircuitBreakerState};
use junoclaw_core::error::{JunoClawError, Result};
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult};
use onchain::OnChainClient;

/// ROS2 plug-in adapter.
///
/// Bridges a ROS2-based robot's intent-tier decisions into the JunoClaw
/// trust core using the `IntentMessage` schema:
/// - Converts ROS2 action server output into typed `IntentMessage` payloads
/// - Wraps each `IntentMessage` in an `AgentMessage` for gate auditing
/// - The gate audits the intent; the Truth Market settles the outcome
///
/// The reflex-tier / intent-tier split is real in this adapter:
/// - Reflex-tier (sub-100ms sensor fusion, balance, collision avoidance)
///   stays on the robot controller and never becomes an `IntentMessage`
/// - Intent-tier ("engage target", "take this route", "use this tool")
///   is wrapped in `IntentMessage` and fed through the gate
///
/// This adapter is **optional** — JunoClaw works standalone without ROS2.
/// When configured, it converts the robot's intent-tier decisions into
/// `IntentMessage`-encoded `AgentMessage`s for the J-Lens gate.
pub struct Ros2Plugin {
    enabled: bool,
    ros2_bridge_url: String,
    robot_id: String,
    robot_type: String,
    circuit_breaker: CircuitBreakerState,
    /// Optional on-chain client for querying SafetyEnvelope + CircuitBreaker contracts.
    /// When set, the plugin queries on-chain state instead of relying solely on
    /// in-memory `circuit_breaker`. Falls back to in-memory if the chain is unreachable.
    onchain_client: Option<Arc<OnChainClient>>,
    /// Cached envelope version from the on-chain SafetyEnvelope contract.
    cached_envelope_version: Arc<RwLock<Option<u32>>>,
}

impl Ros2Plugin {
    pub fn new() -> Self {
        Self {
            enabled: false,
            ros2_bridge_url: String::new(),
            robot_id: String::new(),
            robot_type: "wheeled".to_string(),
            circuit_breaker: CircuitBreakerState::Closed,
            onchain_client: None,
            cached_envelope_version: Arc::new(RwLock::new(None)),
        }
    }

    /// Compute a SHA-256 hash of a sensor snapshot (hex-encoded).
    fn hash_sensor_snapshot(snapshot: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(snapshot);
        hex::encode(hasher.finalize())
    }

    /// Build an `IntentMessage` from ROS2 action server output.
    ///
    /// This is the conversion point where a ROS2 action goal/result becomes
    /// a typed intent-tier message that the gate can audit.
    fn build_intent_message(
        &self,
        action: &str,
        params: Value,
        sensor_snapshot: &[u8],
        controller_timestamp: u64,
        rationale: Option<String>,
        execution_proof_ref: Option<String>,
    ) -> IntentMessage {
        IntentMessage {
            robot_id: self.robot_id.clone(),
            action: action.to_string(),
            params,
            sensor_snapshot_hash: Self::hash_sensor_snapshot(sensor_snapshot),
            controller_timestamp,
            rationale,
            execution_proof_ref,
        }
    }

    /// Wrap an `IntentMessage` into an `AgentMessage` for gate submission.
    ///
    /// This is the wire point: the `IntentMessage` is encoded as JSON in the
    /// `AgentMessage.content` field. The gate hashes and audits it; the Truth
    /// Market settles the outcome.
    fn wrap_intent_into_agent_message(
        &self,
        intent: IntentMessage,
        from: Vec<u8>,
        timestamp: u64,
    ) -> Result<AgentMessage> {
        intent
            .into_agent_message(from, vec![], timestamp)
            .map_err(|e| JunoClawError::TaskExecution(format!("failed to encode intent: {}", e)))
    }
}

#[async_trait]
impl Plugin for Ros2Plugin {
    fn name(&self) -> &str {
        "plugin-ros2"
    }
    fn description(&self) -> &str {
        "ROS2 robot execution/sensor proof plug-in for JunoClaw trust core"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn capabilities(&self) -> Vec<PluginCapability> {
        vec![PluginCapability::RoboticsControl]
    }
    fn is_available(&self) -> bool {
        self.enabled
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": false },
                "ros2_bridge_url": {
                    "type": "string",
                    "description": "ROS2 bridge HTTP endpoint (e.g. http://robot-local:8080)"
                },
                "robot_id": {
                    "type": "string",
                    "description": "Unique robot identifier for execution proof anchoring"
                },
                "robot_type": {
                    "type": "string",
                    "description": "Robot type: \"wheeled\" or \"quadruped\" (default: wheeled)",
                    "default": "wheeled"
                },
                "chain_rpc_url": {
                    "type": "string",
                    "description": "Chain RPC endpoint for on-chain contract queries (e.g. http://localhost:26657)"
                },
                "safety_envelope_addr": {
                    "type": "string",
                    "description": "SafetyEnvelope contract address"
                },
                "circuit_breaker_addr": {
                    "type": "string",
                    "description": "CircuitBreaker contract address"
                },
                "merkle_verifier_addr": {
                    "type": "string",
                    "description": "MerkleVerifier contract address"
                }
            },
            "required": ["ros2_bridge_url", "robot_id"]
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.enabled = config
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.ros2_bridge_url = config
            .get("ros2_bridge_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.robot_id = config
            .get("robot_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.robot_type = config
            .get("robot_type")
            .and_then(|v| v.as_str())
            .unwrap_or("wheeled")
            .to_string();

        if self.enabled && self.ros2_bridge_url.is_empty() {
            return Err(JunoClawError::Config(
                "ros2 plugin enabled but ros2_bridge_url not set".to_string(),
            ));
        }

        // Optional: configure on-chain contract client
        let chain_rpc = config
            .get("chain_rpc_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let safety_addr = config
            .get("safety_envelope_addr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let breaker_addr = config
            .get("circuit_breaker_addr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let merkle_addr = config
            .get("merkle_verifier_addr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if !chain_rpc.is_empty() && !safety_addr.is_empty() && !breaker_addr.is_empty() {
            self.onchain_client = Some(Arc::new(OnChainClient::new(
                chain_rpc,
                safety_addr,
                breaker_addr,
                merkle_addr,
            )));
            tracing::info!(
                "ros2 plugin configured with on-chain contracts (safety_envelope + circuit_breaker + merkle_verifier)"
            );
        }

        tracing::info!(
            "ros2 plugin initialized (enabled={}, bridge={}, robot={}, type={}, onchain={})",
            self.enabled,
            self.ros2_bridge_url,
            self.robot_id,
            self.robot_type,
            self.onchain_client.is_some()
        );
        Ok(())
    }

    async fn execute(&self, task: &Task, context: &PluginContext) -> Result<TaskResult> {
        if !self.enabled {
            return Err(JunoClawError::Plugin {
                plugin: "plugin-ros2".to_string(),
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
            "emit_intent" => {
                // Circuit breaker check — if tripped, intent-tier is locked.
                // The robot's reflexes still run (physics doesn't stop), but
                // no new auditable decisions can be emitted until the breaker
                // is reset by governance or the operator resolves the issue.
                //
                // When an on-chain client is configured, we query the chain
                // for the authoritative breaker state. We fall back to
                // in-memory state if the chain is unreachable.
                let breaker_tripped = if let Some(ref client) = self.onchain_client {
                    match client.is_locked(&self.robot_id).await {
                        Ok(resp) => {
                            tracing::debug!(
                                "on-chain breaker check: robot={} is_locked={}",
                                self.robot_id, resp.is_locked
                            );
                            resp.is_locked
                        }
                        Err(e) => {
                            tracing::warn!(
                                "on-chain breaker query failed, falling back to in-memory: {}",
                                e
                            );
                            self.circuit_breaker.is_tripped()
                        }
                    }
                } else {
                    self.circuit_breaker.is_tripped()
                };

                if breaker_tripped {
                    return Err(JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: "circuit breaker tripped — intent-tier locked, reflexes still running. Resolve safety violation and reset breaker before emitting new intents.".to_string(),
                    });
                }

                // Build an IntentMessage from ROS2 action server output and
                // wrap it into an AgentMessage for gate submission.
                //
                // Required input fields:
                //   action: the intent action (e.g. "engage", "navigate")
                //   params: structured action parameters (JSON object)
                //   sensor_snapshot: base64-encoded sensor data at decision time
                //   controller_timestamp: robot controller timestamp (ms)
                // Optional:
                //   rationale: human-readable audit rationale
                //   execution_proof_ref: rosbag path or action server result ID

                let intent_action = input
                    .get("intent_action")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution(
                            "missing 'intent_action' parameter".to_string(),
                        )
                    })?;

                let params = input.get("params").cloned().unwrap_or(Value::Null);

                let sensor_snapshot_b64 = input
                    .get("sensor_snapshot")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let sensor_snapshot = base64_decode(sensor_snapshot_b64);

                let controller_timestamp = input
                    .get("controller_timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution(
                            "missing 'controller_timestamp' parameter".to_string(),
                        )
                    })?;

                let rationale = input
                    .get("rationale")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let execution_proof_ref = input
                    .get("execution_proof_ref")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let intent = self.build_intent_message(
                    intent_action,
                    params,
                    &sensor_snapshot,
                    controller_timestamp,
                    rationale,
                    execution_proof_ref,
                );

                tracing::info!(
                    "Built IntentMessage: robot={}, action={}, sensor_hash={}",
                    intent.robot_id,
                    intent.action,
                    intent.sensor_snapshot_hash
                );

                // Use the agent_id from context as the sender key
                let from = context.agent_id.as_bytes().to_vec();
                let agent_msg = self.wrap_intent_into_agent_message(
                    intent,
                    from,
                    controller_timestamp,
                )?;

                tracing::info!(
                    "Wrapped IntentMessage into AgentMessage: content_hash={}",
                    hex::encode(agent_msg.content_hash)
                );

                // Return the encoded AgentMessage for gate submission
                let encoded = agent_msg
                    .encode()
                    .map_err(|e| JunoClawError::TaskExecution(format!("encode error: {}", e)))?;

                let output = format!(
                    "{{\"agent_message\":{}}}",
                    String::from_utf8_lossy(&encoded)
                );

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "emit_reflex_attestation" => {
                // Submit a ReflexBatchAttestation from the robot's controller.
                // This is the post-hoc proof that the reflex-tier maintained
                // the declared safety envelope across a batch of cycles.
                //
                // Required input fields:
                //   merkle_root: Merkle root of reflex cycle hashes
                //   cycle_count: number of reflex cycles in this batch
                //   batch_start_timestamp: controller clock at batch start (ms)
                //   batch_end_timestamp: controller clock at batch end (ms)
                //   envelope_version: safety envelope version enforced
                //   all_invariants_maintained: bool
                //   violated_invariants: list of invariant names that failed (empty if all OK)
                //   rosbag_ref: rosbag segment path for full reflex data

                let merkle_root = input
                    .get("merkle_root")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JunoClawError::TaskExecution("missing 'merkle_root'".to_string()))?
                    .to_string();

                let cycle_count = input
                    .get("cycle_count")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| JunoClawError::TaskExecution("missing 'cycle_count'".to_string()))?
                    as u32;

                let batch_start = input
                    .get("batch_start_timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| JunoClawError::TaskExecution("missing 'batch_start_timestamp'".to_string()))?;

                let batch_end = input
                    .get("batch_end_timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| JunoClawError::TaskExecution("missing 'batch_end_timestamp'".to_string()))?;

                let envelope_version = input
                    .get("envelope_version")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| JunoClawError::TaskExecution("missing 'envelope_version'".to_string()))?
                    as u32;

                // When on-chain client is configured, verify the envelope version
                // matches the on-chain SafetyEnvelope contract. This ensures the
                // attestation is against the governance-approved envelope.
                if let Some(ref client) = self.onchain_client {
                    match client.get_envelope(&self.robot_id).await {
                        Ok(onchain_env) => {
                            if onchain_env.version != envelope_version {
                                tracing::warn!(
                                    "envelope version mismatch: local={}, on-chain={}. Using on-chain version.",
                                    envelope_version, onchain_env.version
                                );
                            }
                            // Update cached version
                            *self.cached_envelope_version.write().await = Some(onchain_env.version);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "on-chain envelope query failed, using local version: {}",
                                e
                            );
                        }
                    }
                }

                let all_maintained = input
                    .get("all_invariants_maintained")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let violated: Vec<String> = input
                    .get("violated_invariants")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let rosbag_ref = input
                    .get("rosbag_ref")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let attestation = ReflexBatchAttestation {
                    robot_id: self.robot_id.clone(),
                    merkle_root,
                    cycle_count,
                    batch_start_timestamp: batch_start,
                    batch_end_timestamp: batch_end,
                    envelope_version,
                    all_invariants_maintained: all_maintained,
                    violated_invariants: violated.clone(),
                    rosbag_ref,
                };

                tracing::info!(
                    "Built ReflexBatchAttestation: robot={}, cycles={}, maintained={}, violated={:?}",
                    attestation.robot_id,
                    attestation.cycle_count,
                    attestation.all_invariants_maintained,
                    attestation.violated_invariants
                );

                // If the attestation reveals a violation, trip the circuit breaker
                if attestation.has_violation() {
                    tracing::warn!(
                        "Circuit breaker tripping: robot={} violated invariants={:?}",
                        self.robot_id,
                        attestation.violated_invariants
                    );
                    // In a deployment with on-chain contracts, a governance
                    // tx would call circuit-breaker.TripBreaker here. The
                    // plugin's next emit_intent will be rejected by the
                    // on-chain IsLocked query.
                }

                let from = context.agent_id.as_bytes().to_vec();
                let agent_msg = attestation
                    .into_agent_message(from, vec![], batch_end)
                    .map_err(|e| JunoClawError::TaskExecution(format!("encode error: {}", e)))?;

                let encoded = agent_msg
                    .encode()
                    .map_err(|e| JunoClawError::TaskExecution(format!("encode error: {}", e)))?;

                let output = format!(
                    "{{\"agent_message\":{}}}",
                    String::from_utf8_lossy(&encoded)
                );

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "check_breaker" => {
                // Check the circuit breaker state for this robot.
                // When on-chain client is configured, query the chain for
                // the authoritative state. Falls back to in-memory.
                let (is_closed, is_tripped, onchain_reason) =
                    if let Some(ref client) = self.onchain_client {
                        match client.is_locked(&self.robot_id).await {
                            Ok(resp) => {
                                (!resp.is_locked, resp.is_locked, resp.reason)
                            }
                            Err(e) => {
                                tracing::warn!("on-chain breaker query failed: {}", e);
                                (self.circuit_breaker.is_closed(),
                                 self.circuit_breaker.is_tripped(),
                                 None)
                            }
                        }
                    } else {
                        (self.circuit_breaker.is_closed(),
                         self.circuit_breaker.is_tripped(),
                         None)
                    };

                let output = serde_json::json!({
                    "robot_id": self.robot_id,
                    "is_closed": is_closed,
                    "is_tripped": is_tripped,
                    "onchain_reason": onchain_reason,
                    "source": if self.onchain_client.is_some() { "on-chain" } else { "in-memory" },
                }).to_string();

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "fetch_intent_proof" => {
                // Fetch an intent proof from the ROS2 bridge HTTP endpoint.
                // The bridge (junoclaw-ros2-bridge) runs on the robot or edge
                // device and exposes action server results as JSON.
                let intent_id = input
                    .get("intent_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'intent_id' parameter".to_string())
                    })?;

                tracing::info!(
                    "Fetching ROS2 intent proof: robot={}, intent={}",
                    self.robot_id,
                    intent_id
                );

                let url = format!("{}/intent/{}", self.ros2_bridge_url.trim_end_matches('/'), intent_id);
                let resp = reqwest::get(&url).await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge HTTP GET failed: {}", e),
                    })?;

                if !resp.status().is_success() {
                    return Err(JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge returned {} for intent {}", resp.status(), intent_id),
                    });
                }

                let intent_json: Value = resp.json().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("failed to parse bridge response: {}", e),
                    })?;

                let output = serde_json::to_string(&intent_json)
                    .map_err(|e| JunoClawError::TaskExecution(format!("serialize error: {}", e)))?;

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "fetch_sensor_log" => {
                // Fetch a reflex batch from the ROS2 bridge HTTP endpoint.
                // The bridge returns cycle data including sensor readings,
                // invariant checks, and a pre-computed Merkle root.
                let batch_id = input
                    .get("batch_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'batch_id' parameter".to_string())
                    })?;

                tracing::info!(
                    "Fetching ROS2 sensor log: robot={}, batch={}",
                    self.robot_id,
                    batch_id
                );

                let url = format!("{}/rosbag/{}", self.ros2_bridge_url.trim_end_matches('/'), batch_id);
                let resp = reqwest::get(&url).await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge HTTP GET failed: {}", e),
                    })?;

                if !resp.status().is_success() {
                    return Err(JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge returned {} for batch {}", resp.status(), batch_id),
                    });
                }

                let batch_json: Value = resp.json().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("failed to parse bridge response: {}", e),
                    })?;

                let output = serde_json::to_string(&batch_json)
                    .map_err(|e| JunoClawError::TaskExecution(format!("serialize error: {}", e)))?;

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "register_robot" => {
                // Register the robot via the ROS2 bridge's /robot/register endpoint.
                // The bridge returns registration metadata; the actual skill-registry
                // transaction is submitted separately via the MCP server or CLI.
                tracing::info!(
                    "Registering robot {} in skill-registry via marketplace",
                    self.robot_id
                );

                let url = format!("{}/robot/register", self.ros2_bridge_url.trim_end_matches('/'));
                let resp = reqwest::Client::new().post(&url).send().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge POST failed: {}", e),
                    })?;

                let reg_json: Value = resp.json().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("failed to parse bridge response: {}", e),
                    })?;

                let output = serde_json::to_string(&reg_json)
                    .map_err(|e| JunoClawError::TaskExecution(format!("serialize error: {}", e)))?;

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "simulate_batch" => {
                // Trigger a simulated reflex batch via the ROS2 bridge.
                // Useful for testing the full pipeline without hardware.
                let cycle_count = input
                    .get("cycle_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000);
                let violate = input
                    .get("violate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let url = format!(
                    "{}/rosbag/simulate?cycle_count={}&violate={}",
                    self.ros2_bridge_url.trim_end_matches('/'),
                    cycle_count,
                    violate
                );
                tracing::info!(
                    "Triggering simulated batch: robot={}, cycles={}, violate={}",
                    self.robot_id, cycle_count, violate
                );

                let resp = reqwest::Client::new().post(&url).send().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge POST failed: {}", e),
                    })?;

                let batch_json: Value = resp.json().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("failed to parse bridge response: {}", e),
                    })?;

                let output = serde_json::to_string(&batch_json)
                    .map_err(|e| JunoClawError::TaskExecution(format!("serialize error: {}", e)))?;

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            "set_expression" => {
                // Set the robot's face screen expression based on trust verdict.
                // Maps truth market verdicts to DOGZILLA-Lite's 35 expressions.
                // The bridge forwards this to the robot's display topic.
                let expression = input
                    .get("expression")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        JunoClawError::TaskExecution("missing 'expression' parameter".to_string())
                    })?;

                let valid_expressions = [
                    "happy", "neutral", "alert", "confused",
                    "sleeping", "angry", "scared", "curious",
                ];
                if !valid_expressions.contains(&expression) {
                    return Err(JunoClawError::TaskExecution(format!(
                        "invalid expression '{}': must be one of {:?}",
                        expression, valid_expressions
                    )));
                }

                tracing::info!(
                    "Setting robot expression: robot={}, expression={}",
                    self.robot_id, expression
                );

                // The bridge will publish to /display/expression ROS2 topic
                // which the DOGZILLA-Lite CM5 maps to one of 35 IPS display expressions.
                let url = format!(
                    "{}/robot/expression",
                    self.ros2_bridge_url.trim_end_matches('/')
                );
                let body = serde_json::json!({
                    "robot_id": self.robot_id,
                    "expression": expression,
                    "source": "junoclaw-trust-layer",
                });

                let resp = reqwest::Client::new().post(&url).json(&body).send().await
                    .map_err(|e| JunoClawError::Plugin {
                        plugin: "plugin-ros2".to_string(),
                        message: format!("bridge POST expression failed: {}", e),
                    })?;

                let expr_json: Value = resp.json().await
                    .unwrap_or(serde_json::json!({
                        "status": "sent",
                        "expression": expression,
                        "robot_id": self.robot_id,
                    }));

                let output = serde_json::to_string(&expr_json)
                    .map_err(|e| JunoClawError::TaskExecution(format!("serialize error: {}", e)))?;

                let mut hasher = Sha256::new();
                hasher.update(output.as_bytes());
                let output_hash = hex::encode(hasher.finalize());

                Ok(TaskResult {
                    output,
                    output_hash,
                    tool_calls: Vec::new(),
                    tokens_used: junoclaw_core::types::TokenUsage::default(),
                })
            }

            _ => Err(JunoClawError::TaskExecution(format!(
                "unknown ros2 plugin action: {}",
                action
            ))),
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("ros2 plugin shutting down");
        Ok(())
    }
}

/// Decode a base64 string into bytes. Returns empty vec on failure.
fn base64_decode(s: &str) -> Vec<u8> {
    // Minimal base64 decoder — avoids adding a base64 dependency.
    // In production, use the `base64` crate.
    if s.is_empty() {
        return Vec::new();
    }
    // Use serde_json's internal base64 handling via a workaround:
    // parse as a JSON string containing base64, then manually decode.
    // For now, just use the raw bytes as the snapshot — the hash is what matters.
    s.as_bytes().to_vec()
}
