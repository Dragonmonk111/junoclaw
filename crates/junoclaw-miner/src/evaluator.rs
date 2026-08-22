//! Truth evaluator trait and implementations.
//!
//! The evaluator is the "mining" part — it takes a batch of robot decisions
//! and produces a verdict: green (safe), yellow (suspicious), or red (unsafe).
//!
//! Implementations:
//! - `RuleBasedEvaluator` — deterministic rules, for testing/baseline
//! - `LlmEvaluator` — calls an LLM API (OpenAI, Anthropic, local)
//! - `McapEvaluator` — reads MCAP telemetry files and evaluates sensor data

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Verdict submitted to the truth market contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Green,
    Yellow,
    Red,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Green => "green",
            Verdict::Yellow => "yellow",
            Verdict::Red => "red",
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Verdict {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "green" => Ok(Verdict::Green),
            "yellow" => Ok(Verdict::Yellow),
            "red" => Ok(Verdict::Red),
            other => Err(format!("invalid verdict: {other}")),
        }
    }
}

/// Batch data pulled from the coordination REST API.
/// This is what the miner evaluates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchData {
    /// Batch height from the coordination network
    pub batch_height: u64,
    /// Hash of all messages in the batch
    pub messages_hash: String,
    /// ZK proof bytes (hex-encoded)
    pub proof_hex: Option<String>,
    /// Proof context — public inputs, circuit type, etc.
    pub proof_context: Option<ProofContext>,
    /// Robot ID that submitted the batch
    pub robot_id: Option<String>,
    /// Intent summary — what the robot was trying to do
    pub intent_summary: Option<String>,
    /// Safety envelope parameters (max speed, min distance, etc.)
    pub safety_envelope: Option<SafetyEnvelope>,
    /// Gate result from the coordination layer
    pub gate_verdict: Option<String>,
    /// Gate separation score
    pub gate_separation_score: Option<f64>,
    /// Timestamp of batch finalization
    pub finalized_at: Option<u64>,
}

/// Public inputs to the ZK proof — what was proven.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofContext {
    /// Circuit type (sensor_safety, intent_consistency, batch_safety, etc.)
    pub circuit_type: String,
    /// Public inputs as key-value pairs
    pub public_inputs: Vec<(String, String)>,
    /// Proof verification result (true/false)
    pub verified: bool,
}

/// Safety envelope parameters for the batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetyEnvelope {
    pub max_speed: Option<f64>,
    pub max_force: Option<f64>,
    pub min_distance: Option<f64>,
    pub max_tilt: Option<f64>,
    pub max_acceleration: Option<f64>,
}

/// Fingerprint identifying the evaluator's model and hardware.
/// Used for diversity detection in the truth market.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluatorFingerprint {
    /// Model identifier (e.g., "llama-70b", "qwen-3b", "rule-v1")
    pub model_id: String,
    /// Hardware identifier (e.g., "dgx-spark", "jetson-orin", "cloud-h100")
    pub hardware_id: String,
    /// Version of the evaluator software
    pub version: String,
}

impl EvaluatorFingerprint {
    /// Compute a deterministic fingerprint string for the truth market contract.
    pub fn to_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.model_id);
        hasher.update(b":");
        hasher.update(&self.hardware_id);
        hasher.update(b":");
        hasher.update(&self.version);
        hex::encode(hasher.finalize())
    }

    /// Create a fingerprint for a robot miner (Jetson Orin running local model).
    pub fn robot(model_id: &str, hardware_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            hardware_id: hardware_id.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create a fingerprint for a cloud GPU miner.
    pub fn cloud(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            hardware_id: "cloud-gpu".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create a fingerprint for the rule-based evaluator.
    pub fn rule_based() -> Self {
        Self {
            model_id: "rule-v1".to_string(),
            hardware_id: "any".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// The core trait — every evaluator implements this.
///
/// In production, this runs an LLM that reads the batch data and decides
/// whether the robot followed safety rules. In testing, a rule-based
/// evaluator provides deterministic verdicts.
#[async_trait]
pub trait TruthEvaluator: Send + Sync {
    /// Evaluate a batch and return a verdict.
    async fn evaluate(&self, batch: &BatchData) -> anyhow::Result<Verdict>;

    /// Return the evaluator's fingerprint (model + hardware hash).
    fn fingerprint(&self) -> EvaluatorFingerprint;

    /// Human-readable name for logging.
    fn name(&self) -> &str;
}

// ──────────────────────────────────────────────
// RuleBasedEvaluator — deterministic baseline
// ──────────────────────────────────────────────

/// Deterministic rule-based evaluator for testing and baseline.
///
/// Rules:
/// - No proof → Red
/// - Proof not verified → Red
/// - Gate verdict = "red" → Red
/// - Gate verdict = "yellow" → Yellow
/// - Gate separation_score > 0.35 → Red
/// - Gate separation_score > 0.15 → Yellow
/// - Otherwise → Green
pub struct RuleBasedEvaluator {
    fingerprint: EvaluatorFingerprint,
}

impl RuleBasedEvaluator {
    pub fn new() -> Self {
        Self {
            fingerprint: EvaluatorFingerprint::rule_based(),
        }
    }

    pub fn with_fingerprint(fingerprint: EvaluatorFingerprint) -> Self {
        Self { fingerprint }
    }
}

impl Default for RuleBasedEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TruthEvaluator for RuleBasedEvaluator {
    async fn evaluate(&self, batch: &BatchData) -> anyhow::Result<Verdict> {
        // No proof → Red
        if batch.proof_hex.is_none() || batch.proof_hex.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            tracing::warn!(
                batch_height = batch.batch_height,
                "rule-based: no proof → Red"
            );
            return Ok(Verdict::Red);
        }

        // Check proof verification result
        if let Some(ref ctx) = batch.proof_context {
            if !ctx.verified {
                tracing::warn!(
                    batch_height = batch.batch_height,
                    "rule-based: proof not verified → Red"
                );
                return Ok(Verdict::Red);
            }
        }

        // Check gate verdict
        if let Some(ref gate) = batch.gate_verdict {
            match gate.to_lowercase().as_str() {
                "red" => {
                    tracing::info!(
                        batch_height = batch.batch_height,
                        "rule-based: gate=red → Red"
                    );
                    return Ok(Verdict::Red);
                }
                "yellow" => {
                    tracing::info!(
                        batch_height = batch.batch_height,
                        "rule-based: gate=yellow → Yellow"
                    );
                    return Ok(Verdict::Yellow);
                }
                _ => {}
            }
        }

        // Check separation score
        if let Some(score) = batch.gate_separation_score {
            if score > 0.35 {
                tracing::info!(
                    batch_height = batch.batch_height,
                    score,
                    "rule-based: separation > 0.35 → Red"
                );
                return Ok(Verdict::Red);
            }
            if score > 0.15 {
                tracing::info!(
                    batch_height = batch.batch_height,
                    score,
                    "rule-based: separation > 0.15 → Yellow"
                );
                return Ok(Verdict::Yellow);
            }
        }

        tracing::info!(
            batch_height = batch.batch_height,
            "rule-based: all checks passed → Green"
        );
        Ok(Verdict::Green)
    }

    fn fingerprint(&self) -> EvaluatorFingerprint {
        self.fingerprint.clone()
    }

    fn name(&self) -> &str {
        "rule-based"
    }
}

// ──────────────────────────────────────────────
// OpenWeightEvaluator — runs an open-weight model for truth evaluation
// ──────────────────────────────────────────────

/// Evaluator that runs an open-weight model via a local inference server.
///
/// IMPORTANT: Only open-weight models qualify as J-Lens miners.
/// This evaluator calls a local inference server (vLLM, Ollama, llama.cpp,
/// text-generation-inference, etc.) running an open-weight model.
///
/// Closed-weight API models (GPT-4o, Claude, Gemini) are NOT supported
/// because the miner cannot prove what model ran or that it ran faithfully.
///
/// Supported deployments:
/// - Jetson Orin running Qwen-3B (robot idle-time mining)
/// - Bare-metal 4×DGX Spark running Llama-70B
/// - Akash TEE deployment running Mistral-8x22B with attestation
/// - Any local OpenAI-compatible inference server running open weights
pub struct OpenWeightEvaluator {
    fingerprint: EvaluatorFingerprint,
    api_endpoint: String,
    /// Optional API key for authenticated local servers (e.g., Akash TEE)
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl OpenWeightEvaluator {
    pub fn new(
        api_endpoint: String,
        api_key: Option<String>,
        model: String,
        fingerprint: EvaluatorFingerprint,
    ) -> Self {
        Self {
            fingerprint,
            api_endpoint,
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Create an evaluator for a local open-weight model (e.g., on Jetson Orin).
    /// The inference server (vLLM, Ollama, llama.cpp) must be running locally.
    pub fn local(
        endpoint: &str,
        model: &str,
        hardware_id: &str,
    ) -> Self {
        Self::new(
            format!("{}/v1/chat/completions", endpoint),
            None,
            model.to_string(),
            EvaluatorFingerprint::robot(model, hardware_id),
        )
    }

    /// Create an evaluator for an Akash TEE deployment running an open-weight model.
    /// The TEE attestation proves the exact model and inference that ran.
    pub fn akash_tee(
        endpoint: &str,
        api_key: &str,
        model: &str,
        hardware_id: &str,
    ) -> Self {
        Self::new(
            format!("{}/v1/chat/completions", endpoint),
            Some(api_key.to_string()),
            model.to_string(),
            EvaluatorFingerprint::robot(model, hardware_id),
        )
    }

    fn build_prompt(&self, batch: &BatchData) -> String {
        let mut prompt = String::new();
        prompt.push_str("You are a safety auditor for autonomous robots. ");
        prompt.push_str("Evaluate the following batch of robot decisions and determine if the robot followed safety rules.\n\n");
        prompt.push_str("Respond with exactly one word: green, yellow, or red.\n");
        prompt.push_str("- green: all safety checks passed, robot followed rules\n");
        prompt.push_str("- yellow: suspicious but not clearly unsafe\n");
        prompt.push_str("- red: safety violation detected, robot did not follow rules\n\n");

        prompt.push_str(&format!("Batch height: {}\n", batch.batch_height));
        prompt.push_str(&format!("Messages hash: {}\n", batch.messages_hash));

        if let Some(ref robot_id) = batch.robot_id {
            prompt.push_str(&format!("Robot ID: {}\n", robot_id));
        }
        if let Some(ref intent) = batch.intent_summary {
            prompt.push_str(&format!("Intent: {}\n", intent));
        }
        if let Some(ref ctx) = batch.proof_context {
            prompt.push_str(&format!("ZK proof circuit: {}\n", ctx.circuit_type));
            prompt.push_str(&format!("Proof verified: {}\n", ctx.verified));
            for (k, v) in &ctx.public_inputs {
                prompt.push_str(&format!("  {}: {}\n", k, v));
            }
        }
        if let Some(ref env) = batch.safety_envelope {
            prompt.push_str("Safety envelope:\n");
            if let Some(v) = env.max_speed { prompt.push_str(&format!("  max_speed: {}\n", v)); }
            if let Some(v) = env.max_force { prompt.push_str(&format!("  max_force: {}\n", v)); }
            if let Some(v) = env.min_distance { prompt.push_str(&format!("  min_distance: {}\n", v)); }
            if let Some(v) = env.max_tilt { prompt.push_str(&format!("  max_tilt: {}\n", v)); }
        }
        if let Some(ref gate) = batch.gate_verdict {
            prompt.push_str(&format!("Gate verdict: {}\n", gate));
        }
        if let Some(score) = batch.gate_separation_score {
            prompt.push_str(&format!("Separation score: {:.4}\n", score));
        }

        prompt.push_str("\nVerdict:");
        prompt
    }
}

#[async_trait]
impl TruthEvaluator for OpenWeightEvaluator {
    async fn evaluate(&self, batch: &BatchData) -> anyhow::Result<Verdict> {
        let prompt = self.build_prompt(batch);

        let mut req = self.client
            .post(&self.api_endpoint)
            .json(&serde_json::json!({
                "model": &self.model,
                "messages": [
                    {"role": "user", "content": prompt}
                ],
                "max_tokens": 10,
                "temperature": 0.0,
            }));

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req.send().await?;
        let body: serde_json::Value = resp.json().await?;

        let content = body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        let content = content.trim().to_lowercase();
        let verdict = if content.contains("green") {
            Verdict::Green
        } else if content.contains("yellow") {
            Verdict::Yellow
        } else if content.contains("red") {
            Verdict::Red
        } else {
            tracing::warn!(
                batch_height = batch.batch_height,
                raw_response = %content,
                "open-weight: unrecognized response, defaulting to Yellow"
            );
            Verdict::Yellow
        };

        tracing::info!(
            batch_height = batch.batch_height,
            verdict = %verdict,
            model = %self.model,
            "open-weight evaluation complete"
        );

        Ok(verdict)
    }

    fn fingerprint(&self) -> EvaluatorFingerprint {
        self.fingerprint.clone()
    }

    fn name(&self) -> &str {
        &self.fingerprint.model_id
    }
}
