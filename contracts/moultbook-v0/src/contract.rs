use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, QueryRequest,
    Response, StdError, StdResult, WasmQuery,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    EntriesResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, WhoamiQuery,
    WhoamiTokensResponse,
};
use crate::state::{
    AttestationRef, Config, MoultEntry, Stats, Visibility, BY_AUTHOR, BY_REF, CONFIG, ENTRIES,
    STATS,
};

const CONTRACT_NAME: &str = "crates.io:moultbook-v0";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMITMENT_LEN: usize = 32;
const ID_PREFIX: &str = "moult";
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

    let admin = deps.api.addr_validate(&msg.admin)?;
    let whoami_contract = msg
        .whoami_contract
        .as_deref()
        .map(|s| deps.api.addr_validate(s))
        .transpose()?;

    CONFIG.save(
        deps.storage,
        &Config {
            admin: admin.clone(),
            whoami_contract,
            max_size_bytes: msg.max_size_bytes,
            max_refs: msg.max_refs,
            max_content_type_len: msg.max_content_type_len,
        },
    )?;

    STATS.save(
        deps.storage,
        &Stats {
            total_entries: 0,
            total_active: 0,
            total_redacted: 0,
        },
    )?;

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
        ExecuteMsg::Post {
            commitment,
            content_type,
            size_bytes,
            attestation_ref,
            visibility,
            refs,
        } => execute_post(
            deps,
            env,
            info,
            commitment,
            content_type,
            size_bytes,
            attestation_ref,
            visibility,
            refs,
        ),
        ExecuteMsg::Redact { id } => execute_redact(deps, env, info, id),
        ExecuteMsg::UpdateVisibility { id, visibility } => {
            execute_update_visibility(deps, info, id, visibility)
        }
        ExecuteMsg::UpdateConfig {
            admin,
            whoami_contract,
            max_size_bytes,
            max_refs,
        } => execute_update_config(deps, info, admin, whoami_contract, max_size_bytes, max_refs),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_post(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    commitment: Binary,
    content_type: String,
    size_bytes: u64,
    attestation_ref: Option<AttestationRef>,
    visibility: Visibility,
    refs: Vec<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // 1) Input validation.
    if commitment.len() != COMMITMENT_LEN {
        return Err(ContractError::InvalidCommitmentLength {
            got: commitment.len(),
        });
    }
    if size_bytes > cfg.max_size_bytes {
        return Err(ContractError::SizeTooLarge {
            size: size_bytes,
            max: cfg.max_size_bytes,
        });
    }
    if refs.len() as u32 > cfg.max_refs {
        return Err(ContractError::TooManyRefs {
            count: refs.len() as u32,
            max: cfg.max_refs,
        });
    }
    if content_type.len() as u32 > cfg.max_content_type_len {
        return Err(ContractError::ContentTypeTooLong {
            len: content_type.len() as u32,
            max: cfg.max_content_type_len,
        });
    }

    // 2) Optional identity gating via whoami.
    let author_alias = if let Some(whoami) = cfg.whoami_contract.as_ref() {
        let resp: WhoamiTokensResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: whoami.to_string(),
                msg: to_json_binary(&WhoamiQuery::Tokens {
                    owner: info.sender.to_string(),
                    start_after: None,
                    limit: Some(1),
                })?,
            }))?;
        if resp.tokens.is_empty() {
            return Err(ContractError::NoIdentity {});
        }
        Some(resp.tokens[0].clone())
    } else {
        None
    };

    // 3) Validate refs exist (cite-only-real-entries discipline).
    for r in &refs {
        if !ENTRIES.has(deps.storage, r) {
            return Err(ContractError::InvalidRef { id: r.clone() });
        }
    }

    // 4) Deterministic id = "moult:" + hex(sha256(commitment || sender || posted_at_nanos)).
    let mut hasher = Sha256::new();
    hasher.update(commitment.as_slice());
    hasher.update(info.sender.as_bytes());
    hasher.update(env.block.time.nanos().to_be_bytes());
    let id = format!("{}:{}", ID_PREFIX, hex_encode(&hasher.finalize()));

    if ENTRIES.has(deps.storage, &id) {
        return Err(ContractError::DuplicateEntry { id });
    }

    // 5) Materialise.
    let entry = MoultEntry {
        id: id.clone(),
        author: info.sender.clone(),
        author_alias,
        commitment,
        content_type,
        size_bytes,
        attestation_ref,
        visibility,
        refs: refs.clone(),
        posted_at: env.block.time,
        redacted_at: None,
    };
    ENTRIES.save(deps.storage, &id, &entry)?;
    BY_AUTHOR.save(deps.storage, (&info.sender, id.as_str()), &())?;
    for r in &refs {
        BY_REF.save(deps.storage, (r.as_str(), id.as_str()), &())?;
    }

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_entries += 1;
        s.total_active += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "post")
        .add_attribute("id", id)
        .add_attribute("author", info.sender))
}

fn execute_redact(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut entry = ENTRIES
        .may_load(deps.storage, &id)?
        .ok_or_else(|| ContractError::EntryNotFound { id: id.clone() })?;

    if info.sender != entry.author && info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    if entry.redacted_at.is_some() {
        return Err(ContractError::AlreadyRedacted { id });
    }

    entry.commitment = Binary::default();
    entry.redacted_at = Some(env.block.time);
    ENTRIES.save(deps.storage, &id, &entry)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_active = s.total_active.saturating_sub(1);
        s.total_redacted += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "redact")
        .add_attribute("id", id)
        .add_attribute("by", info.sender))
}

fn execute_update_visibility(
    deps: DepsMut,
    info: MessageInfo,
    id: String,
    visibility: Visibility,
) -> Result<Response, ContractError> {
    let mut entry = ENTRIES
        .may_load(deps.storage, &id)?
        .ok_or_else(|| ContractError::EntryNotFound { id: id.clone() })?;

    if info.sender != entry.author {
        return Err(ContractError::Unauthorized {});
    }
    if matches!(visibility, Visibility::Public)
        && !matches!(entry.visibility, Visibility::Public)
    {
        return Err(ContractError::CannotWidenVisibility {});
    }

    entry.visibility = visibility;
    ENTRIES.save(deps.storage, &id, &entry)?;

    Ok(Response::new()
        .add_attribute("action", "update_visibility")
        .add_attribute("id", id))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    whoami_contract: Option<String>,
    max_size_bytes: Option<u64>,
    max_refs: Option<u32>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(a) = admin {
        cfg.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(w) = whoami_contract {
        cfg.whoami_contract = if w.is_empty() {
            None
        } else {
            Some(deps.api.addr_validate(&w)?)
        };
    }
    if let Some(m) = max_size_bytes {
        cfg.max_size_bytes = m;
    }
    if let Some(m) = max_refs {
        cfg.max_refs = m;
    }
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetEntry { id } => to_json_binary(&query_entry(deps, id)?),
        QueryMsg::ListByAuthor {
            author,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_author(deps, author, start_after, limit)?),
        QueryMsg::ListByRef {
            ref_id,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_ref(deps, ref_id, start_after, limit)?),
        QueryMsg::GetStats {} => to_json_binary(&STATS.load(deps.storage)?),
    }
}

fn query_entry(deps: Deps, id: String) -> StdResult<MoultEntry> {
    ENTRIES
        .may_load(deps.storage, &id)?
        .ok_or_else(|| StdError::not_found(format!("entry: {}", id)))
}

fn query_list_by_author(
    deps: Deps,
    author: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<EntriesResponse> {
    let author = deps.api.addr_validate(&author)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let entries = BY_AUTHOR
        .prefix(&author)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            ENTRIES.load(deps.storage, &id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(EntriesResponse { entries })
}

fn query_list_by_ref(
    deps: Deps,
    ref_id: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<EntriesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let entries = BY_REF
        .prefix(ref_id.as_str())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            ENTRIES.load(deps.storage, &id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(EntriesResponse { entries })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
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
