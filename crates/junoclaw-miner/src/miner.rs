//! Miner core — watches coordination batches, evaluates them, submits verdicts.
//!
//! The miner is the "Bitcoin miner for truth" — it:
//! 1. Watches the coordination REST API for finalized batches
//! 2. Pulls batch data (proof, context, gate result)
//! 3. Runs the evaluator (LLM, rule-based, or MCAP-based)
//! 4. Submits a verdict to the truth market contract on Juno
//! 5. Tracks rewards and slashing

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::evaluator::{TruthEvaluator, Verdict, BatchData};
use crate::identity::MinerIdentity;

/// Miner configuration.
#[derive(Clone, Debug)]
pub struct MinerConfig {
    /// Coordination REST API endpoint (e.g., "http://localhost:8080")
    pub coordination_api: String,
    /// Juno RPC endpoint (e.g., "https://juno.rpc.t.stavr.tech")
    pub juno_rpc: String,
    /// Juno REST/LCD endpoint (e.g., "https://juno-testnet-api.cogwheel.zone")
    pub juno_rest: String,
    /// Truth market contract address
    pub truth_market_contract: String,
    /// Polling interval for new batches (seconds)
    pub poll_interval_secs: u64,
    /// Maximum batches to evaluate per poll cycle
    pub max_batches_per_poll: usize,
    /// Whether to submit verdicts on-chain (false = dry run)
    pub submit_on_chain: bool,
    /// Wallet ID for cosmos-mcp CLI (references encrypted wallet in ~/.junoclaw/wallets/).
    /// Use "dry-run" to skip on-chain submission.
    pub mnemonic: Option<String>,
    /// Gas amount for verdict submission
    pub gas_amount: u64,
    /// Gas price in ujunox
    pub gas_price: u64,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            coordination_api: "http://localhost:8080".to_string(),
            juno_rpc: "https://juno.rpc.t.stavr.tech".to_string(),
            juno_rest: "https://juno-testnet-api.cogwheel.zone".to_string(),
            truth_market_contract: "juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p".to_string(),
            poll_interval_secs: 10,
            max_batches_per_poll: 5,
            submit_on_chain: false,
            mnemonic: None,
            gas_amount: 200000,
            gas_price: 2500,
        }
    }
}

/// Miner state — tracks what we've evaluated and submitted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinerState {
    /// Last batch height we've seen
    pub last_seen_height: u64,
    /// Last batch height we've submitted a verdict for
    pub last_submitted_height: u64,
    /// Total verdicts submitted
    pub total_verdicts: u64,
    /// Verdicts by type
    pub green_count: u64,
    pub yellow_count: u64,
    pub red_count: u64,
    /// Total rewards earned (ujunox)
    pub total_rewards: u128,
    /// Total slashed (ujunox)
    pub total_slashed: u128,
    /// Whether we're registered as an operator
    pub registered: bool,
    /// Our operator stake (ujunox)
    pub stake: u128,
}

impl Default for MinerState {
    fn default() -> Self {
        Self {
            last_seen_height: 0,
            last_submitted_height: 0,
            total_verdicts: 0,
            green_count: 0,
            yellow_count: 0,
            red_count: 0,
            total_rewards: 0,
            total_slashed: 0,
            registered: false,
            stake: 0,
        }
    }
}

/// The miner — runs the evaluation loop.
pub struct Miner {
    config: MinerConfig,
    identity: MinerIdentity,
    evaluator: Arc<dyn TruthEvaluator>,
    state: MinerState,
    client: reqwest::Client,
}

impl Miner {
    pub fn new(
        config: MinerConfig,
        identity: MinerIdentity,
        evaluator: Arc<dyn TruthEvaluator>,
    ) -> Self {
        Self {
            config,
            identity,
            evaluator,
            state: MinerState::default(),
            client: reqwest::Client::new(),
        }
    }

    /// Run the miner loop — polls for batches, evaluates, submits verdicts.
    pub async fn run(&mut self) -> Result<()> {
        info!(
            identity = %self.identity.description(),
            evaluator = %self.evaluator.name(),
            coordination_api = %self.config.coordination_api,
            "miner starting"
        );

        // Check if we're registered
        if !self.state.registered {
            info!("miner not registered — run `junoclaw-miner register` first");
        }

        let poll_duration = Duration::from_secs(self.config.poll_interval_secs);

        loop {
            match self.poll_and_evaluate().await {
                Ok(count) => {
                    if count > 0 {
                        info!(batches_evaluated = count, "poll cycle complete");
                    }
                }
                Err(e) => {
                    error!(err = %e, "poll cycle failed");
                }
            }

            tokio::time::sleep(poll_duration).await;
        }
    }

    /// Poll for new batches and evaluate them.
    pub async fn poll_and_evaluate(&mut self) -> Result<usize> {
        let batches = self.fetch_new_batches().await?;
        let mut evaluated = 0;

        for batch in batches.iter().take(self.config.max_batches_per_poll) {
            match self.evaluator.evaluate(batch).await {
                Ok(verdict) => {
                    info!(
                        batch_height = batch.batch_height,
                        verdict = %verdict,
                        "evaluated batch"
                    );

                    if self.config.submit_on_chain {
                        if let Err(e) = self.submit_verdict(batch, &verdict).await {
                            warn!(batch_height = batch.batch_height, err = %e, "failed to submit verdict");
                        }
                    }

                    self.record_verdict(&verdict);
                    self.state.last_submitted_height = batch.batch_height;
                    evaluated += 1;
                }
                Err(e) => {
                    warn!(batch_height = batch.batch_height, err = %e, "evaluation failed");
                }
            }
        }

        if let Some(last) = batches.last() {
            self.state.last_seen_height = last.batch_height;
        }

        Ok(evaluated)
    }

    /// Fetch new batches from the coordination REST API.
    async fn fetch_new_batches(&self) -> Result<Vec<BatchData>> {
        let url = format!("{}/finalized", self.config.coordination_api);
        let resp = self.client.get(&url).send().await
            .context("failed to fetch finalized batches")?;

        if !resp.status().is_success() {
            anyhow::bail!("coordination API returned {}", resp.status());
        }

        let body: serde_json::Value = resp.json().await?;

        // Parse the response — format depends on coordination API
        let batches = parse_finalized_batches(&body, self.state.last_seen_height);

        Ok(batches)
    }

    /// Submit a verdict to the truth market contract on Juno.
    ///
    /// Uses the cosmos-mcp CLI subprocess for signing and broadcasting,
    /// same pattern as the relayer bridge. The wallet_id references an
    /// encrypted wallet in ~/.junoclaw/wallets/ enrolled via `cosmos-mcp wallet add`.
    ///
    /// Command: node mcp/dist/index.js wallet exec <wallet_id> <contract> <msg_json> --rpc <rpc>
    async fn submit_verdict(&self, batch: &BatchData, verdict: &Verdict) -> Result<()> {
        let wallet_id = self.config.mnemonic.as_deref().unwrap_or("dry-run");

        if wallet_id == "dry-run" || wallet_id.is_empty() {
            info!(
                batch_height = batch.batch_height,
                verdict = %verdict,
                "[dry-run] skipping on-chain verdict submission"
            );
            return Ok(());
        }

        let msg = serde_json::json!({
            "submit_verdict": {
                "batch_height": batch.batch_height,
                "verdict": verdict.as_str(),
                "messages_hash": &batch.messages_hash,
            }
        });
        let msg_json = serde_json::to_string(&msg)
            .context("failed to serialize SubmitVerdict msg")?;

        let mcp_path = std::env::var("MCP_CLI_PATH")
            .unwrap_or_else(|_| "node".to_string());

        info!(
            batch_height = batch.batch_height,
            verdict = %verdict,
            contract = %self.config.truth_market_contract,
            "submitting verdict to truth market via cosmos-mcp"
        );

        let output = tokio::process::Command::new(&mcp_path)
            .arg("mcp/dist/index.js")
            .arg("wallet")
            .arg("exec")
            .arg(wallet_id)
            .arg(&self.config.truth_market_contract)
            .arg(&msg_json)
            .arg("--rpc")
            .arg(&self.config.juno_rpc)
            .output()
            .await
            .context("failed to spawn cosmos-mcp CLI subprocess")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                batch_height = batch.batch_height,
                stderr = %stderr.trim(),
                "cosmos-mcp CLI returned non-zero exit"
            );
            return Err(anyhow::anyhow!("cosmos-mcp CLI failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!(
            batch_height = batch.batch_height,
            verdict = %verdict,
            tx = %stdout.trim(),
            "verdict submitted on Juno"
        );

        Ok(())
    }

    /// Record a verdict in state.
    fn record_verdict(&mut self, verdict: &Verdict) {
        self.state.total_verdicts += 1;
        match verdict {
            Verdict::Green => self.state.green_count += 1,
            Verdict::Yellow => self.state.yellow_count += 1,
            Verdict::Red => self.state.red_count += 1,
        }
    }

    /// Get current state.
    pub fn state(&self) -> &MinerState {
        &self.state
    }

    /// Print status summary.
    pub fn print_status(&self) {
        let s = &self.state;
        println!("═══ Miner Status ═══");
        println!("Identity:     {}", self.identity.description());
        println!("Evaluator:    {}", self.evaluator.name());
        println!("Registered:   {}", if s.registered { "yes" } else { "no" });
        println!("Last seen:    batch #{}", s.last_seen_height);
        println!("Last verdict: batch #{}", s.last_submitted_height);
        println!("Total verdicts: {}", s.total_verdicts);
        println!("  Green: {}", s.green_count);
        println!("  Yellow: {}", s.yellow_count);
        println!("  Red: {}", s.red_count);
        if s.total_rewards > 0 {
            println!("Rewards:      {} ujunox", s.total_rewards);
        }
        if s.total_slashed > 0 {
            println!("Slashed:      {} ujunox", s.total_slashed);
        }
    }
}

/// Parse finalized batches from the coordination API response.
fn parse_finalized_batches(body: &serde_json::Value, after_height: u64) -> Vec<BatchData> {
    let mut batches = Vec::new();

    // Try array format: [{ "batch_height": 1, ... }, ...]
    if let Some(arr) = body.as_array() {
        for item in arr {
            if let Some(batch) = parse_batch_item(item) {
                if batch.batch_height > after_height {
                    batches.push(batch);
                }
            }
        }
    }

    // Try object format: { "batches": [...] }
    if let Some(arr) = body.get("batches").and_then(|b| b.as_array()) {
        for item in arr {
            if let Some(batch) = parse_batch_item(item) {
                if batch.batch_height > after_height {
                    batches.push(batch);
                }
            }
        }
    }

    batches.sort_by_key(|b| b.batch_height);
    batches
}

fn parse_batch_item(item: &serde_json::Value) -> Option<BatchData> {
    let batch_height = item.get("batch_height")?.as_u64()?;
    let messages_hash = item.get("messages_hash")
        .and_then(|h| h.as_str())
        .unwrap_or("")
        .to_string();

    let proof_hex = item.get("proof_hex")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());

    let gate_verdict = item.get("gate_verdict")
        .and_then(|g| g.as_str())
        .map(|s| s.to_string());

    let gate_separation_score = item.get("gate_separation_score")
        .and_then(|s| s.as_f64());

    let robot_id = item.get("robot_id")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string());

    let intent_summary = item.get("intent_summary")
        .and_then(|i| i.as_str())
        .map(|s| s.to_string());

    let finalized_at = item.get("finalized_at")
        .and_then(|t| t.as_u64());

    Some(BatchData {
        batch_height,
        messages_hash,
        proof_hex,
        proof_context: None,
        robot_id,
        intent_summary,
        safety_envelope: None,
        gate_verdict,
        gate_separation_score,
        finalized_at,
    })
}
