//! Gateway configuration (CLI flags + env).

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone, Parser)]
#[command(name = "junoclaw-attest-gateway", version, about)]
pub struct CliArgs {
    /// HTTP bind address.
    #[arg(long, env = "ATTEST_BIND", default_value = "0.0.0.0:8411")]
    pub bind: SocketAddr,

    /// JunoClaw chain ID.
    #[arg(long, env = "ATTEST_CHAIN_ID", default_value = "junoclaw-g1")]
    pub chain_id: String,

    /// Cosmos SDK gRPC endpoint (host:port, plaintext on g1 testnet).
    /// g1 has no Tendermint RPC — do NOT point this at :26657.
    #[arg(long, env = "ATTEST_GRPC", default_value = "51.195.201.74:9090")]
    pub grpc_endpoint: String,

    /// Gas price, e.g. "0.025ujclaw".
    #[arg(long, env = "ATTEST_GAS_PRICE", default_value = "0.025ujclaw")]
    pub gas_price: String,

    /// Gas limit for anchor_root exec txs.
    #[arg(long, env = "ATTEST_GAS_LIMIT", default_value_t = 400_000)]
    pub gas_limit: u64,

    /// merkle-verifier contract address (deployed code ID 22 on g1).
    #[arg(
        long,
        env = "ATTEST_MERKLE_CONTRACT",
        default_value = "juno12k0sr39erruxqtwhxndkrlmxdl5kgt5vt7lqxaa4gpxypww866rsjz2xpq"
    )]
    pub merkle_contract: String,

    /// thought-court contract address (bonded tier, future).
    #[arg(
        long,
        env = "ATTEST_COURT_CONTRACT",
        default_value = "juno10c3dunkwh68fgufsd4cw8q24e0wjcsh9290c0xvy22hfaqjprxds63sy32"
    )]
    pub court_contract: String,

    /// Name of the env var holding the gateway signer mnemonic
    /// (must be the merkle-verifier ADMIN for anchor_root to pass).
    #[arg(long, env = "ATTEST_MNEMONIC_ENV", default_value = "JUNOCLAW_ATTEST_MNEMONIC")]
    pub mnemonic_env: String,

    /// bech32 account prefix.
    #[arg(long, env = "ATTEST_PREFIX", default_value = "juno")]
    pub account_prefix: String,

    /// BIP-44 HD path.
    #[arg(long, env = "ATTEST_HD_PATH", default_value = "m/44'/118'/0'/0/0")]
    pub hd_path: String,

    /// Batch window in seconds — queue drains at least this often.
    #[arg(long, env = "ATTEST_WINDOW_SECS", default_value_t = 60)]
    pub window_secs: u64,

    /// Max leaves per anchored batch.
    #[arg(long, env = "ATTEST_MAX_LEAVES", default_value_t = 1024)]
    pub max_leaves: usize,

    /// Spool file path (JSONL; replayed on boot).
    #[arg(long, env = "ATTEST_SPOOL", default_value = "attest-spool.jsonl")]
    pub spool_path: PathBuf,

    /// Max accepted raw payload bytes (larger must use payload_hash).
    #[arg(long, env = "ATTEST_MAX_PAYLOAD_BYTES", default_value_t = 65_536)]
    pub max_payload_bytes: usize,

    /// API keys as "customer:key" pairs, comma-separated.
    /// Empty = dev mode, all requests accepted as customer "dev".
    #[arg(long, env = "ATTEST_API_KEYS", default_value = "")]
    pub api_keys: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bind: SocketAddr,
    pub chain_id: String,
    pub grpc_endpoint: String,
    pub gas_price: String,
    pub gas_limit: u64,
    pub merkle_contract: String,
    pub court_contract: String,
    pub mnemonic_env: String,
    pub account_prefix: String,
    pub hd_path: String,
    pub window_secs: u64,
    pub max_leaves: usize,
    pub spool_path: PathBuf,
    pub max_payload_bytes: usize,
    /// customer name -> api key
    pub api_keys: Vec<(String, String)>,
}

impl Config {
    pub fn from_cli(args: CliArgs) -> anyhow::Result<Self> {
        let api_keys = args
            .api_keys
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|pair| {
                let (cust, key) = pair.split_once(':').ok_or_else(|| {
                    anyhow::anyhow!("api_keys entry '{pair}' missing 'customer:key' shape")
                })?;
                Ok((cust.trim().to_string(), key.trim().to_string()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            bind: args.bind,
            chain_id: args.chain_id,
            grpc_endpoint: args.grpc_endpoint,
            gas_price: args.gas_price,
            gas_limit: args.gas_limit,
            merkle_contract: args.merkle_contract,
            court_contract: args.court_contract,
            mnemonic_env: args.mnemonic_env,
            account_prefix: args.account_prefix,
            hd_path: args.hd_path,
            window_secs: args.window_secs,
            max_leaves: args.max_leaves,
            spool_path: args.spool_path,
            max_payload_bytes: args.max_payload_bytes,
            api_keys,
        })
    }
}
