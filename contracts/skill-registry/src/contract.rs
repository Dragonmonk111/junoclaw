use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{Config, RegistryStats, SkillEntry, CONFIG, REGISTRY_STATS, SKILLS};

const CONTRACT_NAME: &str = "crates.io:junoclaw-skill-registry";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = msg
        .admin
        .map(|a| deps.api.addr_validate(&a))
        .transpose()?
        .unwrap_or(info.sender.clone());

    let config = Config {
        admin,
        denom: msg.denom.unwrap_or_else(|| "ujuno".to_string()),
        registration_fee: msg.registration_fee,
    };
    CONFIG.save(deps.storage, &config)?;
    REGISTRY_STATS.save(deps.storage, &RegistryStats { total_entries: 0 })?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.to_string())
        .add_attribute("registration_fee", config.registration_fee.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::PublishSkill {
            dapp_name,
            chain_id,
            skill_uri,
            skill_hash,
        } => execute_publish(deps, env, info, dapp_name, chain_id, skill_uri, skill_hash),
        ExecuteMsg::UpdateSkill {
            dapp_name,
            chain_id,
            skill_uri,
            skill_hash,
        } => execute_update(deps, env, info, dapp_name, chain_id, skill_uri, skill_hash),
        ExecuteMsg::RemoveSkill { dapp_name } => execute_remove(deps, info, dapp_name),
        ExecuteMsg::TransferPublisher {
            dapp_name,
            new_publisher,
        } => execute_transfer_publisher(deps, info, dapp_name, new_publisher),
        ExecuteMsg::UpdateConfig {
            admin,
            registration_fee,
        } => execute_update_config(deps, info, admin, registration_fee),
    }
}

fn execute_publish(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dapp_name: String,
    chain_id: String,
    skill_uri: String,
    skill_hash: String,
) -> Result<Response, ContractError> {
    if dapp_name.trim().is_empty() {
        return Err(ContractError::EmptyName {});
    }
    if skill_uri.trim().is_empty() {
        return Err(ContractError::EmptyUri {});
    }
    if skill_hash.trim().is_empty() {
        return Err(ContractError::EmptyHash {});
    }

    if SKILLS.has(deps.storage, &dapp_name) {
        return Err(ContractError::NameAlreadyClaimed { dapp_name });
    }

    let config = CONFIG.load(deps.storage)?;
    if !config.registration_fee.is_zero() {
        let sent = info
            .funds
            .iter()
            .find(|c| c.denom == config.denom)
            .map(|c| c.amount)
            .unwrap_or(Uint128::zero());
        if sent < config.registration_fee {
            return Err(ContractError::InsufficientFee {
                required: config.registration_fee,
                sent,
            });
        }
    }

    let entry = SkillEntry {
        dapp_name: dapp_name.clone(),
        publisher: info.sender.clone(),
        chain_id: chain_id.clone(),
        skill_uri,
        skill_hash,
        version: 1,
        updated_at: env.block.height,
    };
    SKILLS.save(deps.storage, &dapp_name, &entry)?;

    REGISTRY_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_entries += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "publish_skill")
        .add_attribute("dapp_name", dapp_name)
        .add_attribute("publisher", info.sender.to_string())
        .add_attribute("chain_id", chain_id))
}

fn execute_update(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dapp_name: String,
    chain_id: Option<String>,
    skill_uri: Option<String>,
    skill_hash: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    SKILLS.update(deps.storage, &dapp_name, |existing| match existing {
        Some(mut entry) => {
            if entry.publisher != info.sender && info.sender != config.admin {
                return Err(ContractError::Unauthorized {});
            }
            if let Some(c) = chain_id {
                entry.chain_id = c;
            }
            if let Some(u) = skill_uri {
                if u.trim().is_empty() {
                    return Err(ContractError::EmptyUri {});
                }
                entry.skill_uri = u;
            }
            if let Some(h) = skill_hash {
                if h.trim().is_empty() {
                    return Err(ContractError::EmptyHash {});
                }
                entry.skill_hash = h;
            }
            entry.version += 1;
            entry.updated_at = env.block.height;
            Ok(entry)
        }
        None => Err(ContractError::SkillNotFound { dapp_name: dapp_name.clone() }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_skill")
        .add_attribute("dapp_name", dapp_name))
}

fn execute_remove(
    deps: DepsMut,
    info: MessageInfo,
    dapp_name: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if !SKILLS.has(deps.storage, &dapp_name) {
        return Err(ContractError::SkillNotFound { dapp_name });
    }
    SKILLS.remove(deps.storage, &dapp_name);

    REGISTRY_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_entries = s.total_entries.saturating_sub(1);
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "remove_skill")
        .add_attribute("dapp_name", dapp_name))
}

fn execute_transfer_publisher(
    deps: DepsMut,
    info: MessageInfo,
    dapp_name: String,
    new_publisher: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let new_publisher_addr = deps.api.addr_validate(&new_publisher)?;

    SKILLS.update(deps.storage, &dapp_name, |existing| match existing {
        Some(mut entry) => {
            entry.publisher = new_publisher_addr.clone();
            Ok(entry)
        }
        None => Err(ContractError::SkillNotFound { dapp_name: dapp_name.clone() }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "transfer_publisher")
        .add_attribute("dapp_name", dapp_name)
        .add_attribute("new_publisher", new_publisher))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    registration_fee: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(f) = registration_fee {
        config.registration_fee = f;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetSkill { dapp_name } => to_json_binary(&SKILLS.load(deps.storage, &dapp_name)?),
        QueryMsg::ListSkills { start_after, limit } => {
            let limit = limit.unwrap_or(30).min(100) as usize;
            let start = start_after
                .as_deref()
                .map(cw_storage_plus::Bound::exclusive);
            let entries: Vec<SkillEntry> = SKILLS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, e)| e))
                .collect();
            to_json_binary(&entries)
        }
        QueryMsg::SearchByChain {
            chain_id,
            start_after,
            limit,
        } => {
            let limit = limit.unwrap_or(30).min(100) as usize;
            let start = start_after
                .as_deref()
                .map(cw_storage_plus::Bound::exclusive);
            let entries: Vec<SkillEntry> = SKILLS
                .range(deps.storage, start, None, Order::Ascending)
                .filter_map(|r| r.ok().map(|(_, e)| e))
                .filter(|e| e.chain_id == chain_id)
                .take(limit)
                .collect();
            to_json_binary(&entries)
        }
        QueryMsg::GetStats {} => to_json_binary(&REGISTRY_STATS.load(deps.storage)?),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::Unauthorized {});
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
