use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, LastVerifyResponse, MigrateMsg, QueryMsg, VkStatusResponse};
use crate::state::{Config, LastVerification, CONFIG, LAST_VERIFICATION, VK_BYTES};

// Arkworks imports (no_std compatible).
// `Groth16` and `prepare_verifying_key` moved to `crate::bn254_backend`, which
// is the single dispatch point for pure-Wasm vs precompile verification.
use ark_bn254::{Bn254, Fr};
use ark_groth16::{Proof, VerifyingKey};
use ark_ec::pairing::Pairing;
use ark_serialize::CanonicalDeserialize;

const CONTRACT_NAME: &str = "crates.io:junoclaw-zk-verifier";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
        .add_attribute("contract", "zk-verifier"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StoreVk { vk_base64 } => execute_store_vk(deps, info, vk_base64),
        ExecuteMsg::VerifyProof {
            proof_base64,
            public_inputs_base64,
        } => execute_verify_proof(deps, env, proof_base64, public_inputs_base64),
    }
}

fn execute_store_vk(
    deps: DepsMut,
    info: MessageInfo,
    vk_base64: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let vk_bytes = base64_decode(&vk_base64)?;

    // Validate that the bytes can be deserialized as a VerifyingKey<Bn254>
    let _vk = VerifyingKey::<Bn254>::deserialize_compressed(&vk_bytes[..])
        .map_err(|e| ContractError::DeserializationError {
            reason: format!("invalid verification key: {}", e),
        })?;

    let size = vk_bytes.len() as u64;
    VK_BYTES.save(deps.storage, &vk_bytes)?;

    Ok(Response::new()
        .add_attribute("action", "store_vk")
        .add_attribute("vk_size_bytes", size.to_string()))
}

fn execute_verify_proof(
    deps: DepsMut,
    env: Env,
    proof_base64: String,
    public_inputs_base64: String,
) -> Result<Response, ContractError> {
    let public_inputs_len = run_verify(deps.api, deps.storage, &proof_base64, &public_inputs_base64)?;

    let public_inputs = deserialize_public_inputs(&base64_decode(&public_inputs_base64)?)?;
    let _ = public_inputs;

    LAST_VERIFICATION.save(deps.storage, &LastVerification {
        verified: true,
        block_height: env.block.height,
    })?;

    Ok(Response::new()
        .add_attribute("action", "verify_proof")
        .add_attribute("result", "valid")
        .add_attribute("block_height", env.block.height.to_string())
        .add_event(
            cosmwasm_std::Event::new("zk_proof_verified")
                .add_attribute("result", "valid")
                .add_attribute("block_height", env.block.height.to_string())
                .add_attribute("public_inputs_count", public_inputs_len.to_string()),
        ))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::VkStatus {} => {
            let vk = VK_BYTES.may_load(deps.storage)?;
            to_json_binary(&VkStatusResponse {
                has_vk: vk.is_some(),
                vk_size_bytes: vk.map(|v| v.len() as u64).unwrap_or(0),
            })
        }
        QueryMsg::LastVerify {} => {
            let last = LAST_VERIFICATION.may_load(deps.storage)?;
            to_json_binary(&LastVerifyResponse {
                verified: last.as_ref().map(|l| l.verified).unwrap_or(false),
                block_height: last.map(|l| l.block_height).unwrap_or(0),
            })
        }
        QueryMsg::Verify {
            proof_base64,
            public_inputs_base64,
        } => {
            match run_verify(deps.api, deps.storage, &proof_base64, &public_inputs_base64) {
                Ok(_) => to_json_binary(&crate::msg::VerifyResponse {
                    valid: true,
                    reason: String::new(),
                }),
                Err(ContractError::ProofInvalid {}) => {
                    to_json_binary(&crate::msg::VerifyResponse {
                        valid: false,
                        reason: "proof verification failed".to_string(),
                    })
                }
                Err(e) => to_json_binary(&crate::msg::VerifyResponse {
                    valid: false,
                    reason: format!("{}", e),
                }),
            }
        }
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "migrate"))
}

// ── Helpers ──

fn base64_decode(input: &str) -> Result<Vec<u8>, ContractError> {
    // CosmWasm-compatible base64 decoding using cosmwasm_std
    cosmwasm_std::from_json::<cosmwasm_std::Binary>(
        &format!("\"{}\"", input).into_bytes(),
    )
    .map(|b| b.to_vec())
    .map_err(|e| ContractError::DeserializationError {
        reason: format!("base64: {}", e),
    })
}

fn deserialize_public_inputs(bytes: &[u8]) -> Result<Vec<<Bn254 as Pairing>::ScalarField>, ContractError> {
    // Public inputs are serialized as a sequence of Fr elements (32 bytes each)
    let fr_size = 32; // BN254 scalar field element size
    if bytes.len() % fr_size != 0 {
        return Err(ContractError::DeserializationError {
            reason: format!("public inputs length {} not a multiple of {}", bytes.len(), fr_size),
        });
    }
    let mut inputs = Vec::new();
    for chunk in bytes.chunks(fr_size) {
        let fr = Fr::deserialize_compressed(chunk)
            .map_err(|e| ContractError::DeserializationError {
                reason: format!("public input Fr: {}", e),
            })?;
        inputs.push(fr);
    }
    Ok(inputs)
}

/// Shared verification path for `VerifyProof` (execute) and `Verify`
/// (query). Returns the public-input count on success, or a
/// ContractError (ProofInvalid / deserialization / missing VK).
///
/// The expensive part is delegated to `bn254_backend::verify_groth16`,
/// which dispatches on the `bn254-precompile` cargo feature:
///   - off  : pure arkworks (~371k gas on uni-7)
///   - on   : 4-pair `bn254_pairing_equality` + host-side lincomb
///            (~77.6k gas measured on the patched wasmvm)
/// Both paths produce identical accept/reject decisions by design —
/// differential testing is documented in `wasmvm-fork/BUILD_AND_TEST.md`.
fn run_verify(
    api: &dyn cosmwasm_std::Api,
    storage: &dyn cosmwasm_std::Storage,
    proof_base64: &str,
    public_inputs_base64: &str,
) -> Result<usize, ContractError> {
    let vk_bytes = VK_BYTES
        .may_load(storage)?
        .ok_or(ContractError::NoVerificationKey {})?;

    let vk = VerifyingKey::<Bn254>::deserialize_compressed(&vk_bytes[..])
        .map_err(|e| ContractError::DeserializationError {
            reason: format!("vk: {}", e),
        })?;

    let proof_bytes = base64_decode(proof_base64)?;
    let proof = Proof::<Bn254>::deserialize_compressed(&proof_bytes[..])
        .map_err(|e| ContractError::DeserializationError {
            reason: format!("proof: {}", e),
        })?;

    let inputs_bytes = base64_decode(public_inputs_base64)?;
    let public_inputs = deserialize_public_inputs(&inputs_bytes)?;
    let n = public_inputs.len();

    let valid = crate::bn254_backend::verify_groth16(api, &vk, &proof, &public_inputs)?;
    if !valid {
        return Err(ContractError::ProofInvalid {});
    }
    Ok(n)
}
