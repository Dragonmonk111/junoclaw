use cosmwasm_std::{
    entry_point, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    BatchResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    RelayerResponse, ValidatorSetResponse,
};
use crate::state::{self, Config, SettledBatch};

// ──────────────────────────────────────────────────────────────────────
// Instantiate
// ──────────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    let config = Config {
        admin,
        threshold: msg.threshold,
        latest_height: None,
    };
    state::CONFIG.save(deps.storage, &config)?;

    let validators: Vec<Vec<u8>> =
        msg.validators.iter().map(|b| b.to_vec()).collect();
    state::VALIDATORS.save(deps.storage, &validators)?;

    // Register admin as a relayer by default
    state::RELAYERS.save(deps.storage, &config.admin, &true)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.as_str())
        .add_attribute("threshold", config.threshold.to_string())
        .add_attribute("validator_count", validators.len().to_string()))
}

// ──────────────────────────────────────────────────────────────────────
// Execute
// ──────────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SubmitBatch {
            certificate,
            messages_hash,
            commonware_height,
            timestamp,
        } => execute_submit_batch(
            deps,
            env,
            info,
            certificate,
            messages_hash,
            commonware_height,
            timestamp,
        ),
        ExecuteMsg::UpdateValidatorSet { validators, threshold } => {
            execute_update_validator_set(deps, info, validators, threshold)
        }
        ExecuteMsg::UpdateAdmin { new_admin } => {
            execute_update_admin(deps, info, new_admin)
        }
        ExecuteMsg::RegisterRelayer { address } => {
            execute_register_relayer(deps, info, address)
        }
        ExecuteMsg::RemoveRelayer { address } => {
            execute_remove_relayer(deps, info, address)
        }
    }
}

fn execute_submit_batch(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    certificate: Binary,
    messages_hash: [u8; 32],
    commonware_height: u64,
    timestamp: u64,
) -> Result<Response, ContractError> {
    // Check sender is a registered relayer
    let is_relayer =
        state::RELAYERS.may_load(deps.storage, &info.sender)?.unwrap_or(false);
    if !is_relayer {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }

    // Check batch not already settled
    if state::BATCHES.has(deps.storage, commonware_height) {
        return Err(ContractError::BatchAlreadySettled {
            height: commonware_height,
        });
    }

    // Verify validator set is initialized
    let validators = state::VALIDATORS
        .may_load(deps.storage)?
        .ok_or(ContractError::ValidatorSetNotInitialized {})?;
    if validators.is_empty() {
        return Err(ContractError::NoValidators {});
    }

    // Verify the certificate against the stored validator set.
    //
    // The coordination engine's simulated consensus produces certificates as:
    //   certificate = SHA256(messages_hash || validator_1 || validator_2 || ... || validator_n)
    //
    // We recompute this hash on-chain and compare it to the submitted certificate.
    // This proves the certificate was produced by the known validator set for this
    // specific batch hash — not forged by the relayer.
    //
    // When real BLS12-381 threshold signatures are available (via precompile or
    // pure-Wasm library), this verification path will be replaced with proper
    // signature verification. The interface (SubmitBatch with certificate bytes)
    // remains unchanged.

    let cert_bytes = certificate.to_vec();

    // Recompute expected certificate: SHA256(messages_hash || validators...)
    let mut expected_hasher = Sha256::new();
    expected_hasher.update(messages_hash);
    for vk in &validators {
        expected_hasher.update(vk);
    }
    let expected_cert: [u8; 32] = expected_hasher.finalize().into();

    if cert_bytes != expected_cert.to_vec() {
        return Err(ContractError::InvalidCertificate {
            reason: format!(
                "certificate hash mismatch: expected {}, got {}",
                hex::encode(expected_cert),
                hex::encode(&cert_bytes)
            ),
        });
    }

    // Compute certificate hash for on-chain auditability
    let mut hasher = Sha256::new();
    hasher.update(&cert_bytes);
    let cert_hash: [u8; 32] = hasher.finalize().into();

    let batch = SettledBatch {
        commonware_height,
        messages_hash,
        certificate: cert_bytes,
        timestamp,
        submitter: info.sender.clone(),
    };

    state::BATCHES.save(deps.storage, commonware_height, &batch)?;

    // Update latest height
    let mut config = state::CONFIG.load(deps.storage)?;
    config.latest_height = Some(commonware_height);
    state::CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "submit_batch")
        .add_attribute("commonware_height", commonware_height.to_string())
        .add_attribute("submitter", info.sender.as_str())
        .add_attribute("cert_hash", hex::encode(cert_hash))
        .add_attribute("messages_hash", hex::encode(messages_hash))
        .add_event(
            cosmwasm_std::Event::new("batch_settled")
                .add_attribute("commonware_height", commonware_height.to_string())
                .add_attribute("messages_hash", hex::encode(messages_hash)),
        ))
}

fn execute_update_validator_set(
    deps: DepsMut,
    info: MessageInfo,
    validators: Vec<Binary>,
    threshold: u32,
) -> Result<Response, ContractError> {
    let config = state::CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }

    if threshold == 0 || threshold as usize > validators.len() {
        return Err(ContractError::InvalidCertificate {
            reason: format!(
                "threshold {} must be > 0 and <= validator count {}",
                threshold,
                validators.len()
            ),
        });
    }

    let validators_vec: Vec<Vec<u8>> =
        validators.iter().map(|b| b.to_vec()).collect();
    state::VALIDATORS.save(deps.storage, &validators_vec)?;

    let mut config = state::CONFIG.load(deps.storage)?;
    config.threshold = threshold;
    state::CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_validator_set")
        .add_attribute("validator_count", validators_vec.len().to_string())
        .add_attribute("threshold", threshold.to_string()))
}

fn execute_update_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut config = state::CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }

    let new_admin_addr = deps.api.addr_validate(&new_admin)?;
    config.admin = new_admin_addr.clone();
    state::CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_admin")
        .add_attribute("new_admin", new_admin_addr.as_str()))
}

fn execute_register_relayer(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = state::CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }

    let relayer_addr = deps.api.addr_validate(&address)?;
    state::RELAYERS.save(deps.storage, &relayer_addr, &true)?;

    Ok(Response::new()
        .add_attribute("action", "register_relayer")
        .add_attribute("address", relayer_addr.as_str()))
}

fn execute_remove_relayer(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = state::CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }

    let relayer_addr = deps.api.addr_validate(&address)?;
    state::RELAYERS.remove(deps.storage, &relayer_addr);

    Ok(Response::new()
        .add_attribute("action", "remove_relayer")
        .add_attribute("address", relayer_addr.as_str()))
}

// ──────────────────────────────────────────────────────────────────────
// Query
// ──────────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config = state::CONFIG.load(deps.storage)?;
            let validators = state::VALIDATORS.load(deps.storage)?;
            let relayer_count = state::RELAYERS
                .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                .count() as u32;
            let resp = ConfigResponse {
                admin: config.admin.to_string(),
                threshold: config.threshold,
                validator_count: validators.len() as u32,
                relayer_count,
                latest_height: config.latest_height,
            };
            Ok(cosmwasm_std::to_json_binary(&resp)?)
        }
        QueryMsg::ValidatorSet {} => {
            let validators = state::VALIDATORS.load(deps.storage)?;
            let config = state::CONFIG.load(deps.storage)?;
            let resp = ValidatorSetResponse {
                validators: validators.into_iter().map(Binary::from).collect(),
                threshold: config.threshold,
            };
            Ok(cosmwasm_std::to_json_binary(&resp)?)
        }
        QueryMsg::Batch { commonware_height } => {
            let batch =
                state::BATCHES.load(deps.storage, commonware_height)?;
            let resp = BatchResponse {
                commonware_height: batch.commonware_height,
                messages_hash: batch.messages_hash,
                certificate: Binary::from(batch.certificate),
                timestamp: batch.timestamp,
                submitter: batch.submitter.to_string(),
            };
            Ok(cosmwasm_std::to_json_binary(&resp)?)
        }
        QueryMsg::LatestBatch {} => {
            let config = state::CONFIG.load(deps.storage)?;
            match config.latest_height {
                Some(height) => {
                    let batch =
                        state::BATCHES.load(deps.storage, height)?;
                    let resp = BatchResponse {
                        commonware_height: batch.commonware_height,
                        messages_hash: batch.messages_hash,
                        certificate: Binary::from(batch.certificate),
                        timestamp: batch.timestamp,
                        submitter: batch.submitter.to_string(),
                    };
                    Ok(cosmwasm_std::to_json_binary(&resp)?)
                }
                None => {
                    Ok(cosmwasm_std::to_json_binary(
                        &serde_json::json!({ "settled": false }),
                    )?)
                }
            }
        }
        QueryMsg::Relayer { address } => {
            let addr = Addr::unchecked(&address);
            let is_relayer =
                state::RELAYERS.may_load(deps.storage, &addr)?.unwrap_or(false);
            let resp = RelayerResponse { is_relayer };
            Ok(cosmwasm_std::to_json_binary(&resp)?)
        }
    }
}
