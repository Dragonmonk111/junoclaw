use cosmwasm_std::{Addr, Binary};
use cw_storage_plus::Item;

pub const CONFIG: Item<Config> = Item::new("config");
pub const STORED_PROOF: Item<StoredProof> = Item::new("stored_proof");
pub const LAST_VERIFY: Item<LastVerifyResult> = Item::new("last_verify");
pub const VERIFIER_MODE: Item<String> = Item::new("verifier_mode");
/// Serialized JoltVerifierPreprocessing (bincode 2) — the verifying key for
/// Phase 2 full cryptographic verification.
pub const VERIFYING_KEY: Item<Binary> = Item::new("verifying_key");

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Config {
    pub admin: Addr,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct StoredProof {
    pub data: Vec<u8>,
    pub program_hash: Option<String>,
    pub proof_hash: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LastVerifyResult {
    pub verified: bool,
    pub block_height: u64,
    pub error_msg: Option<String>,
}
