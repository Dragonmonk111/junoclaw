use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ActiveBankResponse, BankHistoryResponse, BankListResponse, ExecuteMsg, InstantiateMsg,
    MigrateMsg, QueryMsg,
};
use crate::state::{
    BankStatus, ConceptThreshold, Config, ProbeBank, ACTIVE_BANK, BANKS, BANKS_BY_MODEL, CONFIG,
};

const CONTRACT_NAME: &str = "crates.io:junoclaw-probe-registry";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_MIN_CONCEPTS: u32 = 10;
const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = match msg.admin {
        Some(a) => deps.api.addr_validate(&a)?,
        None => info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(
        deps.storage,
        &Config {
            admin,
            min_concepts: msg.min_concepts.unwrap_or(DEFAULT_MIN_CONCEPTS),
            require_calibration: msg.require_calibration.unwrap_or(true),
        },
    )?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterProbeBank {
            model_id,
            model_revision,
            weights_hash,
            layers,
            concepts,
            method,
            bank_hash,
            uri,
            calibration_hash,
            thresholds,
        } => register_bank(
            deps,
            env,
            info,
            model_id,
            model_revision,
            weights_hash,
            layers,
            concepts,
            method,
            bank_hash,
            uri,
            calibration_hash,
            thresholds,
        ),
        ExecuteMsg::ActivateProbeBank { bank_hash } => activate_bank(deps, env, info, bank_hash),
        ExecuteMsg::DeprecateBank { bank_hash } => deprecate_bank(deps, info, bank_hash),
        ExecuteMsg::RevokeBank { bank_hash, reason } => {
            revoke_bank(deps, info, bank_hash, reason)
        }
        ExecuteMsg::UpdateConfig {
            admin,
            min_concepts,
            require_calibration,
        } => update_config(deps, info, admin, min_concepts, require_calibration),
    }
}

fn register_bank(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    model_id: String,
    model_revision: String,
    weights_hash: String,
    layers: Vec<u32>,
    concepts: Vec<String>,
    method: String,
    bank_hash: String,
    uri: String,
    calibration_hash: String,
    thresholds: Vec<ConceptThreshold>,
) -> Result<Response, ContractError> {
    if BANKS.has(deps.storage, &bank_hash) {
        return Err(ContractError::BankExists { bank_hash });
    }

    let bank = ProbeBank {
        model_id: model_id.clone(),
        model_revision,
        weights_hash,
        layers,
        concepts,
        method,
        bank_hash: bank_hash.clone(),
        uri,
        calibration_hash,
        thresholds,
        status: BankStatus::Proposed,
        proposer: info.sender.clone(),
        proposed_at: env.block.time.seconds(),
        activated_at: None,
    };

    BANKS.save(deps.storage, &bank_hash, &bank)?;

    let mut model_banks = BANKS_BY_MODEL
        .may_load(deps.storage, &model_id)?
        .unwrap_or_default();
    model_banks.push(bank_hash.clone());
    BANKS_BY_MODEL.save(deps.storage, &model_id, &model_banks)?;

    Ok(Response::new()
        .add_attribute("action", "register_probe_bank")
        .add_attribute("model_id", model_id)
        .add_attribute("bank_hash", bank_hash)
        .add_attribute("status", "proposed"))
}

fn activate_bank(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bank_hash: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut bank = BANKS
        .load(deps.storage, &bank_hash)
        .map_err(|_| ContractError::BankNotFound {
            bank_hash: bank_hash.clone(),
        })?;

    if bank.status != BankStatus::Proposed {
        return Err(ContractError::BankNotProposed { bank_hash });
    }
    if (bank.concepts.len() as u32) < cfg.min_concepts {
        return Err(ContractError::TooFewConcepts {
            got: bank.concepts.len() as u32,
            min: cfg.min_concepts,
        });
    }
    if cfg.require_calibration && bank.calibration_hash.is_empty() {
        return Err(ContractError::CalibrationRequired {});
    }

    // Deprecate the previously active bank for this exact weights pin.
    let key = (bank.model_id.as_str(), bank.weights_hash.as_str());
    if let Some(old_hash) = ACTIVE_BANK.may_load(deps.storage, key)? {
        if old_hash != bank_hash {
            let mut old = BANKS.load(deps.storage, &old_hash)?;
            old.status = BankStatus::Deprecated;
            BANKS.save(deps.storage, &old_hash, &old)?;
        }
    }

    bank.status = BankStatus::Active;
    bank.activated_at = Some(env.block.time.seconds());
    BANKS.save(deps.storage, &bank_hash, &bank)?;
    ACTIVE_BANK.save(deps.storage, key, &bank_hash)?;

    Ok(Response::new()
        .add_attribute("action", "activate_probe_bank")
        .add_attribute("model_id", bank.model_id)
        .add_attribute("weights_hash", bank.weights_hash)
        .add_attribute("bank_hash", bank_hash))
}

fn deprecate_bank(
    deps: DepsMut,
    info: MessageInfo,
    bank_hash: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut bank = BANKS
        .load(deps.storage, &bank_hash)
        .map_err(|_| ContractError::BankNotFound {
            bank_hash: bank_hash.clone(),
        })?;
    if bank.status != BankStatus::Active {
        return Err(ContractError::BankNotActive { bank_hash });
    }

    bank.status = BankStatus::Deprecated;
    BANKS.save(deps.storage, &bank_hash, &bank)?;
    ACTIVE_BANK.remove(
        deps.storage,
        (bank.model_id.as_str(), bank.weights_hash.as_str()),
    );

    Ok(Response::new()
        .add_attribute("action", "deprecate_bank")
        .add_attribute("bank_hash", bank_hash))
}

fn revoke_bank(
    deps: DepsMut,
    info: MessageInfo,
    bank_hash: String,
    reason: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut bank = BANKS
        .load(deps.storage, &bank_hash)
        .map_err(|_| ContractError::BankNotFound {
            bank_hash: bank_hash.clone(),
        })?;

    if bank.status == BankStatus::Active {
        ACTIVE_BANK.remove(
            deps.storage,
            (bank.model_id.as_str(), bank.weights_hash.as_str()),
        );
    }

    bank.status = BankStatus::Revoked;
    BANKS.save(deps.storage, &bank_hash, &bank)?;

    Ok(Response::new()
        .add_attribute("action", "revoke_bank")
        .add_attribute("bank_hash", bank_hash)
        .add_attribute("reason", reason))
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    min_concepts: Option<u32>,
    require_calibration: Option<bool>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(a) = admin {
        cfg.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(m) = min_concepts {
        cfg.min_concepts = m;
    }
    if let Some(r) = require_calibration {
        cfg.require_calibration = r;
    }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::ActiveBank {
            model_id,
            weights_hash,
        } => {
            let bank = ACTIVE_BANK
                .may_load(deps.storage, (&model_id, &weights_hash))?
                .map(|h| BANKS.load(deps.storage, &h))
                .transpose()?;
            to_json_binary(&ActiveBankResponse { bank })
        }
        QueryMsg::GetBank { bank_hash } => {
            to_json_binary(&BANKS.load(deps.storage, &bank_hash)?)
        }
        QueryMsg::ListBanks {
            model_id,
            status,
            start_after,
            limit,
        } => {
            let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
            let iter: Box<dyn Iterator<Item = StdResult<(String, ProbeBank)>>> =
                if let Some(mid) = model_id {
                    let hashes = BANKS_BY_MODEL
                        .may_load(deps.storage, &mid)?
                        .unwrap_or_default();
                    Box::new(hashes.into_iter().map(move |h| {
                        BANKS
                            .load(deps.storage, &h)
                            .map(|b| (h.clone(), b))
                    }))
                } else {
                    Box::new(BANKS.range(deps.storage, None, None, Order::Ascending))
                };
            let banks: Vec<ProbeBank> = iter
                .filter_map(|r| r.ok())
                .filter(|(_, b)| status.as_ref().map_or(true, |s| &b.status == s))
                .skip_while(|(h, _)| start_after.as_ref().map_or(false, |s| h == s))
                .skip(if start_after.is_some() { 1 } else { 0 })
                .take(limit)
                .map(|(_, b)| b)
                .collect();
            to_json_binary(&BankListResponse { banks })
        }
        QueryMsg::BankHistory { model_id } => {
            let hashes = BANKS_BY_MODEL
                .may_load(deps.storage, &model_id)?
                .unwrap_or_default();
            let banks: Vec<ProbeBank> = hashes
                .iter()
                .filter_map(|h| BANKS.load(deps.storage, h).ok())
                .collect();
            to_json_binary(&BankHistoryResponse { banks })
        }
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}
