// Prover daemon configuration.
//
// Loaded from a TOML or JSON config file. All fields have sensible defaults
// for a single-robot deployment.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverConfig {
    /// Unique robot identifier (matches skill-registry dapp_name)
    pub robot_id: String,

    /// ROS2 bridge HTTP endpoint
    pub bridge_url: String,

    /// Chain RPC endpoint for on-chain submission
    pub chain_rpc: String,

    /// Directory containing proving/verifying keys
    pub keys_dir: PathBuf,

    /// zk-verifier contract address (optional — if None, proofs saved locally)
    pub verifier_addr: Option<String>,

    /// circuit-breaker contract address (optional — if None, breaker not checked)
    pub circuit_breaker_addr: Option<String>,

    /// safety-envelope contract address (optional — if None, hardcoded envelope used)
    pub safety_envelope_addr: Option<String>,

    /// merkle-verifier contract address (optional)
    pub merkle_verifier_addr: Option<String>,

    /// Polling interval in seconds
    pub poll_interval_secs: u64,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
}

impl Default for ProverConfig {
    fn default() -> Self {
        Self {
            robot_id: "robot-01".to_string(),
            bridge_url: "http://localhost:8080".to_string(),
            chain_rpc: "http://localhost:26657".to_string(),
            keys_dir: PathBuf::from("./keys"),
            verifier_addr: None,
            circuit_breaker_addr: None,
            safety_envelope_addr: None,
            merkle_verifier_addr: None,
            poll_interval_secs: 10,
            log_level: "info".to_string(),
        }
    }
}

impl ProverConfig {
    /// Load config from a file. Supports TOML (.toml) and JSON (.json) based on extension.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read config {}: {}", path.display(), e))?;

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("toml");

        let cfg = match ext {
            "json" => serde_json::from_str(&content)?,
            _ => toml::from_str(&content)?,
        };

        Ok(cfg)
    }

    /// Save config to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("toml");

        let content = match ext {
            "json" => serde_json::to_string_pretty(self)?,
            _ => toml::to_string_pretty(self)?,
        };

        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = ProverConfig::default();
        assert_eq!(cfg.robot_id, "robot-01");
        assert_eq!(cfg.poll_interval_secs, 10);
    }

    #[test]
    fn test_toml_roundtrip() {
        let cfg = ProverConfig {
            robot_id: "test-bot".to_string(),
            bridge_url: "http://robot:8080".to_string(),
            chain_rpc: "http://chain:26657".to_string(),
            keys_dir: PathBuf::from("/keys"),
            verifier_addr: Some("juno1...".to_string()),
            circuit_breaker_addr: None,
            safety_envelope_addr: None,
            merkle_verifier_addr: None,
            poll_interval_secs: 5,
            log_level: "debug".to_string(),
        };

        let toml_str = toml::to_string(&cfg).unwrap();
        let parsed: ProverConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.robot_id, "test-bot");
        assert_eq!(parsed.poll_interval_secs, 5);
    }
}
