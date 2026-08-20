//! Batch watcher — polls the coordination network for finalized batches
//! and relays them to the Juno coordination-settler contract.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::breaker::BreakerConfig;
use crate::executor::ExecutorConfig;
use crate::market::MarketConfig;
use crate::moult::MoultConfig;

/// Configuration for the batch watcher.
#[derive(Clone, Debug)]
pub struct WatcherConfig {
    pub rpc_endpoint: String,
    pub contract_addr: String,
    pub relayer_key: String,
    pub coordination_endpoint: String,
    pub poll_interval_secs: u64,
    /// When set, every settled batch also gets a moultbook entry (the
    /// semantic on-chain index — see moult.rs).
    pub moult: Option<MoultConfig>,
    /// Layer 5: when set, extracted tasks from settled batches are
    /// submitted to the task-ledger contract for execution.
    pub executor: Option<ExecutorConfig>,
    /// Layer 6: when set, eval epochs are finalized after each batch
    /// settlement (truth market reward/slash distribution).
    pub market: Option<MarketConfig>,
    /// Circuit breaker: when set, TripBreaker txs are submitted for
    /// any BreakerActions detected in finalized batches.
    pub breaker: Option<BreakerConfig>,
}

/// A finalized batch from the coordination network.
/// This is the format the coordination node exposes via its REST API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizedBatch {
    /// Block height (aliased from API's "height" field)
    #[serde(alias = "height")]
    pub commonware_height: u64,
    /// Messages hash (hex-encoded string from API)
    #[serde(default)]
    pub messages_hash: String,
    /// Certificate (hex-encoded string from API)
    #[serde(default)]
    pub certificate: String,
    pub timestamp: u64,
    /// Off-chain payload size in bytes, if the coordination node reports it.
    /// 0 = unknown (recorded as-is in the moultbook entry).
    #[serde(default)]
    pub payload_size_bytes: u64,
    /// Breaker actions emitted during consensus (e.g. red-gated robot intents).
    #[serde(default)]
    pub breaker_actions: Vec<junoclaw_coordination::BreakerAction>,
    /// Moultbook context digest fetched during consensus.
    #[serde(default)]
    pub context_digest: Option<String>,
    /// Batch hash (hex-encoded, from API)
    #[serde(default)]
    pub batch_hash: String,
    /// Message count in the batch
    #[serde(default)]
    pub message_count: usize,
}

/// Response from the coordination node's /finalized endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizedBatchesResponse {
    pub batches: Vec<FinalizedBatch>,
    pub latest_height: u64,
}

/// The batch watcher — polls for finalized batches and relays them.
pub struct BatchWatcher {
    config: WatcherConfig,
    http_client: reqwest::Client,
    /// Last relayed height (to avoid resubmitting)
    last_relayed_height: u64,
}

impl BatchWatcher {
    pub fn new(config: WatcherConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self {
            config,
            http_client,
            last_relayed_height: 0,
        }
    }

    /// Run the watcher loop — polls for finalized batches and relays them.
    pub async fn run(mut self) -> Result<()> {
        info!("Batch watcher started — polling for finalized batches");

        loop {
            if let Err(e) = self.poll_and_relay().await {
                error!("Poll cycle failed: {}", e);
            }

            tokio::time::sleep(Duration::from_secs(
                self.config.poll_interval_secs,
            ))
            .await;
        }
    }

    /// Single poll cycle: fetch finalized batches, relay any new ones.
    async fn poll_and_relay(&mut self) -> Result<()> {
        let batches = self.fetch_finalized_batches().await?;

        for batch in batches {
            if batch.commonware_height <= self.last_relayed_height {
                continue;
            }

            info!(
                "Relaying batch at height {} ({} cert chars)",
                batch.commonware_height,
                batch.certificate.len()
            );

            match self.relay_batch(&batch).await {
                Ok(()) => {
                    self.last_relayed_height = batch.commonware_height;
                    info!(
                        "Batch {} settled on Juno",
                        batch.commonware_height
                    );

                    // Moultbook addendum: semantic index entry for the
                    // settled batch. Best-effort — a failed moult post must
                    // not stall settlement of subsequent batches.
                    if let Some(moult_cfg) = &self.config.moult {
                        if let Err(e) = crate::moult::post_batch_moult(
                            &self.config.rpc_endpoint,
                            &self.config.relayer_key,
                            moult_cfg,
                            &batch,
                        )
                        .await
                        {
                            warn!(
                                "Moultbook addendum failed for batch {}: {}",
                                batch.commonware_height, e
                            );
                        }
                    }

                    // Layer 5 — Execution Bridge: extract task requests
                    // from the settled batch and submit them to the
                    // task-ledger contract. Best-effort.
                    if let Some(exec_cfg) = &self.config.executor {
                        let tasks = crate::executor::extract_tasks(&batch);
                        if !tasks.is_empty() {
                            if let Err(e) = crate::executor::submit_tasks(
                                &self.config.rpc_endpoint,
                                &self.config.relayer_key,
                                exec_cfg,
                                &tasks,
                            )
                            .await
                            {
                                warn!(
                                    "Executor bridge failed for batch {}: {}",
                                    batch.commonware_height, e
                                );
                            }
                        }
                    }

                    // Layer 6 — Truth Market: finalize the eval epoch
                    // for this batch, distributing rewards to matching
                    // evaluators and slashing diverging ones. Best-effort.
                    if let Some(market_cfg) = &self.config.market {
                        if let Err(e) = crate::market::finalize_epoch(
                            &self.config.rpc_endpoint,
                            &self.config.relayer_key,
                            market_cfg,
                            &batch,
                        )
                        .await
                        {
                            warn!(
                                "Truth market finalization failed for batch {}: {}",
                                batch.commonware_height, e
                            );
                        }
                    }

                    // Circuit Breaker: submit TripBreaker txs for any
                    // BreakerActions detected during consensus. Best-effort.
                    if let Some(breaker_cfg) = &self.config.breaker {
                        if !batch.breaker_actions.is_empty() {
                            info!(
                                "Submitting {} breaker actions for batch {}",
                                batch.breaker_actions.len(),
                                batch.commonware_height
                            );
                            if let Err(e) = crate::breaker::submit_breaker_actions(
                                &self.config.rpc_endpoint,
                                &self.config.relayer_key,
                                breaker_cfg,
                                &batch.breaker_actions,
                            )
                            .await
                            {
                                warn!(
                                    "Circuit breaker submission failed for batch {}: {}",
                                    batch.commonware_height, e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to relay batch {}: {}",
                        batch.commonware_height, e
                    );
                    // Don't update last_relayed_height — will retry next cycle
                    break;
                }
            }
        }

        Ok(())
    }

    /// Fetch finalized batches from the coordination node.
    async fn fetch_finalized_batches(&self) -> Result<Vec<FinalizedBatch>> {
        let url = format!(
            "{}/finalized?after={}",
            self.config.coordination_endpoint, self.last_relayed_height
        );

        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            warn!(
                "Coordination node returned {} for {}",
                resp.status(),
                url
            );
            return Ok(vec![]);
        }

        let body: FinalizedBatchesResponse = resp.json().await?;
        Ok(body.batches)
    }

    /// Relay a single finalized batch to the Juno coordination-settler contract.
    async fn relay_batch(&self, batch: &FinalizedBatch) -> Result<()> {
        // TODO: Implement actual on-chain submission using cosmjs or a Rust
        // Cosmos SDK client. This requires:
        // 1. Loading the relayer key (mnemonic or keyfile)
        // 2. Building a CosmWasm ExecuteMsg::SubmitBatch tx
        // 3. Signing and broadcasting to Juno RPC
        //
        // For now, we log the batch that would be submitted.
        // The actual tx submission will use the junoclaw MCP wallet store
        // or a direct cosmrs/cosmrs-based signer.

        info!(
            "Would submit SubmitBatch to {} at height {} (batch_hash={})",
            self.config.contract_addr,
            batch.commonware_height,
            batch.messages_hash,
        );

        // Placeholder: in production, this calls bridge::submit_batch()
        // which signs and broadcasts the transaction.
        crate::bridge::submit_batch(
            &self.config.rpc_endpoint,
            &self.config.contract_addr,
            &self.config.relayer_key,
            batch,
        )
        .await
    }
}
