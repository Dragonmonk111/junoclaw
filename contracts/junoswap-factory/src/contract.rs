use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo,
    Order, Response, StdResult, WasmMsg,
};
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
    let new_id = count + 1;

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

    // For now, store a placeholder — the pair address will be set via reply or manual registration
    // In production, use Reply to capture the instantiated address
    PAIR_COUNT.save(deps.storage, &new_id)?;

    let event = Event::new("wasm-create_pair")
        .add_attribute("pair_id", new_id.to_string())
        .add_attribute("token_a", key_a.clone())
        .add_attribute("token_b", key_b.clone())
        .add_attribute("fee_bps", fee.to_string())
        .add_attribute("creator", info.sender.to_string());

    Ok(Response::new()
        .add_message(instantiate_msg)
        .add_event(event)
        .add_attribute("action", "create_pair")
        .add_attribute("pair_id", new_id.to_string()))
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
            let start = start_after.map(|s| s + 1).unwrap_or(1);
            let pairs: Vec<PairResponse> = ALL_PAIRS
                .range(deps.storage, None, None, Order::Ascending)
                .filter(|r| r.as_ref().map(|(k, _)| *k >= start).unwrap_or(false))
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
