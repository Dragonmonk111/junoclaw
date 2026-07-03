use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order,
    QueryRequest, Reply, Response, StdError, StdResult, SubMsg, WasmMsg, WasmQuery,
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
    AttestationRef, Config, Disclosure, MoultEntry, MoultKeyState, PendingDisclosure,
    PendingVerification, Stats, Visibility, BY_AUTHOR, BY_MOULT_KEY, BY_REF, BY_TOPIC, CONFIG,
    DISCLOSURES, ENTRIES, MOULT_KEY_STATE, PENDING_DISCLOSURE, PENDING_VERIFICATION, STATS,
};

const CONTRACT_NAME: &str = "crates.io:moultbook-v0";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMITMENT_LEN: usize = 32;
const ID_PREFIX: &str = "moult";
const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;
const REPLY_ID_ZK_VERIFY: u64 = 1;
const REPLY_ID_DISCLOSURE_VERIFY: u64 = 2;

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

    let zk_verifier = msg
        .zk_verifier
        .as_deref()
        .map(|s| deps.api.addr_validate(s))
        .transpose()?;
    let agent_registry = msg
        .agent_registry
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
            max_group_size: msg.max_group_size,
            zk_verifier,
            agent_registry,
            membership_vk_hash: msg.membership_vk_hash,
            entries_per_key_per_epoch: msg.entries_per_key_per_epoch.unwrap_or(10),
            epoch_blocks: msg.epoch_blocks.unwrap_or(14400),
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
            max_group_size,
            membership_vk_hash,
        } => execute_update_config(
            deps,
            info,
            admin,
            whoami_contract,
            max_size_bytes,
            max_refs,
            max_group_size,
            membership_vk_hash,
        ),
        ExecuteMsg::PublishAnon {
            topic_hash,
            content_cid,
            proof_base64,
            public_inputs_base64,
        } => execute_publish_anon(
            deps,
            env,
            info,
            topic_hash,
            content_cid,
            proof_base64,
            public_inputs_base64,
        ),
        ExecuteMsg::VoluntaryDisclose {
            entry_id,
            primary_key,
            derivation_proof_base64,
            derivation_public_inputs_base64,
        } => execute_voluntary_disclose(
            deps,
            env,
            info,
            entry_id,
            primary_key,
            derivation_proof_base64,
            derivation_public_inputs_base64,
        ),
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
    if let Visibility::Group(ref addrs) = visibility {
        if addrs.len() as u32 > cfg.max_group_size {
            return Err(ContractError::GroupTooLarge {
                count: addrs.len() as u32,
                max: cfg.max_group_size,
            });
        }
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
        topic_hash: None,
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
    max_group_size: Option<u32>,
    membership_vk_hash: Option<String>,
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
    if let Some(m) = max_group_size {
        cfg.max_group_size = m;
    }
    if let Some(m) = membership_vk_hash {
        cfg.membership_vk_hash = if m.is_empty() { None } else { Some(m) };
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
        QueryMsg::ListByMoultKey {
            moult_key,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_moult_key(deps, moult_key, start_after, limit)?),
        QueryMsg::MoultKeyStats { moult_key } => {
            to_json_binary(&query_moult_key_stats(deps, moult_key)?)
        }
        QueryMsg::GetDisclosure { entry_id } => {
            to_json_binary(&DISCLOSURES.may_load(deps.storage, &entry_id)?)
        }
        QueryMsg::ListByTopic {
            topic_hash,
            start_after,
            limit,
        } => to_json_binary(&query_list_by_topic(deps, topic_hash, start_after, limit)?),
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

fn query_list_by_topic(
    deps: Deps,
    topic_hash: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<EntriesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let entries = BY_TOPIC
        .prefix(topic_hash.as_str())
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
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        REPLY_ID_ZK_VERIFY => handle_zk_verify_reply(deps, env, msg),
        REPLY_ID_DISCLOSURE_VERIFY => handle_disclosure_verify_reply(deps, env, msg),
        _ => Err(ContractError::Std(StdError::generic_err(format!(
            "unknown reply id: {}",
            msg.id
        )))),
    }
}

fn handle_zk_verify_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    // If the sub-message failed, the ZK proof was invalid.
    if msg.result.is_err() {
        PENDING_VERIFICATION.remove(deps.storage);
        return Err(ContractError::MembershipProofInvalid {});
    }

    let pending = PENDING_VERIFICATION.load(deps.storage)?;
    PENDING_VERIFICATION.remove(deps.storage);

    // Deterministic id for anonymous entries
    let mut hasher = Sha256::new();
    hasher.update(pending.moult_key.as_bytes());
    hasher.update(pending.topic_hash.as_bytes());
    hasher.update(env.block.time.nanos().to_be_bytes());
    let id = format!("{}:{}", ID_PREFIX, hex_encode(&hasher.finalize()));

    let entry = MoultEntry {
        id: id.clone(),
        author: pending.moult_key.clone(),
        author_alias: None,
        commitment: pending.proof.clone(),
        content_type: "application/ipfs-cid".to_string(),
        size_bytes: 0,
        attestation_ref: Some(AttestationRef::ZkProof {
            verifier: CONFIG.load(deps.storage)?.zk_verifier.unwrap(),
            proof_id: id.clone(),
        }),
        visibility: Visibility::Public,
        refs: vec![],
        posted_at: env.block.time,
        redacted_at: None,
        topic_hash: Some(pending.topic_hash.clone()),
    };

    ENTRIES.save(deps.storage, &id, &entry)?;
    BY_AUTHOR.save(deps.storage, (&pending.moult_key, id.as_str()), &())?;
    BY_MOULT_KEY.save(deps.storage, (&pending.moult_key, id.as_str()), &())?;
    BY_TOPIC.save(deps.storage, (&pending.topic_hash, id.as_str()), &())?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_entries += 1;
        s.total_active += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "publish_anon")
        .add_attribute("id", id)
        .add_attribute("moult_key", pending.moult_key)
        .add_attribute("topic_hash", pending.topic_hash)
        .add_attribute("content_cid", pending.content_cid))
}

#[allow(clippy::too_many_arguments)]
fn execute_publish_anon(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    topic_hash: String,
    content_cid: String,
    proof_base64: String,
    public_inputs_base64: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let zk_verifier = cfg
        .zk_verifier
        .ok_or(ContractError::ZkVerifierNotConfigured {})?;
    let _membership_vk_hash = cfg
        .membership_vk_hash
        .ok_or(ContractError::MembershipVkNotConfigured {})?;

    // Epoch-based rate limiting
    let current_epoch = env.block.height / cfg.epoch_blocks;
    let mut key_state = MOULT_KEY_STATE
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(MoultKeyState {
            entries_this_epoch: 0,
            last_epoch: current_epoch,
        });

    if key_state.last_epoch < current_epoch {
        key_state.entries_this_epoch = 0;
        key_state.last_epoch = current_epoch;
    }

    if key_state.entries_this_epoch >= cfg.entries_per_key_per_epoch {
        return Err(ContractError::EpochRateLimited {
            count: key_state.entries_this_epoch,
            max: cfg.entries_per_key_per_epoch,
        });
    }

    key_state.entries_this_epoch += 1;
    MOULT_KEY_STATE.save(deps.storage, &info.sender, &key_state)?;

    // Store pending verification state for the reply handler
    PENDING_VERIFICATION.save(
        deps.storage,
        &PendingVerification {
            moult_key: info.sender.clone(),
            topic_hash: topic_hash.clone(),
            content_cid: content_cid.clone(),
            proof: Binary::from(proof_base64.as_bytes()),
            public_inputs: Binary::from(public_inputs_base64.as_bytes()),
        },
    )?;

    // Fire sub-message to zk-verifier::VerifyProof
    let verify_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: zk_verifier.to_string(),
        msg: to_json_binary(&serde_json::json!({
            "verify_proof": {
                "proof_base64": proof_base64,
                "public_inputs_base64": public_inputs_base64
            }
        }))?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(verify_msg, REPLY_ID_ZK_VERIFY))
        .add_attribute("action", "publish_anon_pending")
        .add_attribute("moult_key", info.sender)
        .add_attribute("topic_hash", topic_hash))
}

#[allow(clippy::too_many_arguments)]
fn execute_voluntary_disclose(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    entry_id: String,
    primary_key: String,
    derivation_proof_base64: String,
    derivation_public_inputs_base64: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let entry = ENTRIES
        .may_load(deps.storage, &entry_id)?
        .ok_or_else(|| ContractError::EntryNotFound {
            id: entry_id.clone(),
        })?;

    // Only the moult-key author can disclose
    if info.sender != entry.author {
        return Err(ContractError::NotEntryAuthor {
            id: entry_id.clone(),
        });
    }

    // No double disclosure
    if DISCLOSURES.has(deps.storage, &entry_id) {
        return Err(ContractError::AlreadyDisclosed { id: entry_id });
    }

    let primary = deps.api.addr_validate(&primary_key)?;

    // Verification of the derivation proof is mandatory. The membership
    // circuit (in disclosure mode) proves that `moult_key` was derived from
    // `primary_key` without leaking the derivation secret. We delegate the
    // Groth16 check to the configured zk-verifier as a reply_on_success
    // sub-message: an invalid proof aborts the verifier call, which rolls the
    // whole tx back, so the disclosure is never persisted.
    let zk_verifier = cfg
        .zk_verifier
        .ok_or(ContractError::ZkVerifierNotConfigured {})?;

    // Stash the pending disclosure so the reply handler can finalise it once
    // the proof verifies. Storing the validated primary key avoids re-parsing.
    PENDING_DISCLOSURE.save(
        deps.storage,
        &PendingDisclosure {
            entry_id: entry_id.clone(),
            moult_key: info.sender.clone(),
            primary_key: primary.clone(),
        },
    )?;

    let verify_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: zk_verifier.to_string(),
        msg: to_json_binary(&serde_json::json!({
            "verify_proof": {
                "proof_base64": derivation_proof_base64,
                "public_inputs_base64": derivation_public_inputs_base64
            }
        }))?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            verify_msg,
            REPLY_ID_DISCLOSURE_VERIFY,
        ))
        .add_attribute("action", "voluntary_disclose_pending")
        .add_attribute("entry_id", entry_id)
        .add_attribute("primary_key", primary)
        .add_attribute("moult_key", info.sender))
}

fn handle_disclosure_verify_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    // A failed sub-message means the derivation proof was invalid. Clear the
    // pending state and surface a typed error so the tx rolls back cleanly.
    if msg.result.is_err() {
        PENDING_DISCLOSURE.remove(deps.storage);
        return Err(ContractError::DerivationProofInvalid {});
    }

    let pending = PENDING_DISCLOSURE.load(deps.storage)?;
    PENDING_DISCLOSURE.remove(deps.storage);

    // Guard against a TOCTOU race: a disclosure for the same entry may have
    // landed between the original execute and this reply.
    if DISCLOSURES.has(deps.storage, &pending.entry_id) {
        return Err(ContractError::AlreadyDisclosed {
            id: pending.entry_id,
        });
    }

    DISCLOSURES.save(
        deps.storage,
        &pending.entry_id,
        &Disclosure {
            entry_id: pending.entry_id.clone(),
            primary_key: pending.primary_key.clone(),
            disclosed_at: env.block.time,
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "voluntary_disclose")
        .add_attribute("entry_id", pending.entry_id)
        .add_attribute("primary_key", pending.primary_key)
        .add_attribute("moult_key", pending.moult_key))
}

fn query_list_by_moult_key(
    deps: Deps,
    moult_key: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<EntriesResponse> {
    let moult_key = deps.api.addr_validate(&moult_key)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let entries = BY_MOULT_KEY
        .prefix(&moult_key)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            ENTRIES.load(deps.storage, &id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(EntriesResponse { entries })
}

fn query_moult_key_stats(deps: Deps, moult_key: String) -> StdResult<MoultKeyState> {
    let moult_key = deps.api.addr_validate(&moult_key)?;
    MOULT_KEY_STATE
        .may_load(deps.storage, &moult_key)?
        .ok_or_else(|| StdError::not_found("moult_key_state"))
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
