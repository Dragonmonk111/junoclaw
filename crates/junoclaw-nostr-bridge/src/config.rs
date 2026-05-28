use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Bridge configuration. Load from environment or TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Tendermint RPC endpoint (websocket will be derived from this)
    pub rpc_url: String,
    /// task-ledger contract address
    pub contract: String,
    /// Chain ID (e.g. "juno-1", "uni-7")
    pub chain_id: String,
    /// zk-verifier contract address (included in each task event)
    pub zk_verifier: String,
    /// Hex-encoded secp256k1 private key for signing Nostr events
    pub nostr_privkey_hex: String,
    /// Nostr relay websocket URLs (≥3 recommended)
    pub relays: Vec<String>,
    /// How long to wait before republishing an unchanged open task (seconds)
    #[serde(default = "default_republish_interval")]
    pub republish_interval_secs: u64,
    /// Log level filter string (e.g. "info", "debug,junoclaw_nostr_bridge=trace")
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_republish_interval() -> u64 { 3600 }
fn default_log_level() -> String { "info".into() }

impl BridgeConfig {
    /// Load from environment variables (12-factor app style).
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            rpc_url: std::env::var("JUNOCLAW_RPC")
                .unwrap_or_else(|_| "https://rpc.juno.strange.love:443".into()),
            contract: std::env::var("JUNOCLAW_CONTRACT")
                .context("JUNOCLAW_CONTRACT not set")?,
            chain_id: std::env::var("JUNOCLAW_CHAIN_ID")
                .unwrap_or_else(|_| "juno-1".into()),
            zk_verifier: std::env::var("JUNOCLAW_ZK_VERIFIER")
                .unwrap_or_else(|_| String::new()),
            nostr_privkey_hex: std::env::var("JUNOCLAW_NOSTR_PRIVKEY")
                .context("JUNOCLAW_NOSTR_PRIVKEY not set")?,
            relays: std::env::var("JUNOCLAW_NOSTR_RELAYS")
                .unwrap_or_else(|_| {
                    "wss://relay.damus.io,wss://nos.lol,wss://relay.snort.social".into()
                })
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            republish_interval_secs: std::env::var("JUNOCLAW_REPUBLISH_INTERVAL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            log_level: std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".into()),
        })
    }

    /// Derive the websocket URL from the HTTP RPC URL.
    pub fn ws_url(&self) -> String {
        let base = self.rpc_url.trim_end_matches('/');
        if base.starts_with("https://") {
            format!("wss://{}/websocket", &base["https://".len()..])
        } else if base.starts_with("http://") {
            format!("ws://{}/websocket", &base["http://".len()..])
        } else {
            format!("{base}/websocket")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(rpc: &str) -> BridgeConfig {
        BridgeConfig {
            rpc_url: rpc.into(),
            contract: "juno1test".into(),
            chain_id: "uni-7".into(),
            zk_verifier: "juno1verifier".into(),
            nostr_privkey_hex: "aa".repeat(32),
            relays: vec!["wss://relay.damus.io".into()],
            republish_interval_secs: 3600,
            log_level: "info".into(),
        }
    }

    #[test]
    fn test_ws_url_https() {
        let cfg = test_config("https://rpc.juno.strange.love:443");
        assert_eq!(cfg.ws_url(), "wss://rpc.juno.strange.love:443/websocket");
    }

    #[test]
    fn test_ws_url_http() {
        let cfg = test_config("http://localhost:26657");
        assert_eq!(cfg.ws_url(), "ws://localhost:26657/websocket");
    }

    #[test]
    fn test_ws_url_trailing_slash() {
        let cfg = test_config("https://rpc.example.com/");
        assert_eq!(cfg.ws_url(), "wss://rpc.example.com/websocket");
    }

    #[test]
    fn test_ws_url_bare() {
        let cfg = test_config("rpc.example.com:26657");
        assert_eq!(cfg.ws_url(), "rpc.example.com:26657/websocket");
    }
}
