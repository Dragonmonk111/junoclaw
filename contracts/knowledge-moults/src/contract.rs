use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, StdError,
    StdResult, Response,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, MoultsResponse, QueryMsg,
    StatsResponse,
};
use crate::state::{Config, KnowledgeMoult, Stats, BY_AGENT, BY_OWNER, CONFIG, MOULTS, STATS};

const CONTRACT_NAME: &str = "crates.io:knowledge-moults";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ID_PREFIX: &str = "kmoult";
const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;

#[cfg_attr(not(feature = "library"), entry_point)]
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

    CONFIG.save(
        deps.storage,
        &Config {
            admin: admin.clone(),
            mother_moult_id: msg.mother_moult_id,
            max_summary_len: msg.max_summary_len,
            max_source_moults: msg.max_source_moults,
        },
    )?;
    STATS.save(deps.storage, &Stats { total_minted: 0 })?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin)
        .add_attribute("instantiator", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint {
            agent,
            motive,
            knowledge_summary,
            source_moults,
            owner,
        } => execute_mint(deps, env, info, agent, motive, knowledge_summary, source_moults, owner),
        ExecuteMsg::Transfer { id, recipient } => execute_transfer(deps, info, id, recipient),
        ExecuteMsg::UpdateMotherMoult { mother_moult_id } => {
            execute_update_mother_moult(deps, info, mother_moult_id)
        }
        ExecuteMsg::UpdateConfig {
            admin,
            max_summary_len,
            max_source_moults,
        } => execute_update_config(deps, info, admin, max_summary_len, max_source_moults),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    agent: String,
    motive: String,
    knowledge_summary: String,
    source_moults: Vec<String>,
    owner: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    if agent.trim().is_empty() {
        return Err(ContractError::EmptyAgent {});
    }
    if knowledge_summary.len() as u32 > cfg.max_summary_len {
        return Err(ContractError::SummaryTooLong {
            len: knowledge_summary.len() as u32,
            max: cfg.max_summary_len,
        });
    }
    if source_moults.len() as u32 > cfg.max_source_moults {
        return Err(ContractError::TooManySourceMoults {
            count: source_moults.len() as u32,
            max: cfg.max_source_moults,
        });
    }

    let owner_addr = match owner {
        Some(o) => deps.api.addr_validate(&o)?,
        None => info.sender.clone(),
    };

    // Deterministic id = "kmoult:" + hex(sha256(minter || agent || motive ||
    // knowledge_summary || source_moults || minted_at_nanos)). agent/motive
    // alone are not unique enough (an agent can complete the same-named
    // motive twice); summary + source_moults give each artifact its own
    // "commitment" the same way Moultbook's content hash does.
    let mut hasher = Sha256::new();
    hasher.update(info.sender.as_bytes());
    hasher.update(agent.as_bytes());
    hasher.update(motive.as_bytes());
    hasher.update(knowledge_summary.as_bytes());
    hasher.update(source_moults.join(",").as_bytes());
    hasher.update(env.block.time.nanos().to_be_bytes());
    let id = format!("{}:{}", ID_PREFIX, hex_encode(&hasher.finalize()));

    if MOULTS.has(deps.storage, &id) {
        return Err(ContractError::DuplicateEntry { id });
    }

    let moult = KnowledgeMoult {
        id: id.clone(),
        owner: owner_addr.clone(),
        minter: info.sender.clone(),
        agent: agent.clone(),
        motive,
        knowledge_summary,
        source_moults,
        mother_moult_id: cfg.mother_moult_id,
        minted_at: env.block.time,
    };

    MOULTS.save(deps.storage, &id, &moult)?;
    BY_OWNER.save(deps.storage, (&owner_addr, id.as_str()), &())?;
    BY_AGENT.save(deps.storage, (agent.as_str(), id.as_str()), &())?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_minted += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_attribute("id", id)
        .add_attribute("agent", agent)
        .add_attribute("owner", owner_addr)
        .add_attribute("minter", info.sender))
}

fn execute_transfer(
    deps: DepsMut,
    info: MessageInfo,
    id: String,
    recipient: String,
) -> Result<Response, ContractError> {
    let mut moult = MOULTS
        .may_load(deps.storage, &id)?
        .ok_or_else(|| ContractError::NotFound { id: id.clone() })?;

    if info.sender != moult.owner {
        return Err(ContractError::Unauthorized {});
    }

    let recipient_addr = deps.api.addr_validate(&recipient)?;
    let old_owner = moult.owner.clone();

    BY_OWNER.remove(deps.storage, (&old_owner, id.as_str()));
    moult.owner = recipient_addr.clone();
    MOULTS.save(deps.storage, &id, &moult)?;
    BY_OWNER.save(deps.storage, (&recipient_addr, id.as_str()), &())?;

    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("id", id)
        .add_attribute("from", old_owner)
        .add_attribute("to", recipient_addr))
}

fn execute_update_mother_moult(
    deps: DepsMut,
    info: MessageInfo,
    mother_moult_id: String,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    cfg.mother_moult_id = mother_moult_id.clone();
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "update_mother_moult")
        .add_attribute("mother_moult_id", mother_moult_id))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    max_summary_len: Option<u32>,
    max_source_moults: Option<u32>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(a) = admin {
        cfg.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(m) = max_summary_len {
        cfg.max_summary_len = m;
    }
    if let Some(m) = max_source_moults {
        cfg.max_source_moults = m;
    }
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        QueryMsg::GetMoult { id } => to_json_binary(&query_moult(deps, id)?),
        QueryMsg::ListByOwner {
            owner,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_owner(deps, owner, start_after, limit)?),
        QueryMsg::ListByAgent {
            agent,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_agent(deps, agent, start_after, limit)?),
        QueryMsg::GetStats {} => to_json_binary(&query_stats(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        admin: cfg.admin.to_string(),
        mother_moult_id: cfg.mother_moult_id,
        max_summary_len: cfg.max_summary_len,
        max_source_moults: cfg.max_source_moults,
    })
}

fn query_moult(deps: Deps, id: String) -> StdResult<KnowledgeMoult> {
    MOULTS
        .may_load(deps.storage, &id)?
        .ok_or_else(|| StdError::not_found(format!("knowledge moult: {}", id)))
}

fn query_list_by_owner(
    deps: Deps,
    owner: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MoultsResponse> {
    let owner: Addr = deps.api.addr_validate(&owner)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let moults = BY_OWNER
        .prefix(&owner)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            MOULTS.load(deps.storage, &id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(MoultsResponse { moults })
}

fn query_list_by_agent(
    deps: Deps,
    agent: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MoultsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let moults = BY_AGENT
        .prefix(agent.as_str())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            MOULTS.load(deps.storage, &id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(MoultsResponse { moults })
}

fn query_stats(deps: Deps) -> StdResult<StatsResponse> {
    let s = STATS.load(deps.storage)?;
    Ok(StatsResponse {
        total_minted: s.total_minted,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "migrate"))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}
