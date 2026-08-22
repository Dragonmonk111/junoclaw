//! Miner identity — robot or GPU, enrolled in the truth market.
//!
//! A robot that mines truth during idle time uses its jclaw-credential
//! (soulbound, non-transferable) as its identity. A standalone GPU miner
//! gets a similar identity — same pattern, different enrollment.
//!
//! IMPORTANT: Only open-weight models qualify as J-Lens miners. Closed-weight
//! API models (GPT-4o, Claude, etc.) cannot be verified — the miner can't prove
//! what model ran or that it ran faithfully. Open-weight models (Llama, Qwen,
//! Mistral, etc.) running on hardware the miner controls are verifiable.
//! Akash TEE deployments count because the TEE attests to the exact model
//! and inference that ran inside the enclave.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Type of miner identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityType {
    /// Robot with a jclaw-credential (soulbound). Mines truth during idle time.
    /// Example: Jetson Orin running Qwen-3B, mining truth after housekeeping.
    Robot,
    /// Bare-metal GPU miner running open-weight models.
    /// Example: 4×DGX Spark running Llama-70B in someone's garage.
    GpuMiner,
    /// Akash deployment with TEE (Trusted Execution Environment).
    /// The TEE attests to the exact model and inference that ran inside
    /// the enclave, providing verifiable computation without owning hardware.
    /// Example: Akash H100 with confidential computing, running Mistral-8x22B.
    AkashTeeMiner,
}

/// Whether the model's weights are open or closed.
/// J-Lens miners MUST use open-weight models — closed-weight API calls
/// (GPT-4o, Claude, Gemini) are not verifiable and do not qualify.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelWeightType {
    /// Open-weight model (Llama, Qwen, Mistral, DeepSeek, etc.)
    /// Miner runs inference locally and can prove what model ran.
    OpenWeight,
    /// Open-weight model running inside a TEE enclave on Akash.
    /// TEE attestation proves the exact model and inference.
    OpenWeightTee,
}

/// Miner identity — who is evaluating and on what hardware.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinerIdentity {
    /// Type of miner
    pub identity_type: IdentityType,
    /// Whether the model weights are open or TEE-attested
    pub weight_type: ModelWeightType,
    /// Juno wallet address (used for staking and rewards)
    pub address: String,
    /// Self-reported model identifier (e.g., "llama-70b", "qwen-3b", "mistral-8x22b")
    /// MUST be an open-weight model. Closed-weight APIs (GPT-4o, Claude) do not qualify.
    pub model_id: String,
    /// Self-reported hardware identifier (e.g., "jetson-orin", "dgx-spark", "akash-h100-tee")
    pub hardware_id: String,
    /// Optional jclaw-credential token ID (for robot miners)
    pub credential_token_id: Option<u64>,
    /// Optional TEE attestation hash (for Akash TEE miners)
    pub tee_attestation: Option<String>,
    /// Optional geographic region hint (for diversity, not enforced)
    pub region: Option<String>,
}

impl MinerIdentity {
    /// Create a robot miner identity (e.g., Jetson Orin on a vacuum robot).
    /// Mines truth during idle time using an open-weight model.
    pub fn robot(
        address: &str,
        model_id: &str,
        hardware_id: &str,
        credential_token_id: Option<u64>,
    ) -> Self {
        Self {
            identity_type: IdentityType::Robot,
            weight_type: ModelWeightType::OpenWeight,
            address: address.to_string(),
            model_id: model_id.to_string(),
            hardware_id: hardware_id.to_string(),
            credential_token_id,
            tee_attestation: None,
            region: None,
        }
    }

    /// Create a GPU miner identity (bare-metal rig running open-weight model).
    pub fn gpu_miner(address: &str, model_id: &str, hardware_id: &str) -> Self {
        Self {
            identity_type: IdentityType::GpuMiner,
            weight_type: ModelWeightType::OpenWeight,
            address: address.to_string(),
            model_id: model_id.to_string(),
            hardware_id: hardware_id.to_string(),
            credential_token_id: None,
            tee_attestation: None,
            region: None,
        }
    }

    /// Create an Akash TEE miner identity (open-weight model in TEE enclave).
    /// The TEE attestation proves the exact model and inference that ran.
    pub fn akash_tee_miner(
        address: &str,
        model_id: &str,
        hardware_id: &str,
        tee_attestation: &str,
    ) -> Self {
        Self {
            identity_type: IdentityType::AkashTeeMiner,
            weight_type: ModelWeightType::OpenWeightTee,
            address: address.to_string(),
            model_id: model_id.to_string(),
            hardware_id: hardware_id.to_string(),
            credential_token_id: None,
            tee_attestation: Some(tee_attestation.to_string()),
            region: None,
        }
    }

    /// Compute the fingerprint hash for the truth market contract.
    /// This is the `fingerprint` field in `RegisterOperator`.
    pub fn fingerprint_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.model_id);
        hasher.update(b":");
        hasher.update(&self.hardware_id);
        hasher.update(b":");
        hasher.update(match self.weight_type {
            ModelWeightType::OpenWeight => b"open".as_slice(),
            ModelWeightType::OpenWeightTee => b"tee".as_slice(),
        });
        if let Some(ref token) = self.credential_token_id {
            hasher.update(b":credential:");
            hasher.update(token.to_string().as_bytes());
        }
        if let Some(ref attestation) = self.tee_attestation {
            hasher.update(b":tee:");
            hasher.update(attestation.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Human-readable description for logging.
    pub fn description(&self) -> String {
        match self.identity_type {
            IdentityType::Robot => {
                format!("robot[{}]: {} on {}", self.address, self.model_id, self.hardware_id)
            }
            IdentityType::GpuMiner => {
                format!("gpu[{}]: {} on {}", self.address, self.model_id, self.hardware_id)
            }
            IdentityType::AkashTeeMiner => {
                format!("akash-tee[{}]: {} on {}", self.address, self.model_id, self.hardware_id)
            }
        }
    }

    /// Check if this identity uses a verifiable model (open-weight or TEE-attested).
    /// Closed-weight API models are NOT verifiable and do not qualify as J-Lens miners.
    pub fn is_verifiable(&self) -> bool {
        matches!(self.weight_type, ModelWeightType::OpenWeight | ModelWeightType::OpenWeightTee)
    }
}
