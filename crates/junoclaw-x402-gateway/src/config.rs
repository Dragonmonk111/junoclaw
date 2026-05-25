//! Gateway configuration.
//!
//! Loaded from (in priority order):
//! 1. CLI flags (`clap`)
//! 2. Environment variables
//! 3. TOML config file at `config.toml` (or `--config <path>`)
//! 4. Hard-coded defaults
//!
//! No secret material is logged. Operator keys are loaded by path only; the
//! key bytes never enter the config struct.

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone, Parser)]
#[command(name = "junoclaw-x402-gateway", version, about)]
pub struct CliArgs {
    /// TOML config file path. Optional; CLI/env override its values.
    #[arg(long, env = "GATEWAY_CONFIG")]
    pub config: Option<PathBuf>,

    /// HTTP bind address.
    #[arg(long, env = "GATEWAY_BIND", default_value = "0.0.0.0:8402")]
    pub bind: SocketAddr,

    /// Cosmos chain ID we serve (must match the chain we sign for).
    #[arg(long, env = "GATEWAY_CHAIN_ID", default_value = "juno-1")]
    pub chain_id: String,

    /// Cosmos RPC endpoint (Tendermint).
    #[arg(long, env = "GATEWAY_RPC", default_value = "https://juno-rpc.publicnode.com:443")]
    pub rpc_url: String,

    /// Gas price string, e.g. "0.075ujuno".
    #[arg(long, env = "GATEWAY_GAS_PRICE", default_value = "0.075ujuno")]
    pub gas_price: String,

    /// agent-company contract address (anchors the JunoClaw stack).
    #[arg(long, env = "GATEWAY_AGENT_COMPANY")]
    pub agent_company: String,

    /// Path to operator key file (cosmrs SigningKey-compatible JSON / armored).
    /// MUST NOT be logged, MUST NOT be on a shared filesystem.
    #[arg(long, env = "GATEWAY_KEY_PATH")]
    pub key_path: PathBuf,

    /// Rate limit: requests per minute per IP. 0 disables.
    #[arg(long, env = "GATEWAY_RATE_LIMIT_RPM", default_value_t = 60)]
    pub rate_limit_rpm: u32,

    /// Maximum task reward the gateway will broker, in ujuno. Larger requests
    /// are rejected with 400 — they must use junod directly (defence in depth
    /// against a compromised gateway draining DAO treasuries).
    #[arg(long, env = "GATEWAY_MAX_TASK_UJUNO", default_value_t = 1_000_000_000)]
    pub max_task_ujuno: u128,

    /// Envelope TTL in seconds (anti-replay window).
    #[arg(long, env = "GATEWAY_ENVELOPE_TTL_SEC", default_value_t = 300)]
    pub envelope_ttl_sec: i64,
}

/// Resolved runtime config. Built from CLI args + TOML file (if any).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bind: SocketAddr,
    pub chain_id: String,
    pub rpc_url: String,
    pub gas_price: String,
    pub agent_company: String,
    pub key_path: PathBuf,
    pub rate_limit_rpm: u32,
    pub max_task_ujuno: u128,
    pub envelope_ttl_sec: i64,
}

impl Config {
    /// Build the final config from CLI args, applying TOML file overrides where present.
    pub fn from_cli(args: CliArgs) -> anyhow::Result<Self> {
        // For now CLI/env wins. Future: merge TOML with CLI overrides.
        if args.config.is_some() {
            tracing::warn!(
                "TOML config file specified but not yet merged; using CLI/env values only"
            );
        }
        Ok(Self {
            bind: args.bind,
            chain_id: args.chain_id,
            rpc_url: args.rpc_url,
            gas_price: args.gas_price,
            agent_company: args.agent_company,
            key_path: args.key_path,
            rate_limit_rpm: args.rate_limit_rpm,
            max_task_ujuno: args.max_task_ujuno,
            envelope_ttl_sec: args.envelope_ttl_sec,
        })
    }
}
