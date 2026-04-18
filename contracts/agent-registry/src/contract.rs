use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{AgentStats, Config, AGENTS, AGENT_BY_OWNER, AGENT_STATS, CONFIG, NEXT_AGENT_ID};
use junoclaw_common::{AgentProfile, ContractRegistry};

const CONTRACT_NAME: &str = "crates.io:junoclaw-agent-registry";
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

    let registry = match msg.registry {
        Some(r) => {
            let agent_registry = match r.agent_registry {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            let task_ledger = match r.task_ledger {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            let escrow = match r.escrow {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            ContractRegistry { agent_registry, task_ledger, escrow }
        }
        None => ContractRegistry {
            agent_registry: None,
            task_ledger: None,
            escrow: None,
        },
    };

    let config = Config {
        admin,
        max_agents: msg.max_agents,
        registration_fee_ujuno: msg.registration_fee_ujuno,
        denom: msg.denom.unwrap_or_else(|| "ujunox".to_string()),
        registry,
    };
    CONFIG.save(deps.storage, &config)?;
    NEXT_AGENT_ID.save(deps.storage, &1u64)?;
    AGENT_STATS.save(
        deps.storage,
        &AgentStats {
            total_registered: 0,
            total_active: 0,
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.to_string())
        .add_attribute("max_agents", msg.max_agents.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterAgent {
            name,
            description,
            capabilities_hash,
            model,
        } => execute_register(deps, env, info, name, description, capabilities_hash, model),
        ExecuteMsg::UpdateAgent {
            agent_id,
            name,
            description,
            capabilities_hash,
            model,
        } => execute_update(deps, info, agent_id, name, description, capabilities_hash, model),
        ExecuteMsg::DeactivateAgent { agent_id } => execute_deactivate(deps, info, agent_id),
        ExecuteMsg::IncrementTasks { agent_id, success } => {
            execute_increment_tasks(deps, info, agent_id, success)
        }
        ExecuteMsg::SlashAgent { agent_id, reason } => {
            execute_slash_agent(deps, info, agent_id, reason)
        }
        ExecuteMsg::UpdateConfig {
            admin,
            max_agents,
            registration_fee_ujuno,
        } => execute_update_config(deps, info, admin, max_agents, registration_fee_ujuno),
        ExecuteMsg::UpdateRegistry {
            agent_registry,
            task_ledger,
            escrow,
        } => execute_update_registry(deps, info, agent_registry, task_ledger, escrow),
    }
}

fn execute_register(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    name: String,
    description: String,
    capabilities_hash: String,
    model: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let stats = AGENT_STATS.load(deps.storage)?;

    if stats.total_registered >= config.max_agents {
        return Err(ContractError::AgentLimitReached {
            max: config.max_agents,
        });
    }

    if !config.registration_fee_ujuno.is_zero() {
        let sent = info
            .funds
            .iter()
            .find(|c| c.denom == config.denom)
            .map(|c| c.amount)
            .unwrap_or(Uint128::zero());
        if sent < config.registration_fee_ujuno {
            return Err(ContractError::InsufficientFee {
                required: config.registration_fee_ujuno,
                sent,
            });
        }
    }

    let agent_id = NEXT_AGENT_ID.load(deps.storage)?;

    let profile = AgentProfile {
        owner: info.sender.clone(),
        name: name.clone(),
        description,
        capabilities_hash,
        model,
        registered_at: env.block.time.seconds(),
        is_active: true,
        total_tasks: 0,
        successful_tasks: 0,
        trust_score: 0,
    };

    AGENTS.save(deps.storage, agent_id, &profile)?;
    NEXT_AGENT_ID.save(deps.storage, &(agent_id + 1))?;

    AGENT_BY_OWNER.update(deps.storage, &info.sender, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(agent_id);
        Ok(ids)
    })?;

    AGENT_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_registered += 1;
        s.total_active += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "register_agent")
        .add_attribute("agent_id", agent_id.to_string())
        .add_attribute("owner", info.sender.to_string())
        .add_attribute("name", name))
}

fn execute_update(
    deps: DepsMut,
    info: MessageInfo,
    agent_id: u64,
    name: Option<String>,
    description: Option<String>,
    capabilities_hash: Option<String>,
    model: Option<String>,
) -> Result<Response, ContractError> {
    AGENTS.update(deps.storage, agent_id, |existing| match existing {
        Some(mut profile) => {
            if profile.owner != info.sender {
                return Err(ContractError::NotOwner {});
            }
            if let Some(n) = name {
                profile.name = n;
            }
            if let Some(d) = description {
                profile.description = d;
            }
            if let Some(c) = capabilities_hash {
                profile.capabilities_hash = c;
            }
            if let Some(m) = model {
                profile.model = m;
            }
            Ok(profile)
        }
        None => Err(ContractError::AgentNotFound { agent_id }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_agent")
        .add_attribute("agent_id", agent_id.to_string()))
}

fn execute_deactivate(
    deps: DepsMut,
    info: MessageInfo,
    agent_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    AGENTS.update(deps.storage, agent_id, |existing| match existing {
        Some(mut profile) => {
            if profile.owner != info.sender && info.sender != config.admin {
                return Err(ContractError::NotOwner {});
            }
            if !profile.is_active {
                return Err(ContractError::AlreadyDeactivated {});
            }
            profile.is_active = false;
            Ok(profile)
        }
        None => Err(ContractError::AgentNotFound { agent_id }),
    })?;

    AGENT_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_active = s.total_active.saturating_sub(1);
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "deactivate_agent")
        .add_attribute("agent_id", agent_id.to_string()))
}

fn execute_increment_tasks(
    deps: DepsMut,
    info: MessageInfo,
    agent_id: u64,
    success: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // Status coherence: only task-ledger or admin can increment
    let is_task_ledger = config.registry.task_ledger
        .as_ref()
        .map(|tl| *tl == info.sender)
        .unwrap_or(false);
    if !is_task_ledger && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    AGENTS.update(deps.storage, agent_id, |existing| match existing {
        Some(mut profile) => {
            profile.total_tasks += 1;
            if success {
                profile.successful_tasks += 1;
                profile.trust_score = profile.trust_score.saturating_add(1);
            }
            Ok(profile)
        }
        None => Err(ContractError::AgentNotFound { agent_id }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "increment_tasks")
        .add_attribute("agent_id", agent_id.to_string())
        .add_attribute("success", success.to_string()))
}

fn execute_slash_agent(
    deps: DepsMut,
    info: MessageInfo,
    agent_id: u64,
    reason: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    AGENTS.update(deps.storage, agent_id, |existing| match existing {
        Some(mut profile) => {
            profile.trust_score = profile.trust_score.saturating_sub(5);
            Ok(profile)
        }
        None => Err(ContractError::AgentNotFound { agent_id }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "slash_agent")
        .add_attribute("agent_id", agent_id.to_string())
        .add_attribute("reason", reason))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    max_agents: Option<u64>,
    registration_fee_ujuno: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(m) = max_agents {
        config.max_agents = m;
    }
    if let Some(f) = registration_fee_ujuno {
        config.registration_fee_ujuno = f;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Admin-only: rewire any subset of the cross-contract registry pointers.
/// This is the sole mechanism by which `IncrementTasks` can be authorised
/// (via `registry.task_ledger`) after instantiate.
fn execute_update_registry(
    deps: DepsMut,
    info: MessageInfo,
    agent_registry: Option<String>,
    task_ledger: Option<String>,
    escrow: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = agent_registry.as_ref() {
        config.registry.agent_registry = Some(deps.api.addr_validate(a)?);
    }
    if let Some(a) = task_ledger.as_ref() {
        config.registry.task_ledger = Some(deps.api.addr_validate(a)?);
    }
    if let Some(a) = escrow.as_ref() {
        config.registry.escrow = Some(deps.api.addr_validate(a)?);
    }

    CONFIG.save(deps.storage, &config)?;

    let mut response = Response::new().add_attribute("action", "update_registry");
    if let Some(a) = agent_registry {
        response = response.add_attribute("agent_registry", a);
    }
    if let Some(a) = task_ledger {
        response = response.add_attribute("task_ledger", a);
    }
    if let Some(a) = escrow {
        response = response.add_attribute("escrow", a);
    }
    Ok(response)
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetAgent { agent_id } => to_json_binary(&AGENTS.load(deps.storage, agent_id)?),
        QueryMsg::GetAgentsByOwner { owner } => {
            let addr = deps.api.addr_validate(&owner)?;
            let ids = AGENT_BY_OWNER
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            let agents: Vec<AgentProfile> = ids
                .iter()
                .filter_map(|id| AGENTS.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&agents)
        }
        QueryMsg::GetStats {} => to_json_binary(&AGENT_STATS.load(deps.storage)?),
        QueryMsg::ListAgents { start_after, limit } => {
            let limit = limit.unwrap_or(10).min(50) as usize;
            let start = start_after.map(cw_storage_plus::Bound::exclusive);
            let agents: Vec<AgentProfile> = AGENTS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, p)| p))
                .collect();
            to_json_binary(&agents)
        }
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
