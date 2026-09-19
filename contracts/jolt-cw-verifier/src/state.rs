use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const CONFIG: Item<Config> = Item::new("config");
/// Proof metadata only — the proof bytes themselves are stored raw under
/// `PROOF_DATA_KEY` (Item<T> JSON-serializes, which inflates Vec<u8> ~3.5x
/// and Binary ~1.33x, exceeding the 128KB MAX_LENGTH_DB_VALUE limit).
pub const STORED_PROOF: Item<StoredProof> = Item::new("stored_proof");
pub const LAST_VERIFY: Item<LastVerifyResult> = Item::new("last_verify");
pub const VERIFIER_MODE: Item<String> = Item::new("verifier_mode");

/// Raw storage key for the serialized Jolt proof bytes.
/// Written via `deps.storage.set(PROOF_DATA_KEY, &bytes)` — no JSON overhead.
pub const PROOF_DATA_KEY: &[u8] = b"proof_data";
/// Raw storage key for the serialized JoltVerifierPreprocessing (bincode 2) —
/// the verifying key for Phase 2 full cryptographic verification.
pub const VERIFYING_KEY_KEY: &[u8] = b"verifying_key";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Config {
    pub admin: Addr,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct StoredProof {
    /// Byte length of the proof stored under PROOF_DATA_KEY.
    pub size: u64,
    pub program_hash: Option<String>,
    pub proof_hash: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LastVerifyResult {
    pub verified: bool,
    pub block_height: u64,
    pub error_msg: Option<String>,
}
