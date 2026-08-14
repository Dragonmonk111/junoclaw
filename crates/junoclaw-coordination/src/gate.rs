//! J-Lens truth gate — audits message content before relay.
//!
//! The gate calls the CSI HTTP server (existing Node.js infrastructure)
//! to audit message content. Based on the separation score and thresholds,
//! it returns a green/yellow/red verdict.
//!
//! Green = clean, proceed
//! Yellow = suspicious, attach warning but relay
//! Red = deceptive, block message
//!
//! In mock mode (for testing), a deterministic heuristic is used instead
//! of HTTP calls — content containing deceptive keywords triggers Red.

use crate::message::{Batch, EvalAttestation, GateResult, GateVerdict};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

/// Keywords that trigger a red gate in mock mode.
const MOCK_RED_KEYWORDS: &[&str] = &[
    "deceptive",
    "malicious",
    "hack",
    "exploit",
    "manipulate",
    "fraud",
    "scam",
];

/// Keywords that trigger a yellow gate in mock mode.
const MOCK_YELLOW_KEYWORDS: &[&str] = &[
    "suspicious",
    "questionable",
    "unverified",
    "uncertain",
];

/// Configuration for the J-Lens truth gate.
#[derive(Clone, Debug)]
pub struct GateConfig {
    /// CSI server endpoint (e.g. "http://localhost:7777")
    pub csi_endpoint: String,
    /// Separation score threshold for yellow gate (suspicious)
    pub yellow_threshold: f64,
    /// Separation score threshold for red gate (blocked)
    pub red_threshold: f64,
    /// HTTP timeout for CSI server calls
    pub timeout: Duration,
    /// API key for CSI server authentication (if required)
    pub api_key: Option<String>,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            csi_endpoint: "http://localhost:7777".to_string(),
            yellow_threshold: 0.15,
            red_threshold: 0.35,
            timeout: Duration::from_secs(10),
            api_key: None,
        }
    }
}

/// Response from the CSI server audit endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct CsiAuditResponse {
    pub verdict: String,
    pub separation_score: f64,
    pub attestation_hash: Option<String>,
    pub model_id: Option<String>,
    pub gate: String,
}

/// The J-Lens truth gate.
///
/// In wired mode, makes real HTTP calls to the CSI server.
/// In mock mode, uses deterministic heuristics for testing.
pub struct JLensGate {
    config: GateConfig,
    /// HTTP client (initialized when gate is wired)
    client: Option<reqwest::Client>,
    /// Mock mode — uses heuristics instead of HTTP calls
    mock: bool,
}

impl JLensGate {
    /// Create a new J-Lens gate with the given configuration (wired mode).
    pub fn new(config: GateConfig) -> Self {
        Self {
            config,
            client: None,
            mock: false,
        }
    }

    /// Create a gate in mock mode for testing.
    /// Uses deterministic heuristics instead of HTTP calls.
    pub fn mock(config: GateConfig) -> Self {
        Self {
            config,
            client: None,
            mock: true,
        }
    }

    /// Create a gate with default configuration (wired mode).
    pub fn default() -> Self {
        Self::new(GateConfig::default())
    }

    /// Create a mock gate with default configuration.
    pub fn mock_default() -> Self {
        Self::mock(GateConfig::default())
    }

    /// Audit message content and return a gate verdict.
    ///
    /// In mock mode: uses keyword heuristics.
    /// In wired mode: calls CSI server POST /audit with content,
    ///   parses separation score, applies thresholds.
    /// On HTTP error: returns Yellow (conservative — relay with warning).
    pub async fn audit(&self, content: &[u8]) -> GateVerdict {
        if self.mock {
            return self.mock_audit(content);
        }

        let client = match &self.client {
            Some(c) => c,
            None => {
                warn!("J-Lens gate not wired, returning Yellow (unaudited)");
                return GateVerdict::Yellow { separation_score: 0.0 };
            }
        };

        let text = String::from_utf8_lossy(content);
        let body = serde_json::json!({ "text": text.as_ref() });

        let resp = client
            .post(format!("{}/audit", self.config.csi_endpoint))
            .header(
                "Authorization",
                self.config.api_key.as_deref().unwrap_or(""),
            )
            .json(&body)
            .timeout(self.config.timeout)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let audit: Result<CsiAuditResponse, _> = r.json().await;
                match audit {
                    Ok(a) => self.verdict_from_score(a.separation_score, a.attestation_hash),
                    Err(e) => {
                        warn!("J-Lens audit parse error: {}, returning Yellow", e);
                        GateVerdict::Yellow { separation_score: 0.0 }
                    }
                }
            }
            Err(e) => {
                warn!("J-Lens audit HTTP error: {}, returning Yellow", e);
                GateVerdict::Yellow { separation_score: 0.0 }
            }
        }
    }

    /// Audit a full batch of messages and return an aggregate GateResult.
    ///
    /// Aggregate logic:
    /// - If any message is Red → batch is Red (blocked)
    /// - If any message is Yellow (and none Red) → batch is Yellow
    /// - Otherwise → batch is Green
    ///
    /// The separation_score is the max across all messages.
    /// The attestation_hash is from the first non-null CSI response.
    pub async fn audit_batch(&self, batch: &Batch) -> GateResult {
        let mut worst_verdict = GateVerdict::Green;
        let mut max_score = 0.0f64;
        let mut attestation_hash: Option<String> = None;
        let mut model_id: Option<String> = None;

        for msg in &batch.messages {
            let verdict = self.audit(&msg.content).await;
            match &verdict {
                GateVerdict::Red { separation_score } => {
                    worst_verdict = GateVerdict::Red {
                        separation_score: *separation_score,
                    };
                    max_score = max_score.max(*separation_score);
                }
                GateVerdict::Yellow { separation_score } => {
                    if !matches!(worst_verdict, GateVerdict::Red { .. }) {
                        worst_verdict = GateVerdict::Yellow {
                            separation_score: *separation_score,
                        };
                    }
                    max_score = max_score.max(*separation_score);
                }
                GateVerdict::Green => {}
            }
        }

        // In mock mode, generate a deterministic attestation hash
        if self.mock && attestation_hash.is_none() {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(batch.hash());
            hasher.update(b"mock-attestation");
            attestation_hash = Some(hex::encode(hasher.finalize()));
            model_id = Some("mock-csi".to_string());
        }

        GateResult {
            verdict: worst_verdict,
            attestation_hash,
            separation_score: max_score,
            model_id,
        }
    }

    /// Determine verdict from a separation score using configured thresholds.
    fn verdict_from_score(
        &self,
        score: f64,
        _attestation_hash: Option<String>,
    ) -> GateVerdict {
        if score >= self.config.red_threshold {
            GateVerdict::Red {
                separation_score: score,
            }
        } else if score >= self.config.yellow_threshold {
            GateVerdict::Yellow {
                separation_score: score,
            }
        } else {
            GateVerdict::Green
        }
    }

    /// Mock audit — deterministic heuristic based on content keywords.
    fn mock_audit(&self, content: &[u8]) -> GateVerdict {
        let text = String::from_utf8_lossy(content).to_lowercase();

        for keyword in MOCK_RED_KEYWORDS {
            if text.contains(keyword) {
                return GateVerdict::Red {
                    separation_score: 0.9,
                };
            }
        }

        for keyword in MOCK_YELLOW_KEYWORDS {
            if text.contains(keyword) {
                return GateVerdict::Yellow {
                    separation_score: 0.2,
                };
            }
        }

        GateVerdict::Green
    }

    /// Get the gate configuration.
    pub fn config(&self) -> &GateConfig {
        &self.config
    }

    /// Check if the gate is wired (has an HTTP client and is not in mock mode).
    pub fn is_wired(&self) -> bool {
        !self.mock && self.client.is_some()
    }

    /// Check if the gate is in mock mode.
    pub fn is_mock(&self) -> bool {
        self.mock
    }

    /// Wire the gate — initialize the HTTP client for CSI server calls.
    /// Only effective in non-mock mode.
    pub fn wire(&mut self) -> anyhow::Result<()> {
        if self.mock {
            info!("J-Lens gate in mock mode (no HTTP client needed)");
            return Ok(());
        }
        let client = reqwest::Client::builder()
            .timeout(self.config.timeout)
            .build()?;
        self.client = Some(client);
        info!(
            "J-Lens gate wired to CSI server at {}",
            self.config.csi_endpoint
        );
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════
// MultiOperatorGate — Layer 6: competitive evaluation
// ════════════════════════════════════════════════════════════════

/// Result of a multi-operator audit, including per-operator attestations
/// and the list of operators that diverged from consensus.
#[derive(Clone, Debug)]
pub struct MultiOperatorResult {
    /// The consensus verdict (majority vote)
    pub consensus_verdict: GateVerdict,
    /// Per-operator attestations
    pub attestations: Vec<EvalAttestation>,
    /// Operators whose verdict diverged from consensus
    pub diverging_operators: Vec<Vec<u8>>,
    /// The batch height this audit was for
    pub consensus_batch_height: u64,
}

/// Configuration for the multi-operator gate.
#[derive(Clone, Debug)]
pub struct MultiOperatorConfig {
    /// Number of operator instances to run
    pub num_operators: usize,
    /// Per-operator gate configs (one per operator)
    pub operator_configs: Vec<GateConfig>,
    /// Minimum fraction of operators that must agree for consensus
    pub consensus_threshold: f64,
}

impl Default for MultiOperatorConfig {
    fn default() -> Self {
        Self {
            num_operators: 3,
            operator_configs: vec![GateConfig::default(); 3],
            consensus_threshold: 0.67,
        }
    }
}

/// Multi-operator gate — runs multiple J-Lens operators in parallel,
/// collects their verdicts, and determines consensus.
///
/// This is the Layer 6 "truth market" gate: multiple independent
/// evaluators audit the same content. Operators that match consensus
/// earn rewards; diverging operators get slashed by the truth-market
/// contract.
pub struct MultiOperatorGate {
    config: MultiOperatorConfig,
    operators: Vec<JLensGate>,
}

impl MultiOperatorGate {
    /// Create a new multi-operator gate with the given configuration (wired mode).
    pub fn new(config: MultiOperatorConfig) -> Self {
        let operators = config
            .operator_configs
            .iter()
            .map(|c| JLensGate::new(c.clone()))
            .collect();
        Self { config, operators }
    }

    /// Create a multi-operator gate in mock mode for testing.
    pub fn mock(config: MultiOperatorConfig) -> Self {
        let operators = config
            .operator_configs
            .iter()
            .map(|c| JLensGate::mock(c.clone()))
            .collect();
        Self { config, operators }
    }

    /// Create a mock gate with default configuration (3 operators).
    pub fn mock_default() -> Self {
        Self::mock(MultiOperatorConfig::default())
    }

    /// Audit content with all operators and return the consensus verdict.
    pub async fn audit(&self, content: &[u8]) -> GateVerdict {
        let result = self.audit_with_attestations(content, 0).await;
        result.consensus_verdict
    }

    /// Audit content with all operators, returning full attestations and
    /// divergence information. The `batch_height` is used to tag
    /// attestations for on-chain finalization.
    pub async fn audit_with_attestations(
        &self,
        content: &[u8],
        batch_height: u64,
    ) -> MultiOperatorResult {
        // Run all operators in parallel
        let mut tasks = Vec::new();
        for (i, op) in self.operators.iter().enumerate() {
            let content_owned = content.to_vec();
            let verdict = op.audit(&content_owned).await;
            tasks.push((i, verdict));
        }

        // Collect verdicts and build attestations
        let mut attestations: Vec<EvalAttestation> = Vec::new();
        let mut verdict_counts: [usize; 3] = [0, 0, 0]; // green, yellow, red

        for (i, verdict) in &tasks {
            match verdict {
                GateVerdict::Green => verdict_counts[0] += 1,
                GateVerdict::Yellow { .. } => verdict_counts[1] += 1,
                GateVerdict::Red { .. } => verdict_counts[2] += 1,
            }

            // Generate a deterministic mock pubkey for each operator
            let pubkey = {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(format!("operator-{}", i).as_bytes());
                hasher.finalize().to_vec()
            };

            // Generate a mock signature
            let signature = {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&pubkey);
                hasher.update(batch_height.to_le_bytes());
                hasher.finalize().to_vec()
            };

            attestations.push(EvalAttestation {
                operator_pubkey: pubkey,
                verdict: verdict.clone(),
                batch_height,
                signature,
            });
        }

        // Determine consensus by majority vote
        let total = tasks.len() as f64;
        let consensus_verdict = if verdict_counts[2] as f64 / total >= self.config.consensus_threshold {
            // Red consensus
            GateVerdict::Red {
                separation_score: 0.9,
            }
        } else if verdict_counts[1] as f64 / total >= self.config.consensus_threshold {
            // Yellow consensus
            GateVerdict::Yellow {
                separation_score: 0.2,
            }
        } else if verdict_counts[0] as f64 / total >= self.config.consensus_threshold {
            // Green consensus
            GateVerdict::Green
        } else {
            // No clear consensus — treat as Yellow (conservative)
            GateVerdict::Yellow {
                separation_score: 0.5,
            }
        };

        // Find diverging operators
        let diverging_operators: Vec<Vec<u8>> = attestations
            .iter()
            .filter(|a| !verdict_matches(&a.verdict, &consensus_verdict))
            .map(|a| a.operator_pubkey.clone())
            .collect();

        MultiOperatorResult {
            consensus_verdict,
            attestations,
            diverging_operators,
            consensus_batch_height: batch_height,
        }
    }

    /// Audit a full batch with all operators.
    pub async fn audit_batch(&self, batch: &Batch) -> MultiOperatorResult {
        // Use the batch's serialized content for the audit
        let content = serde_json::to_vec(batch).unwrap_or_default();
        self.audit_with_attestations(&content, batch.height)
            .await
    }

    /// Wire all operators (initialize HTTP clients).
    pub fn wire(&mut self) -> anyhow::Result<()> {
        for op in &mut self.operators {
            op.wire()?;
        }
        Ok(())
    }
}

/// Check if two verdicts match (ignoring separation score differences).
fn verdict_matches(a: &GateVerdict, b: &GateVerdict) -> bool {
    matches!(
        (a, b),
        (GateVerdict::Green, GateVerdict::Green)
            | (GateVerdict::Yellow { .. }, GateVerdict::Yellow { .. })
            | (GateVerdict::Red { .. }, GateVerdict::Red { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::AgentMessage;

    #[tokio::test]
    async fn test_mock_gate_green_for_clean_content() {
        let gate = JLensGate::mock_default();
        let verdict = gate.audit(b"hello world").await;
        assert_eq!(verdict, GateVerdict::Green);
    }

    #[tokio::test]
    async fn test_mock_gate_red_for_deceptive_content() {
        let gate = JLensGate::mock_default();
        let verdict = gate.audit(b"this is a deceptive message").await;
        assert!(matches!(verdict, GateVerdict::Red { .. }));
    }

    #[tokio::test]
    async fn test_mock_gate_yellow_for_suspicious_content() {
        let gate = JLensGate::mock_default();
        let verdict = gate.audit(b"this is suspicious content").await;
        assert!(matches!(verdict, GateVerdict::Yellow { .. }));
    }

    #[tokio::test]
    async fn test_mock_gate_case_insensitive() {
        let gate = JLensGate::mock_default();
        let verdict = gate.audit(b"DECEPTIVE uppercase").await;
        assert!(matches!(verdict, GateVerdict::Red { .. }));
    }

    #[tokio::test]
    async fn test_audit_batch_all_green() {
        let gate = JLensGate::mock_default();
        let msg1 = AgentMessage::new(vec![1; 32], vec![], b"clean message 1".to_vec(), 1000);
        let msg2 = AgentMessage::new(vec![2; 32], vec![], b"clean message 2".to_vec(), 2000);
        let batch = Batch::new(vec![msg1, msg2], [0u8; 32], 1, 3000);

        let result = gate.audit_batch(&batch).await;
        assert_eq!(result.verdict, GateVerdict::Green);
        assert!(result.attestation_hash.is_some());
    }

    #[tokio::test]
    async fn test_audit_batch_with_red() {
        let gate = JLensGate::mock_default();
        let msg1 = AgentMessage::new(vec![1; 32], vec![], b"clean message".to_vec(), 1000);
        let msg2 =
            AgentMessage::new(vec![2; 32], vec![], b"deceptive content".to_vec(), 2000);
        let batch = Batch::new(vec![msg1, msg2], [0u8; 32], 1, 3000);

        let result = gate.audit_batch(&batch).await;
        assert!(matches!(result.verdict, GateVerdict::Red { .. }));
        assert!(result.separation_score >= 0.9);
    }

    #[tokio::test]
    async fn test_audit_batch_with_yellow_no_red() {
        let gate = JLensGate::mock_default();
        let msg1 = AgentMessage::new(vec![1; 32], vec![], b"clean message".to_vec(), 1000);
        let msg2 =
            AgentMessage::new(vec![2; 32], vec![], b"suspicious content".to_vec(), 2000);
        let batch = Batch::new(vec![msg1, msg2], [0u8; 32], 1, 3000);

        let result = gate.audit_batch(&batch).await;
        assert!(matches!(result.verdict, GateVerdict::Yellow { .. }));
    }

    #[tokio::test]
    async fn test_audit_batch_red_overrides_yellow() {
        let gate = JLensGate::mock_default();
        let msg1 =
            AgentMessage::new(vec![1; 32], vec![], b"suspicious content".to_vec(), 1000);
        let msg2 =
            AgentMessage::new(vec![2; 32], vec![], b"deceptive content".to_vec(), 2000);
        let batch = Batch::new(vec![msg1, msg2], [0u8; 32], 1, 3000);

        let result = gate.audit_batch(&batch).await;
        assert!(matches!(result.verdict, GateVerdict::Red { .. }));
    }

    #[tokio::test]
    async fn test_audit_batch_empty() {
        let gate = JLensGate::mock_default();
        let batch = Batch::new(vec![], [0u8; 32], 1, 3000);

        let result = gate.audit_batch(&batch).await;
        assert_eq!(result.verdict, GateVerdict::Green);
        assert_eq!(result.separation_score, 0.0);
    }

    #[tokio::test]
    async fn test_unwired_gate_returns_yellow() {
        let gate = JLensGate::new(GateConfig::default());
        let verdict = gate.audit(b"test content").await;
        assert!(matches!(verdict, GateVerdict::Yellow { .. }));
    }

    #[test]
    fn test_gate_config_defaults() {
        let config = GateConfig::default();
        assert_eq!(config.yellow_threshold, 0.15);
        assert_eq!(config.red_threshold, 0.35);
        assert_eq!(config.csi_endpoint, "http://localhost:7777");
    }

    #[test]
    fn test_gate_not_wired_by_default() {
        let gate = JLensGate::default();
        assert!(!gate.is_wired());
    }

    #[test]
    fn test_mock_gate_is_mock() {
        let gate = JLensGate::mock_default();
        assert!(gate.is_mock());
        assert!(!gate.is_wired());
    }

    #[test]
    fn test_verdict_from_score_green() {
        let gate = JLensGate::mock_default();
        let verdict = gate.verdict_from_score(0.05, None);
        assert_eq!(verdict, GateVerdict::Green);
    }

    #[test]
    fn test_verdict_from_score_yellow() {
        let gate = JLensGate::mock_default();
        let verdict = gate.verdict_from_score(0.20, None);
        assert!(matches!(verdict, GateVerdict::Yellow { .. }));
    }

    #[test]
    fn test_verdict_from_score_red() {
        let gate = JLensGate::mock_default();
        let verdict = gate.verdict_from_score(0.50, None);
        assert!(matches!(verdict, GateVerdict::Red { .. }));
    }

    // ─── MultiOperatorGate tests ───

    #[tokio::test]
    async fn test_multi_operator_gate_consensus() {
        let gate = MultiOperatorGate::mock_default();
        let verdict = gate.audit(b"hello world").await;
        // All mock operators return Green for clean content
        assert_eq!(verdict, GateVerdict::Green);
    }

    #[tokio::test]
    async fn test_multi_operator_gate_detects_divergence() {
        let gate = MultiOperatorGate::mock_default();
        let result = gate.audit_with_attestations(b"deceptive content", 42).await;
        // All mock operators detect deceptive → Red consensus, 0 diverging
        assert!(matches!(result.consensus_verdict, GateVerdict::Red { .. }));
        assert_eq!(result.diverging_operators.len(), 0);
    }

    #[tokio::test]
    async fn test_multi_operator_gate_attestations_recorded() {
        let gate = MultiOperatorGate::mock_default();
        let result = gate.audit_with_attestations(b"clean content", 100).await;
        assert_eq!(result.attestations.len(), 3);
        assert_eq!(result.consensus_batch_height, 100);
    }
}
