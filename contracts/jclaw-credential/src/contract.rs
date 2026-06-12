use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult,
};
use cw2::set_contract_version;
use junoclaw_mayo_verify::ParameterSet;

use crate::error::ContractError;
use crate::msg::{
    AncestryResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, ListChildrenResponse,
    ListMembersResponse, MayoPkHashResponse, MemberResponse, MigrateMsg, QueryMsg,
    SunsetStatusResponse, TotalWeightResponse,
};
use crate::state::{
    Config, Member, MemberRole, SunsetState, CHILDREN, CONFIG, MEMBERS, SUNSET, TOTAL_WEIGHT,
    TOTAL_WEIGHT_ITEM,
};

const CONTRACT_NAME: &str = "crates.io:jclaw-credential";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;

// ═════════════════════════════════════════════════════════════════════════════
// Instantiate
// ═════════════════════════════════════════════════════════════════════════════

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = msg
        .admin
        .as_ref()
        .map(|a| deps.api.addr_validate(a))
        .unwrap_or(Ok(info.sender.clone()))?;

    let genesis_addr = msg
        .genesis
        .as_ref()
        .map(|g| deps.api.addr_validate(g))
        .unwrap_or(Ok(info.sender.clone()))?;

    let grace = msg.sunset_grace_seconds;
    CONFIG.save(
        deps.storage,
        &Config {
            admin: admin.clone(),
            sunset_grace_seconds: grace,
        },
    )?;

    // Genesis member holds 100% weight initially.
    let genesis = Member {
        addr: genesis_addr.clone(),
        weight: TOTAL_WEIGHT,
        role: MemberRole::Genesis,
        parent: None,
        depth: 0,
        start_height: env.block.height,
        mayo_pk_hash: None,
    };
    MEMBERS.save(deps.storage, &genesis_addr, &genesis)?;
    TOTAL_WEIGHT_ITEM.save(deps.storage, &TOTAL_WEIGHT)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin)
        .add_attribute("genesis", genesis_addr))
}

// ═════════════════════════════════════════════════════════════════════════════
// Execute
// ═════════════════════════════════════════════════════════════════════════════

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Bud {
            parent,
            child,
            child_weight,
            mayo_pk,
        } => execute_bud(deps, env, info, parent, child, child_weight, mayo_pk),
        ExecuteMsg::BreakChannel { addr } => execute_break_channel(deps, info, addr),
        ExecuteMsg::InitiateSunset {} => execute_initiate_sunset(deps, env, info),
        ExecuteMsg::ExecuteSunset {} => execute_execute_sunset(deps, env, info),
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
        ExecuteMsg::VerifyMayoAttestation {
            addr,
            message,
            signature,
            public_key,
        } => execute_verify_mayo_attestation(deps, addr, message, signature, public_key),
    }
}

fn check_admin(deps: &DepsMut, info: &MessageInfo) -> Result<(), ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn check_not_sunsetting(deps: &DepsMut) -> Result<(), ContractError> {
    if let Ok(s) = SUNSET.load(deps.storage) {
        if s.initiated {
            return Err(ContractError::SunsettingNoBuds {});
        }
    }
    Ok(())
}

fn execute_bud(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    parent: String,
    child: String,
    child_weight: u64,
    mayo_pk: Option<Vec<u8>>,
) -> Result<Response, ContractError> {
    check_admin(&deps, &info)?;
    check_not_sunsetting(&deps)?;

    let parent_addr = deps.api.addr_validate(&parent)?;
    let child_addr = deps.api.addr_validate(&child)?;

    if MEMBERS.has(deps.storage, &child_addr) {
        return Err(ContractError::DuplicateMember {
            addr: child.into(),
        });
    }

    let mut parent_mem = MEMBERS
        .load(deps.storage, &parent_addr)
        .map_err(|_| ContractError::ParentNotFound {
            addr: parent.clone(),
        })?;

    if parent_mem.weight < child_weight {
        return Err(ContractError::InvalidWeights {
            expected: TOTAL_WEIGHT,
            got: parent_mem.weight,
        });
    }

    // Deduct weight from parent, create child.
    parent_mem.weight -= child_weight;
    MEMBERS.save(deps.storage, &parent_addr, &parent_mem)?;

    // Validate MAYO PK length and compute hash if provided.
    let mayo_pk_hash = mayo_pk.map(|pk| {
        if pk.len() != junoclaw_mayo_verify::Mayo2::PK_BYTES {
            return Err(ContractError::MayoInvalidPkLength {
                expected: junoclaw_mayo_verify::Mayo2::PK_BYTES,
                actual: pk.len(),
            });
        }
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(&pk);
        Ok(hex::encode(hash))
    }).transpose()?;

    let child_mem = Member {
        addr: child_addr.clone(),
        weight: child_weight,
        role: MemberRole::Bud,
        parent: Some(parent_addr.clone()),
        depth: parent_mem.depth + 1,
        start_height: env.block.height,
        mayo_pk_hash,
    };
    MEMBERS.save(deps.storage, &child_addr, &child_mem)?;

    // Index child under parent.
    let mut kids = CHILDREN
        .may_load(deps.storage, &parent_addr)?
        .unwrap_or_default();
    kids.push(child_addr.clone());
    CHILDREN.save(deps.storage, &parent_addr, &kids)?;

    Ok(Response::new()
        .add_attribute("action", "bud")
        .add_attribute("parent", parent_addr)
        .add_attribute("child", child_addr)
        .add_attribute("child_weight", child_weight.to_string()))
}

fn execute_verify_mayo_attestation(
    deps: DepsMut,
    addr: String,
    message: Vec<u8>,
    signature: Vec<u8>,
    public_key: Vec<u8>,
) -> Result<Response, ContractError> {
    let target = deps.api.addr_validate(&addr)?;
    let member = MEMBERS
        .load(deps.storage, &target)
        .map_err(|_| ContractError::MemberNotFound { addr: addr.clone() })?;

    let stored_hash = member
        .mayo_pk_hash
        .ok_or_else(|| ContractError::MayoPkNotFound {
            addr: addr.clone(),
        })?;

    // Verify provided PK matches stored hash.
    use sha2::{Digest, Sha256};
    let computed_hash = hex::encode(Sha256::digest(&public_key));
    if computed_hash != stored_hash {
        return Err(ContractError::MayoPkHashMismatch { addr: addr.clone() });
    }

    // Run pure-Rust MAYO-2 verifier.
    let valid = junoclaw_mayo_verify::verify::<junoclaw_mayo_verify::Mayo2>(
        &message,
        &signature,
        &public_key,
    )
    .map_err(|_| ContractError::MayoVerifyFailed { addr: addr.clone() })?;

    if !valid {
        return Err(ContractError::MayoVerifyFailed { addr: addr.clone() });
    }

    Ok(Response::new()
        .add_attribute("action", "verify_mayo_attestation")
        .add_attribute("addr", addr)
        .add_attribute("valid", "true"))
}

fn execute_break_channel(
    deps: DepsMut,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    check_admin(&deps, &info)?;

    let target = deps.api.addr_validate(&addr)?;
    let member = MEMBERS
        .load(deps.storage, &target)
        .map_err(|_| ContractError::MemberNotFound { addr: addr.clone() })?;

    // Cannot break the root (Genesis).
    if member.parent.is_none() {
        return Err(ContractError::BreakChannelRoot {});
    }

    // Recursively collect all descendants.
    let to_remove = collect_subtree(deps.storage, &target)?;
    let mut removed_weight: u64 = 0;

    for m in &to_remove {
        removed_weight = removed_weight.saturating_add(m.weight);
        MEMBERS.remove(deps.storage, &m.addr);
        CHILDREN.remove(deps.storage, &m.addr);
    }

    // Return removed weight to Genesis (root).
    let root = find_root(deps.storage)?;
    let mut root_mem = MEMBERS.load(deps.storage, &root)?;
    root_mem.weight = root_mem.weight.saturating_add(removed_weight);
    MEMBERS.save(deps.storage, &root, &root_mem)?;

    // Remove target from parent's children index.
    if let Some(parent_addr) = member.parent {
        let mut kids = CHILDREN
            .may_load(deps.storage, &parent_addr)?
            .unwrap_or_default();
        kids.retain(|k| k != &target);
        if kids.is_empty() {
            CHILDREN.remove(deps.storage, &parent_addr);
        } else {
            CHILDREN.save(deps.storage, &parent_addr, &kids)?;
        }
    }

    Ok(Response::new()
        .add_attribute("action", "break_channel")
        .add_attribute("removed_addr", addr)
        .add_attribute("removed_count", to_remove.len().to_string())
        .add_attribute("removed_weight", removed_weight.to_string()))
}

/// Recursively collect a member and all descendants.
fn collect_subtree(
    storage: &dyn cosmwasm_std::Storage,
    root: &cosmwasm_std::Addr,
) -> StdResult<Vec<Member>> {
    let mut result = vec![];
    let mut stack = vec![root.clone()];
    while let Some(addr) = stack.pop() {
        if let Ok(m) = MEMBERS.load(storage, &addr) {
            result.push(m.clone());
            if let Ok(kids) = CHILDREN.load(storage, &addr) {
                for k in kids {
                    stack.push(k);
                }
            }
        }
    }
    Ok(result)
}

fn find_root(storage: &dyn cosmwasm_std::Storage) -> StdResult<cosmwasm_std::Addr> {
    let members: Vec<_> = MEMBERS
        .range(storage, None, None, Order::Ascending)
        .collect::<Result<_, _>>()?;
    for (_, m) in members {
        if m.parent.is_none() {
            return Ok(m.addr);
        }
    }
    // Should never happen.
    Err(cosmwasm_std::StdError::generic_err("no root member found"))
}

fn execute_initiate_sunset(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    check_admin(&deps, &info)?;

    if SUNSET.may_load(deps.storage)?.is_some() {
        let s = SUNSET.load(deps.storage)?;
        if s.initiated {
            return Err(ContractError::SunsetAlreadyInitiated {
                height: s.initiated_at,
            });
        }
    }

    // Every member must have zero children (passed their bud).
    let members: Vec<_> = MEMBERS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<Result<_, _>>()?;
    for (_, m) in &members {
        let kids = CHILDREN
            .may_load(deps.storage, &m.addr)?
            .unwrap_or_default();
        if !kids.is_empty() {
            return Err(ContractError::SunsetBlocked {
                addr: m.addr.to_string(),
                children: kids.len() as u32,
            });
        }
    }

    SUNSET.save(
        deps.storage,
        &SunsetState {
            initiated: true,
            initiated_at: env.block.height,
            executed: false,
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "initiate_sunset")
        .add_attribute("initiated_at", env.block.height.to_string()))
}

fn execute_execute_sunset(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut s = SUNSET
        .load(deps.storage)
        .map_err(|_| ContractError::SunsetNotInitiated {})?;

    if s.executed {
        return Err(ContractError::AlreadySunset {});
    }
    if !s.initiated {
        return Err(ContractError::SunsetNotInitiated {});
    }

    // Grace period measured in blocks (approximation; real chains use time).
    let elapsed = env.block.height.saturating_sub(s.initiated_at);
    if elapsed < cfg.sunset_grace_seconds {
        return Err(ContractError::SunsetGracePeriod {
            remaining: cfg.sunset_grace_seconds - elapsed,
        });
    }

    s.executed = true;
    SUNSET.save(deps.storage, &s)?;

    Ok(Response::new()
        .add_attribute("action", "execute_sunset")
        .add_attribute("block_height", env.block.height.to_string()))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    check_admin(&deps, &info)?;
    let new = deps.api.addr_validate(&new_admin)?;
    CONFIG.update(deps.storage, |mut cfg| -> StdResult<_> {
        cfg.admin = new.clone();
        Ok(cfg)
    })?;
    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", new_admin))
}

// ═════════════════════════════════════════════════════════════════════════════
// Query
// ═════════════════════════════════════════════════════════════════════════════

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Member { addr } => to_json_binary(&query_member(deps, addr)?),
        QueryMsg::ListMembers { start_after, limit } => {
            to_json_binary(&query_list_members(deps, start_after, limit)?)
        }
        QueryMsg::TotalWeight {} => to_json_binary(&query_total_weight(deps)?),
        QueryMsg::ListChildren { addr } => to_json_binary(&query_list_children(deps, addr)?),
        QueryMsg::Ancestry { addr } => to_json_binary(&query_ancestry(deps, addr)?),
        QueryMsg::SunsetStatus {} => to_json_binary(&query_sunset_status(deps)?),
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::MayoPkHash { addr } => to_json_binary(&query_mayo_pk_hash(deps, addr)?),
    }
}

fn query_member(deps: Deps, addr: String) -> StdResult<MemberResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let m = MEMBERS.load(deps.storage, &addr)?;
    Ok(member_to_response(m))
}

fn query_list_members(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<ListMembersResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(|s| deps.api.addr_validate(s).unwrap());

    let members: Vec<MemberResponse> = MEMBERS
        .range(deps.storage, None, None, Order::Ascending)
        .filter(|r| {
            if let Ok((ref addr, _)) = r {
                if let Some(ref sa) = start {
                    return addr > sa;
                }
            }
            true
        })
        .take(limit)
        .map(|item| item.map(|(_, m)| member_to_response(m)))
        .collect::<Result<_, _>>()?;

    Ok(ListMembersResponse { members })
}

fn query_total_weight(deps: Deps) -> StdResult<TotalWeightResponse> {
    let weight = TOTAL_WEIGHT_ITEM.load(deps.storage)?;
    Ok(TotalWeightResponse { weight })
}

fn query_list_children(deps: Deps, addr: String) -> StdResult<ListChildrenResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let kids = CHILDREN
        .may_load(deps.storage, &addr)?
        .unwrap_or_default();
    let children: Vec<MemberResponse> = kids
        .into_iter()
        .map(|k| MEMBERS.load(deps.storage, &k).map(member_to_response))
        .collect::<Result<_, _>>()?;
    Ok(ListChildrenResponse { children })
}

fn query_ancestry(deps: Deps, addr: String) -> StdResult<AncestryResponse> {
    let mut path = vec![];
    let mut current = deps.api.addr_validate(&addr)?;

    while let Ok(m) = MEMBERS.load(deps.storage, &current) {
        path.push(member_to_response(m.clone()));
        match &m.parent {
            Some(p) => current = p.clone(),
            None => break,
        }
    }

    path.reverse(); // root -> leaf
    Ok(AncestryResponse { path })
}

fn query_sunset_status(deps: Deps) -> StdResult<SunsetStatusResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let s = SUNSET.may_load(deps.storage)?.unwrap_or(SunsetState {
        initiated: false,
        initiated_at: 0,
        executed: false,
    });

    let can_execute = s.initiated && !s.executed;
    let remaining = if s.initiated && !s.executed {
        Some(cfg.sunset_grace_seconds)
    } else {
        None
    };

    Ok(SunsetStatusResponse {
        initiated: s.initiated,
        initiated_at: if s.initiated { Some(s.initiated_at) } else { None },
        executed: s.executed,
        can_execute,
        remaining_grace_seconds: remaining,
    })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        admin: cfg.admin.to_string(),
        sunset_grace_seconds: cfg.sunset_grace_seconds,
    })
}

fn member_to_response(m: Member) -> MemberResponse {
    MemberResponse {
        addr: m.addr.to_string(),
        weight: m.weight,
        role: m.role,
        parent: m.parent.map(|p| p.to_string()),
        depth: m.depth,
        start_height: m.start_height,
        mayo_pk_hash: m.mayo_pk_hash,
    }
}

fn query_mayo_pk_hash(deps: Deps, addr: String) -> StdResult<MayoPkHashResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let m = MEMBERS.load(deps.storage, &addr)?;
    Ok(MayoPkHashResponse {
        addr: addr.to_string(),
        mayo_pk_hash: m.mayo_pk_hash,
    })
}

// ═════════════════════════════════════════════════════════════════════════════
// Migrate
// ═════════════════════════════════════════════════════════════════════════════

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "migrate"))
}
