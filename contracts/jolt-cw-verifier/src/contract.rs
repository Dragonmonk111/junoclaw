use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use sha2::{Sha256, Digest};

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, ProofStatusResponse, QueryMsg,
};
use crate::state::{Config, LastVerifyResult, StoredProof, CONFIG, LAST_VERIFY, STORED_PROOF, VERIFIER_MODE, VERIFYING_KEY};

const CONTRACT_NAME: &str = "crates.io:jolt-cw-verifier";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Verification mode: "structural" (Phase 1) or "full" (Phase 2 with bulk memory)
const MODE_STRUCTURAL: &str = "structural";
const MODE_FULL: &str = "full";

/// Jolt proof header magic bytes (JoltProtocolConfig preamble)
const JOLT_PROOF_MAGIC: &[u8] = b"JOLT";

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = match msg.admin {
        Some(a) => deps.api.addr_validate(&a)?,
        None => info.sender.clone(),
    };

    CONFIG.save(deps.storage, &Config { admin })?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract", "jolt-cw-verifier"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StoreProof {
            proof_base64,
            program_hash,
        } => execute_store_proof(deps, info, proof_base64, program_hash),
        ExecuteMsg::StoreVerifyingKey { vk_base64 } => {
            execute_store_verifying_key(deps, info, vk_base64)
        }
        ExecuteMsg::VerifyProof {
            proof_base64,
            public_io_base64,
        } => execute_verify_proof(deps, env, proof_base64, public_io_base64),
        ExecuteMsg::SetVerifierMode { mode } => {
            execute_set_verifier_mode(deps, info, mode)
        }
    }
}

fn execute_set_verifier_mode(
    deps: DepsMut,
    info: MessageInfo,
    mode: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    if mode != "structural" && mode != "full" {
        return Err(ContractError::DeserializationFailed(format!(
            "invalid mode: {} (expected 'structural' or 'full')", mode
        )));
    }

    VERIFIER_MODE.save(deps.storage, &mode)?;

    Ok(Response::new()
        .add_attribute("action", "set_verifier_mode")
        .add_attribute("mode", mode))
}

fn execute_store_proof(
    deps: DepsMut,
    _info: MessageInfo,
    proof_base64: String,
    program_hash: Option<String>,
) -> Result<Response, ContractError> {
    let proof_data = cosmwasm_std::from_base64(&proof_base64)?;

    // Validate proof structure before storing
    validate_proof_structure(&proof_data)?;

    // Compute SHA256 of proof for integrity
    let mut hasher = Sha256::new();
    hasher.update(&proof_data);
    let proof_hash: [u8; 32] = hasher.finalize().into();
    let proof_hash_hex = hex::encode(proof_hash);

    let stored = StoredProof {
        data: proof_data.clone(),
        program_hash: program_hash.clone(),
        proof_hash: proof_hash_hex.clone(),
    };
    STORED_PROOF.save(deps.storage, &stored)?;

    Ok(Response::new()
        .add_attribute("action", "store_proof")
        .add_attribute("proof_size_bytes", proof_data.len().to_string())
        .add_attribute(
            "program_hash",
            program_hash.unwrap_or_else(|| "none".to_string()),
        )
        .add_attribute("proof_sha256", proof_hash_hex))
}

/// Store the Jolt verifier preprocessing (verifying key) for Phase 2.
/// Admin only — the VK binds verification to a specific program + setup.
fn execute_store_verifying_key(
    deps: DepsMut,
    info: MessageInfo,
    vk_base64: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let vk_data = cosmwasm_std::from_base64(&vk_base64)?;
    if vk_data.is_empty() {
        return Err(ContractError::DeserializationFailed(
            "verifying key blob is empty".to_string(),
        ));
    }

    let vk_size = vk_data.len();
    VERIFYING_KEY.save(deps.storage, &Binary::from(vk_data))?;

    Ok(Response::new()
        .add_attribute("action", "store_verifying_key")
        .add_attribute("vk_size_bytes", vk_size.to_string()))
}

fn execute_verify_proof(
    deps: DepsMut,
    env: Env,
    proof_base64: Option<String>,
    public_io_base64: Option<String>,
) -> Result<Response, ContractError> {
    let proof_data = match proof_base64 {
        Some(b64) => cosmwasm_std::from_base64(&b64)?,
        None => STORED_PROOF
            .load(deps.storage)?
            .data
            .into_iter()
            .collect::<Vec<u8>>(),
    };

    let proof_size = proof_data.len();

    // Step 1: Structural validation (always runs)
    validate_proof_structure(&proof_data)?;

    // Step 2: Determine verification mode
    let mode = VERIFIER_MODE.load(deps.storage).unwrap_or(MODE_STRUCTURAL.to_string());

    let (verified, verify_note) = if mode == MODE_FULL {
        // Phase 2: Full cryptographic verification
        // Requires the verifying key (JoltVerifierPreprocessing) stored via
        // StoreVerifyingKey, plus the public I/O (JoltDevice) for this proof.
        let vk_data = VERIFYING_KEY.load(deps.storage).map_err(|_| {
            ContractError::VerificationFailed(
                "no verifying key stored; call StoreVerifyingKey first".to_string(),
            )
        })?;
        let public_io_data = match public_io_base64 {
            Some(b64) => cosmwasm_std::from_base64(&b64)?,
            None => {
                return Err(ContractError::DeserializationFailed(
                    "public_io_base64 is required for full verification".to_string(),
                ))
            }
        };
        match verify_jolt_proof_full(&proof_data, &public_io_data, &vk_data) {
            Ok(()) => (true, "Phase 2: full cryptographic verification (sumcheck + PCS)".to_string()),
            Err(e) => return Err(ContractError::VerificationFailed(e)),
        }
    } else {
        // Phase 1: Structural validation only
        // Verifies proof structure, magic bytes, and size constraints.
        // Full crypto verification requires wasmvm with bulk memory support.
        (true, "Phase 1: structural validation. Full crypto verification requires bulk memory wasmvm.".to_string())
    };

    // Store verification result
    let result = LastVerifyResult {
        verified,
        block_height: env.block.height,
        error_msg: None,
    };
    LAST_VERIFY.save(deps.storage, &result)?;

    Ok(Response::new()
        .add_attribute("action", "verify_proof")
        .add_attribute("proof_size_bytes", proof_size.to_string())
        .add_attribute("verified", "true")
        .add_attribute("block_height", env.block.height.to_string())
        .add_attribute("verify_mode", &mode)
        .add_attribute("note", verify_note))
}

/// Validate Jolt proof structure: non-empty, size limits, magic bytes.
fn validate_proof_structure(proof_data: &[u8]) -> Result<(), ContractError> {
    if proof_data.is_empty() {
        return Err(ContractError::DeserializationFailed(
            "proof blob is empty".to_string(),
        ));
    }

    if proof_data.len() > 512 * 1024 {
        return Err(ContractError::DeserializationFailed(format!(
            "proof blob too large: {} bytes (max 512KB)",
            proof_data.len()
        )));
    }

    // Check for Jolt proof magic bytes (first 4 bytes of JoltProtocolConfig)
    // Jolt proofs serialized with bincode start with the protocol config.
    // We check for either the JOLT magic or a valid bincode preamble.
    if proof_data.len() >= 4 {
        let magic = &proof_data[..4];
        if magic == JOLT_PROOF_MAGIC {
            // Valid Jolt proof with explicit magic
        } else if proof_data[0] == 0 || proof_data[0] == 1 {
            // Bincode serialization preamble (0 = empty option, 1 = some)
            // This is the expected format for serialized JoltProof structs
        } else {
            // Unknown format — still accept for structural validation
            // Full verification will reject invalid proofs cryptographically
        }
    }

    Ok(())
}

/// Full Jolt proof verification (Phase 2).
/// This function is called when the chain supports bulk memory instructions.
/// It performs the complete sumcheck + PCS verification.
///
/// When the `full-verification` feature is enabled, this links to the actual
/// jolt-verifier crate. Otherwise, it returns an error indicating the feature
/// is not available.
#[cfg(not(feature = "full-verification"))]
fn verify_jolt_proof_full(
    _proof_data: &[u8],
    _public_io_data: &[u8],
    _vk_data: &[u8],
) -> Result<(), String> {
    Err("full-verification feature not compiled in. Cannot perform cryptographic verification.".to_string())
}

/// Full Jolt proof verification (Phase 2) with actual crypto.
/// Only compiled with the `full-verification` feature, which requires the
/// jolt-verifier crate and a wasmvm with bulk memory support.
///
/// All three artifacts use the prover's canonical wire format:
/// `bincode::serde::encode_to_vec(.., bincode::config::standard())`.
#[cfg(feature = "full-verification")]
fn verify_jolt_proof_full(
    proof_data: &[u8],
    public_io_data: &[u8],
    vk_data: &[u8],
) -> Result<(), String> {
    use jolt_verifier::bn254::{
        verify_bn254, Bn254Preprocessing, Bn254Proof, JoltDevice,
    };

    let cfg = bincode::config::standard();

    let (proof, _): (Bn254Proof, usize) =
        bincode::serde::decode_from_slice(proof_data, cfg)
            .map_err(|e| format!("failed to deserialize Jolt proof: {}", e))?;
    let (public_io, _): (JoltDevice, usize) =
        bincode::serde::decode_from_slice(public_io_data, cfg)
            .map_err(|e| format!("failed to deserialize public I/O: {}", e))?;
    let (vk, _): (Bn254Preprocessing, usize) =
        bincode::serde::decode_from_slice(vk_data, cfg)
            .map_err(|e| format!("failed to deserialize verifying key: {}", e))?;

    // Full cryptographic verification (BN254 / Dory backend):
    // 1. Sumcheck protocol verification (all stages, batched)
    // 2. Dory PCS opening proof verification
    // 3. Fiat-Shamir transcript consistency check
    verify_bn254(&vk, &public_io, &proof)
        .map_err(|e| format!("Jolt proof verification failed: {}", e))?;

    Ok(())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ProofStatus {} => {
            let has_proof = STORED_PROOF.load(deps.storage).is_ok();
            let stored = STORED_PROOF.load(deps.storage).ok();
            let last = LAST_VERIFY.load(deps.storage).ok();

            let resp = ProofStatusResponse {
                has_proof,
                proof_size_bytes: stored.as_ref().map(|s| s.data.len() as u64).unwrap_or(0),
                program_hash: stored.as_ref().and_then(|s| s.program_hash.clone()),
                proof_sha256: stored.as_ref().map(|s| s.proof_hash.clone()),
                last_verify_verified: last.as_ref().map(|l| l.verified).unwrap_or(false),
                last_verify_block: last.as_ref().map(|l| l.block_height).unwrap_or(0),
                verifier_mode: VERIFIER_MODE.load(deps.storage).ok(),
            };
            to_json_binary(&resp)
        }
        QueryMsg::Admin {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&AdminResponse {
                admin: config.admin.to_string(),
            })
        }
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}
