use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// IBC relay configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Juno gRPC endpoint
    pub juno_grpc: String,
    /// `ibc-task-host` contract address on Juno
    pub host_contract: String,
    /// IBC channel from source chain to Juno (e.g. "channel-0")
    pub channel_to_juno: String,
    /// Source chain gRPC endpoint (where the agent lives)
    pub source_grpc: String,
    /// Source chain ID
    pub source_chain_id: String,
    /// Denom to send in ICS-20 transfer (1 token = minimum bond)
    pub transfer_denom: String,
    /// 24-hour grace window for IBC-relayed proofs (blocks)
    #[serde(default = "default_grace_blocks")]
    pub grace_blocks: u64,
}

fn default_grace_blocks() -> u64 { 14400 } // ~24h at 6s/block

impl RelayConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            juno_grpc: std::env::var("JUNO_GRPC")
                .unwrap_or_else(|_| "https://juno-grpc.polkachu.com:12690".into()),
            host_contract: std::env::var("IBC_TASK_HOST")
                .context("IBC_TASK_HOST not set")?,
            channel_to_juno: std::env::var("IBC_CHANNEL_TO_JUNO")
                .context("IBC_CHANNEL_TO_JUNO not set")?,
            source_grpc: std::env::var("SOURCE_GRPC")
                .context("SOURCE_GRPC not set")?,
            source_chain_id: std::env::var("SOURCE_CHAIN_ID")
                .context("SOURCE_CHAIN_ID not set")?,
            transfer_denom: std::env::var("TRANSFER_DENOM")
                .unwrap_or_else(|_| "uosmo".into()),
            grace_blocks: std::env::var("IBC_GRACE_BLOCKS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(14400),
        })
    }
}
