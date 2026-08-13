//! Batch watcher — polls the coordination network for finalized batches
//! and relays them to the Juno coordination-settler contract.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

/// Configuration for the batch watcher.
#[derive(Clone, Debug)]
pub struct WatcherConfig {
    pub rpc_endpoint: String,
    pub contract_addr: String,
    pub relayer_key: String,
    pub coordination_endpoint: String,
    pub poll_interval_secs: u64,
}

/// A finalized batch from the coordination network.
/// This is the format the coordination node exposes via its REST API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizedBatch {
    pub commonware_height: u64,
    pub messages_hash: [u8; 32],
    pub certificate: Vec<u8>,
    pub timestamp: u64,
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
                "Relaying batch at height {} ({} bytes cert)",
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
            "Would submit SubmitBatch to {} at height {} (cert_hash={})",
            self.config.contract_addr,
            batch.commonware_height,
            hex::encode(&batch.messages_hash)
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
