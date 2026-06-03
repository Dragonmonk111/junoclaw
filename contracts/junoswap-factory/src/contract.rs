use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo,
    Order, Reply, Response, StdResult, SubMsg, SubMsgResult, WasmMsg,
};
use cw_storage_plus::Bound;
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;
use junoclaw_common::AssetInfo;

const CONTRACT_NAME: &str = "crates.io:junoswap-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn sort_assets(a: &AssetInfo, b: &AssetInfo) -> (String, String) {
    let ka = a.denom_key();
    let kb = b.denom_key();
    if ka <= kb { (ka, kb) } else { (kb, ka) }
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.default_fee_bps > 10000 {
        return Err(ContractError::InvalidFee {});
    }

    let junoclaw = msg
        .junoclaw_contract
        .map(|a| deps.api.addr_validate(&a))
        .transpose()?;

    let config = Config {
        owner: info.sender.clone(),
        pair_code_id: msg.pair_code_id,
        default_fee_bps: msg.default_fee_bps,
        junoclaw_contract: junoclaw,
    };
    CONFIG.save(deps.storage, &config)?;
    PAIR_COUNT.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreatePair { token_a, token_b, fee_bps } => {
            execute_create_pair(deps, env, info, token_a, token_b, fee_bps)
        }
        ExecuteMsg::UpdateConfig { pair_code_id, default_fee_bps, junoclaw_contract } => {
            execute_update_config(deps, info, pair_code_id, default_fee_bps, junoclaw_contract)
        }
    }
}

fn execute_create_pair(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_a: AssetInfo,
    token_b: AssetInfo,
    fee_bps: Option<u16>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if token_a.denom_key() == token_b.denom_key() {
        return Err(ContractError::IdenticalAssets {});
    }

    let (key_a, key_b) = sort_assets(&token_a, &token_b);
    if PAIRS.has(deps.storage, (&key_a, &key_b)) {
        return Err(ContractError::PairExists {});
    }

    let fee = fee_bps.unwrap_or(config.default_fee_bps);
    if fee > 10000 {
        return Err(ContractError::InvalidFee {});
    }

    let count = PAIR_COUNT.load(deps.storage)?;
    let new_id = count.checked_add(1).ok_or(ContractError::Overflow {})?;

    // Instantiate the pair contract via sub-message
    let pair_instantiate_msg = crate::msg::PairInstantiateMsg {
        token_a: token_a.clone(),
        token_b: token_b.clone(),
        fee_bps: fee,
        factory: env.contract.address.to_string(),
        junoclaw_contract: config.junoclaw_contract.map(|a| a.to_string()),
    };

    let instantiate_msg = WasmMsg::Instantiate {
        admin: Some(env.contract.address.to_string()),
        code_id: config.pair_code_id,
        msg: to_json_binary(&pair_instantiate_msg)?,
        funds: vec![],
        label: format!("junoswap-pair-{}", new_id),
    };

    // Track the instantiate as a reply-bearing sub-message so we can capture the
    // spawned pair address and register it in PAIRS / ALL_PAIRS. The pending
    // metadata is keyed by the reply id (= new_id) and finalised in `reply`.
    // PAIR_COUNT is only advanced here; PAIRS/ALL_PAIRS are written on success.
    PAIR_COUNT.save(deps.storage, &new_id)?;
    PENDING_PAIRS.save(
        deps.storage,
        new_id,
        &PendingPair {
            id: new_id,
            token_a: token_a.clone(),
            token_b: token_b.clone(),
        },
    )?;

    let event = Event::new("wasm-create_pair")
        .add_attribute("pair_id", new_id.to_string())
        .add_attribute("token_a", key_a.clone())
        .add_attribute("token_b", key_b.clone())
        .add_attribute("fee_bps", fee.to_string())
        .add_attribute("creator", info.sender.to_string());

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(instantiate_msg, new_id))
        .add_event(event)
        .add_attribute("action", "create_pair")
        .add_attribute("pair_id", new_id.to_string()))
}

/// Extract the instantiated contract address from a successful instantiate
/// reply. wasmd emits an `instantiate` event carrying `_contract_address`.
fn parse_instantiate_addr(result: SubMsgResult) -> Result<String, ContractError> {
    let res = result.into_result().map_err(|_| ContractError::ReplyParse {})?;
    for ev in &res.events {
        if ev.ty == "instantiate" {
            for attr in &ev.attributes {
                if attr.key == "_contract_address" {
                    return Ok(attr.value.clone());
                }
            }
        }
    }
    Err(ContractError::ReplyParse {})
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    let reply_id = msg.id;
    let pending = PENDING_PAIRS
        .may_load(deps.storage, reply_id)?
        .ok_or(ContractError::UnknownReplyId { id: reply_id })?;

    let addr_str = parse_instantiate_addr(msg.result)?;
    let pair_addr = deps.api.addr_validate(&addr_str)?;

    let key_a = pending.token_a.denom_key();
    let key_b = pending.token_b.denom_key();
    let (key_a, key_b) = if key_a <= key_b {
        (key_a, key_b)
    } else {
        (key_b, key_a)
    };

    PAIRS.save(deps.storage, (&key_a, &key_b), &pair_addr)?;
    ALL_PAIRS.save(
        deps.storage,
        pending.id,
        &PairRecord {
            id: pending.id,
            pair_addr: pair_addr.clone(),
            token_a: pending.token_a,
            token_b: pending.token_b,
            created_at: env.block.height,
        },
    )?;
    PENDING_PAIRS.remove(deps.storage, reply_id);

    Ok(Response::new()
        .add_attribute("action", "register_pair")
        .add_attribute("pair_id", pending.id.to_string())
        .add_attribute("pair_addr", pair_addr))
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_code_id: Option<u64>,
    default_fee_bps: Option<u16>,
    junoclaw_contract: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(id) = pair_code_id {
        config.pair_code_id = id;
    }
    if let Some(fee) = default_fee_bps {
        if fee > 10000 {
            return Err(ContractError::InvalidFee {});
        }
        config.default_fee_bps = fee;
    }
    if let Some(addr) = junoclaw_contract {
        config.junoclaw_contract = Some(deps.api.addr_validate(&addr)?);
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            let count = PAIR_COUNT.load(deps.storage)?;
            to_json_binary(&ConfigResponse {
                owner: config.owner,
                pair_code_id: config.pair_code_id,
                default_fee_bps: config.default_fee_bps,
                junoclaw_contract: config.junoclaw_contract,
                pair_count: count,
            })
        }
        QueryMsg::Pair { token_a, token_b } => {
            let (ka, kb) = sort_assets(&token_a, &token_b);
            let addr = PAIRS.load(deps.storage, (&ka, &kb))?;
            to_json_binary(&PairResponse {
                pair_addr: addr,
                token_a,
                token_b,
            })
        }
        QueryMsg::AllPairs { start_after, limit } => {
            let limit = limit.unwrap_or(30).min(100) as usize;
            // Seek directly to the start key at the storage layer — O(limit),
            // not O(n) over every pair (audit F3).
            let start_bound = start_after.map(Bound::exclusive);
            let pairs: Vec<PairResponse> = ALL_PAIRS
                .range(deps.storage, start_bound, None, Order::Ascending)
                .take(limit)
                .map(|r| {
                    let (_, rec) = r?;
                    Ok(PairResponse {
                        pair_addr: rec.pair_addr,
                        token_a: rec.token_a,
                        token_b: rec.token_b,
                    })
                })
                .collect::<StdResult<_>>()?;
            to_json_binary(&PairsResponse { pairs })
        }
        QueryMsg::PairCount {} => {
            let count = PAIR_COUNT.load(deps.storage)?;
            to_json_binary(&count)
        }
    }
}
