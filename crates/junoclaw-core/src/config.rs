use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{JunoClawError, Result};

// ──────────────────────────────────────────────
// Top-Level Config
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunoClawConfig {
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub compute: ComputeConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub chain: ChainConfig,
    #[serde(default)]
    pub wavs: WavsConfig,
}

impl Default for JunoClawConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig::default(),
            llm: LlmConfig::default(),
            compute: ComputeConfig::default(),
            storage: StorageConfig::default(),
            chain: ChainConfig::default(),
            wavs: WavsConfig::default(),
        }
    }
}

impl JunoClawConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content =
            toml::to_string_pretty(self).map_err(|e| JunoClawError::Config(e.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn default_path() -> PathBuf {
        Self::data_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".junoclaw")
    }

    pub fn workspaces_dir() -> PathBuf {
        Self::data_dir().join("workspaces")
    }

    pub fn agents_dir() -> PathBuf {
        Self::data_dir().join("agents")
    }

    pub fn sessions_dir() -> PathBuf {
        Self::data_dir().join("sessions")
    }
}

// ──────────────────────────────────────────────
// Daemon Config
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7777,
            log_level: "info".to_string(),
        }
    }
}

// ──────────────────────────────────────────────
// LLM Config
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub default_provider: String,
    pub fallback_chain: Vec<String>,
    pub monthly_budget_usd: f64,
    pub per_task_budget_usd: f64,
    pub rate_limit_rpm: u32,
    #[serde(default)]
    pub providers: LlmProviders,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            default_provider: "ollama".to_string(),
            fallback_chain: vec!["ollama".to_string()],
            monthly_budget_usd: 50.0,
            per_task_budget_usd: 2.0,
            rate_limit_rpm: 60,
            providers: LlmProviders::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmProviders {
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
    #[serde(default)]
    pub anthropic: Option<CloudLlmConfig>,
    #[serde(default)]
    pub openai: Option<CloudLlmConfig>,
    #[serde(default)]
    pub akash: Option<AkashLlmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub default_model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            default_model: "llama3.2:3b".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudLlmConfig {
    /// Environment variable name holding the API key (never stored directly)
    pub api_key_env: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AkashLlmConfig {
    pub deployment_id: Option<String>,
    pub model: String,
    pub gpu: String,
}

// ──────────────────────────────────────────────
// Compute Config
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeConfig {
    #[serde(default)]
    pub local: LocalComputeConfig,
    #[serde(default)]
    pub akash: Option<AkashComputeConfig>,
}

impl Default for ComputeConfig {
    fn default() -> Self {
        Self {
            local: LocalComputeConfig::default(),
            akash: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalComputeConfig {
    pub enabled: bool,
    pub max_concurrent_tasks: u32,
}

impl Default for LocalComputeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_tasks: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AkashComputeConfig {
    pub enabled: bool,
    /// Env var for Akash wallet mnemonic
    pub wallet_mnemonic_env: String,
    pub default_gpu: String,
    pub max_hourly_rate_uakt: u64,
    pub auto_close_timeout_minutes: u32,
    pub docker_image: String,
    /// Payment token: "akt", "juno", "usdc", "atom"
    pub payment_token: String,
    pub max_slippage_percent: f64,
}

// ──────────────────────────────────────────────
// Storage Config
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub local_path: Option<String>,
    pub max_session_history: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            local_path: None,
            max_session_history: 100,
        }
    }
}

// ──────────────────────────────────────────────
// Chain Config (Juno)
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub enabled: bool,
    pub chain_id: String,
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
    pub gas_prices: String,
    pub contracts: ContractAddresses,
    /// On-chain signing configuration. Defaults to disabled + paused so the
    /// daemon never holds a key or broadcasts a write unless explicitly opted in.
    #[serde(default)]
    pub signing: SigningConfig,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            chain_id: "juno-1".to_string(),
            rpc_endpoint: "https://rpc-juno.itastakers.com:443".to_string(),
            grpc_endpoint: "https://grpc-juno.itastakers.com:443".to_string(),
            gas_prices: "0.075ujuno".to_string(),
            contracts: ContractAddresses::default(),
            signing: SigningConfig::default(),
        }
    }
}

/// Guarded on-chain signing config for the autonomous daemon.
///
/// Security posture: the BIP-39 mnemonic is NEVER stored in config/TOML — only
/// the *name* of the environment variable that holds it. Both `enabled` and the
/// `signing_paused` kill-switch must be satisfied (enabled=true, paused=false)
/// before the runtime will sign or broadcast anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Master switch. When false, the runtime never loads a key or signs.
    pub enabled: bool,
    /// Runtime kill-switch. When true, signing is refused even if `enabled`.
    /// Defaults to true so a freshly-enabled signer is still parked until an
    /// operator explicitly un-pauses it.
    pub signing_paused: bool,
    /// Name of the env var holding the BIP-39 mnemonic (e.g. JUNOCLAW_MNEMONIC).
    pub mnemonic_env: String,
    /// bech32 account prefix.
    pub account_prefix: String,
    /// BIP-44 HD derivation path.
    pub hd_path: String,
    /// Gas limit applied to execute/instantiate (static; simulation is a follow-up).
    pub default_gas_limit: u64,
    /// Code ID of the agent-company contract — required to instantiate DAOs on-chain.
    pub agent_company_code_id: Option<u64>,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            signing_paused: true,
            mnemonic_env: "JUNOCLAW_MNEMONIC".to_string(),
            account_prefix: "juno".to_string(),
            hd_path: "m/44'/118'/0'/0/0".to_string(),
            default_gas_limit: 500_000,
            agent_company_code_id: None,
        }
    }
}

impl SigningConfig {
    /// True only when signing is both enabled and not paused.
    pub fn can_sign(&self) -> bool {
        self.enabled && !self.signing_paused
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractAddresses {
    pub agent_registry: Option<String>,
    pub task_ledger: Option<String>,
    pub escrow: Option<String>,
    pub zk_verifier: Option<String>,
    /// moultbook-v0 address. When set, DAO deploys with `anon_endorsement`
    /// enabled will populate the agent-company `moultbook` field (ADR-005).
    pub moultbook: Option<String>,
}

// ──────────────────────────────────────────────
// WAVS Config (Layer.xyz)
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavsConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub use_tee: bool,
    pub operator_set: Option<String>,
}

impl Default for WavsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            use_tee: false,
            operator_set: None,
        }
    }
}
