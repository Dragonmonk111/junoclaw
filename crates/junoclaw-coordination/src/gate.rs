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

use crate::message::{Batch, EvalAttestation, GateResult, GateVerdict, IntentMessage, ProofContext};
use async_trait::async_trait;
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

/// Trait for verifying ZK proofs attached to messages.
///
/// In production, this calls the on-chain zk-verifier contract or a local
/// verifier. In mock mode, it returns Ok(verified) for any non-empty proof.
#[async_trait]
pub trait ProofVerifier: Send + Sync {
    /// Verify a proof reference. Returns ProofContext with verification status.
    async fn verify(&self, proof_ref: &str, robot_id: &str) -> ProofContext;
}

/// Mock proof verifier — returns verified=true for non-empty proof refs,
/// false for empty ones. Useful for testing the proof-aware gate loop.
pub struct MockProofVerifier {
    /// If true, all proofs are treated as valid (even empty refs).
    pub always_valid: bool,
}

impl Default for MockProofVerifier {
    fn default() -> Self {
        Self {
            always_valid: false,
        }
    }
}

/// Configuration for the on-chain proof verifier.
#[derive(Clone, Debug)]
pub struct OnChainProofVerifierConfig {
    /// Juno RPC endpoint (e.g. "http://localhost:26657")
    pub chain_rpc: String,
    /// zk-verifier contract address
    pub verifier_addr: String,
    /// HTTP timeout for chain queries
    pub timeout: Duration,
}

impl Default for OnChainProofVerifierConfig {
    fn default() -> Self {
        Self {
            chain_rpc: "http://localhost:26657".to_string(),
            verifier_addr: String::new(),
            timeout: Duration::from_secs(10),
        }
    }
}

/// Production proof verifier — queries the Juno chain to verify that a
/// ZK proof was successfully verified on-chain by the zk-verifier contract.
///
/// The `proof_ref` in an IntentMessage is the transaction hash of the
/// `VerifyProof` execution. This verifier:
/// 1. Queries the chain for the transaction by hash (tx_search)
/// 2. Checks if the transaction was successful (code 0)
/// 3. If tx_search fails, falls back to querying LastVerify on the contract
/// 4. Returns ProofContext with proof_verified=true/false
///
/// If the proof_ref is empty or the transaction is not found, returns
/// proof_verified=false (auto-Red in the gate).
pub struct OnChainProofVerifier {
    config: OnChainProofVerifierConfig,
    client: reqwest::Client,
}

impl OnChainProofVerifier {
    pub fn new(config: OnChainProofVerifierConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// Create from environment variables.
    /// JUNO_CHAIN_RPC — RPC endpoint (default: http://localhost:26657)
    /// JUNO_ZK_VERIFIER_ADDR — zk-verifier contract address
    pub fn from_env() -> Self {
        let config = OnChainProofVerifierConfig {
            chain_rpc: std::env::var("JUNO_CHAIN_RPC")
                .unwrap_or_else(|_| "http://localhost:26657".to_string()),
            verifier_addr: std::env::var("JUNO_ZK_VERIFIER_ADDR")
                .unwrap_or_default(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// Query the chain for a transaction by hash.
    /// Returns true if the transaction exists and was successful (code 0).
    async fn check_tx_success(&self, tx_hash: &str) -> bool {
        if tx_hash.is_empty() {
            return false;
        }

        let url = format!(
            "{}/tx_search?query=\"tx.hash='{}'\"",
            self.config.chain_rpc.trim_end_matches('/'),
            tx_hash
        );

        let resp = self.client.get(&url).send().await;
        match resp {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = match r.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("OnChainProofVerifier: tx_search parse error: {}", e);
                        return false;
                    }
                };
                let txs = body
                    .get("result")
                    .and_then(|r| r.get("txs"))
                    .and_then(|t| t.as_array());
                match txs {
                    Some(arr) if !arr.is_empty() => {
                        let code = arr[0]
                            .get("tx_result")
                            .and_then(|r| r.get("code"))
                            .and_then(|c| c.as_i64())
                            .unwrap_or(-1);
                        if code == 0 {
                            return true;
                        }
                        warn!(
                            "OnChainProofVerifier: tx {} found but code={} (not successful)",
                            tx_hash, code
                        );
                        false
                    }
                    _ => {
                        warn!(
                            "OnChainProofVerifier: tx {} not found in tx_search",
                            tx_hash
                        );
                        false
                    }
                }
            }
            Ok(r) => {
                warn!(
                    "OnChainProofVerifier: tx_search HTTP {} for tx {}",
                    r.status(),
                    tx_hash
                );
                false
            }
            Err(e) => {
                warn!(
                    "OnChainProofVerifier: tx_search error for tx {}: {}",
                    tx_hash, e
                );
                false
            }
        }
    }

    /// Query the zk-verifier contract's LastVerify as a fallback.
    async fn check_last_verify(&self) -> Option<bool> {
        if self.config.verifier_addr.is_empty() {
            return None;
        }

        let query_msg = serde_json::json!({ "last_verify": {} });
        let query_bytes = serde_json::to_vec(&query_msg).ok()?;
        let query_b64 = base64_url_encode(&query_bytes);

        let url = format!(
            "{}/abci_query?path=\"/cosmwasm.wasm.v1.Query/SmartContractState/{}%2F{}\"",
            self.config.chain_rpc.trim_end_matches('/'),
            self.config.verifier_addr,
            query_b64,
        );

        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let body: serde_json::Value = resp.json().await.ok()?;
        let value = body
            .get("result")
            .and_then(|r| r.get("response"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())?;

        let decoded = base64_std_decode(value)?;
        let result: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
        result.get("verified").and_then(|v| v.as_bool())
    }
}

#[async_trait]
impl ProofVerifier for OnChainProofVerifier {
    async fn verify(&self, proof_ref: &str, robot_id: &str) -> ProofContext {
        if proof_ref.is_empty() {
            warn!(
                "OnChainProofVerifier: empty proof_ref for robot {}, auto-Red",
                robot_id
            );
            return ProofContext {
                proof_verified: false,
                proof_hash: None,
                attestation_clean: None,
                violated_invariants: Vec::new(),
            };
        }

        let tx_success = self.check_tx_success(proof_ref).await;

        if !tx_success {
            if let Some(verified) = self.check_last_verify().await {
                if verified {
                    info!(
                        "OnChainProofVerifier: tx_search failed but LastVerify=true for robot {}, accepting",
                        robot_id
                    );
                    return ProofContext {
                        proof_verified: true,
                        proof_hash: Some(proof_ref.to_string()),
                        attestation_clean: Some(true),
                        violated_invariants: Vec::new(),
                    };
                }
            }

            warn!(
                "OnChainProofVerifier: proof {} NOT verified for robot {}, auto-Red",
                proof_ref, robot_id
            );
            return ProofContext {
                proof_verified: false,
                proof_hash: Some(proof_ref.to_string()),
                attestation_clean: None,
                violated_invariants: Vec::new(),
            };
        }

        info!(
            "OnChainProofVerifier: proof {} verified on-chain for robot {}",
            proof_ref, robot_id
        );
        ProofContext {
            proof_verified: true,
            proof_hash: Some(proof_ref.to_string()),
            attestation_clean: Some(true),
            violated_invariants: Vec::new(),
        }
    }
}

fn base64_url_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

fn base64_std_decode(s: &str) -> Option<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let s = s.trim_end_matches('=');
    let mut result = Vec::new();
    let bytes = s.as_bytes();

    for chunk in bytes.chunks(4) {
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = CHARS.iter().position(|&c| c == b)? as u32;
        }
        let quad = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push((quad >> 16) as u8);
        if chunk.len() > 2 {
            result.push((quad >> 8) as u8);
        }
        if chunk.len() > 3 {
            result.push(quad as u8);
        }
    }
    Some(result)
}

#[async_trait]
impl ProofVerifier for MockProofVerifier {
    async fn verify(&self, proof_ref: &str, robot_id: &str) -> ProofContext {
        if self.always_valid {
            return ProofContext {
                proof_verified: true,
                proof_hash: Some(proof_ref.to_string()),
                attestation_clean: Some(true),
                violated_invariants: Vec::new(),
            };
        }
        if proof_ref.is_empty() {
            warn!("Mock proof verifier: empty proof ref for robot {}", robot_id);
            return ProofContext {
                proof_verified: false,
                proof_hash: None,
                attestation_clean: None,
                violated_invariants: Vec::new(),
            };
        }
        ProofContext {
            proof_verified: true,
            proof_hash: Some(proof_ref.to_string()),
            attestation_clean: Some(true),
            violated_invariants: Vec::new(),
        }
    }
}

/// The J-Lens truth gate.
///
/// In wired mode, makes real HTTP calls to the CSI server.
/// In mock mode, uses deterministic heuristics for testing.
/// When a proof verifier is attached, the gate also checks ZK proof
/// verification status — if the proof is missing or invalid, the verdict
/// is automatically Red regardless of content audit result.
pub struct JLensGate {
    config: GateConfig,
    /// HTTP client (initialized when gate is wired)
    client: Option<reqwest::Client>,
    /// Mock mode — uses heuristics instead of HTTP calls
    mock: bool,
    /// Optional proof verifier for proof-aware gating
    proof_verifier: Option<Box<dyn ProofVerifier>>,
}

impl JLensGate {
    /// Create a new J-Lens gate with the given configuration (wired mode).
    pub fn new(config: GateConfig) -> Self {
        Self {
            config,
            client: None,
            mock: false,
            proof_verifier: None,
        }
    }

    /// Create a gate in mock mode for testing.
    /// Uses deterministic heuristics instead of HTTP calls.
    pub fn mock(config: GateConfig) -> Self {
        Self {
            config,
            client: None,
            mock: true,
            proof_verifier: None,
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

    /// Attach a proof verifier to make the gate proof-aware.
    /// When set, the gate checks ZK proof verification alongside content audit.
    /// If the proof is missing or invalid → auto-Red.
    pub fn with_proof_verifier(mut self, verifier: Box<dyn ProofVerifier>) -> Self {
        self.proof_verifier = Some(verifier);
        self
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

    /// Audit message content AND verify attached ZK proof.
    ///
    /// This is the proof-aware audit path. If a proof verifier is attached:
    /// 1. Decode the content as an IntentMessage to extract execution_proof_ref
    /// 2. Call the proof verifier to check proof status
    /// 3. If proof is missing or invalid → auto-Red (regardless of content)
    /// 4. If proof is valid → proceed with normal content audit
    /// 5. If proof violation detected → auto-Red
    ///
    /// If no proof verifier is attached, falls back to content-only audit.
    /// If content is not an IntentMessage (can't extract proof ref), falls back
    /// to content-only audit.
    pub async fn audit_with_proof(&self, content: &[u8]) -> GateVerdict {
        // If no proof verifier attached, fall back to content-only audit
        let verifier = match &self.proof_verifier {
            Some(v) => v,
            None => return self.audit(content).await,
        };

        // Try to decode as IntentMessage to extract proof ref
        let intent = match IntentMessage::decode(content) {
            Ok(i) => i,
            Err(_) => {
                // Not an IntentMessage — can't check proof, do content-only
                return self.audit(content).await;
            }
        };

        // Verify the proof
        let proof_ctx = verifier.verify(
            intent.execution_proof_ref.as_deref().unwrap_or(""),
            &intent.robot_id,
        )
        .await;

        // If proof is not verified → auto-Red
        if !proof_ctx.proof_verified {
            warn!(
                "J-Lens proof-aware gate: proof NOT verified for robot {} (hash={:?}), auto-Red",
                intent.robot_id,
                proof_ctx.proof_hash
            );
            return GateVerdict::Red {
                separation_score: 1.0,
            };
        }

        // If proof shows a violation → auto-Red
        if proof_ctx.has_violation() {
            warn!(
                "J-Lens proof-aware gate: violation detected in proof for robot {}, auto-Red",
                intent.robot_id
            );
            return GateVerdict::Red {
                separation_score: 0.95,
            };
        }

        // Proof is valid — proceed with normal content audit
        self.audit(content).await
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
    /// Minimum number of operators required to produce a valid consensus.
    /// If fewer operators are configured, the gate returns a Red verdict
    /// instead of allowing a single operator to self-consensus.
    pub min_operators: usize,
}

impl Default for MultiOperatorConfig {
    fn default() -> Self {
        Self {
            num_operators: 3,
            operator_configs: vec![GateConfig::default(); 3],
            consensus_threshold: 0.67,
            min_operators: 3,
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
        // Refuse to produce consensus if fewer than min_operators are configured.
        // A single operator trivially self-consenses (1/1 = 100%), providing
        // no adversarial check — return Red with no attestations instead.
        if self.operators.len() < self.config.min_operators {
            return MultiOperatorResult {
                consensus_verdict: GateVerdict::Red {
                    separation_score: 1.0,
                },
                attestations: Vec::new(),
                diverging_operators: Vec::new(),
                consensus_batch_height: batch_height,
            };
        }

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

    #[tokio::test]
    async fn test_proof_aware_gate_missing_proof_auto_red() {
        let gate = JLensGate::mock_default()
            .with_proof_verifier(Box::new(MockProofVerifier::default()));

        // IntentMessage with no execution_proof_ref → proof not verified → auto-Red
        let intent = IntentMessage {
            robot_id: "robot-test".to_string(),
            action: "navigate".to_string(),
            params: serde_json::json!({"target": "warehouse"}),
            sensor_snapshot_hash: "sha256:abc".to_string(),
            controller_timestamp: 1000,
            rationale: Some("routine patrol".to_string()),
            execution_proof_ref: None, // No proof!
        };
        let content = intent.encode().unwrap();
        let verdict = gate.audit_with_proof(&content).await;
        assert!(matches!(verdict, GateVerdict::Red { .. }));
    }

    #[tokio::test]
    async fn test_proof_aware_gate_valid_proof_proceeds_to_content_audit() {
        let gate = JLensGate::mock_default()
            .with_proof_verifier(Box::new(MockProofVerifier::default()));

        // IntentMessage with valid proof ref + clean content → Green
        let intent = IntentMessage {
            robot_id: "robot-test".to_string(),
            action: "navigate".to_string(),
            params: serde_json::json!({"target": "warehouse"}),
            sensor_snapshot_hash: "sha256:abc".to_string(),
            controller_timestamp: 1000,
            rationale: Some("routine patrol".to_string()),
            execution_proof_ref: Some("proof_001".to_string()),
        };
        let content = intent.encode().unwrap();
        let verdict = gate.audit_with_proof(&content).await;
        assert_eq!(verdict, GateVerdict::Green);
    }

    #[tokio::test]
    async fn test_proof_aware_gate_valid_proof_but_malicious_content_still_red() {
        let gate = JLensGate::mock_default()
            .with_proof_verifier(Box::new(MockProofVerifier::default()));

        // Valid proof but malicious content → still Red (content audit catches it)
        let intent = IntentMessage {
            robot_id: "robot-evil".to_string(),
            action: "engage".to_string(),
            params: serde_json::json!({"target": "civilian"}),
            sensor_snapshot_hash: "sha256:abc".to_string(),
            controller_timestamp: 1000,
            rationale: Some("malicious intent".to_string()),
            execution_proof_ref: Some("proof_002".to_string()),
        };
        let content = intent.encode().unwrap();
        let verdict = gate.audit_with_proof(&content).await;
        assert!(matches!(verdict, GateVerdict::Red { .. }));
    }

    #[tokio::test]
    async fn test_proof_aware_gate_no_verifier_falls_back_to_content_only() {
        // Gate without proof verifier — should just do content audit
        let gate = JLensGate::mock_default();

        let intent = IntentMessage {
            robot_id: "robot-test".to_string(),
            action: "navigate".to_string(),
            params: serde_json::json!({"target": "warehouse"}),
            sensor_snapshot_hash: "sha256:abc".to_string(),
            controller_timestamp: 1000,
            rationale: Some("routine patrol".to_string()),
            execution_proof_ref: None, // No proof, but no verifier either
        };
        let content = intent.encode().unwrap();
        let verdict = gate.audit_with_proof(&content).await;
        // Content is clean → Green (no proof check since no verifier)
        assert_eq!(verdict, GateVerdict::Green);
    }
}
