use cosmwasm_std::{
    coins, entry_point, to_json_binary, Binary, Deps, DepsMut, Env, Event,
    MessageInfo, Response, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;

const CONTRACT_NAME: &str = "crates.io:ibc-task-host";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let allowed_pairs = msg
        .allowed_pairs
        .iter()
        .map(|a| deps.api.addr_validate(a))
        .collect::<StdResult<Vec<_>>>()?;

    let config = HostConfig {
        admin: deps.api.addr_validate(&msg.admin)?,
        task_ledger: msg
            .task_ledger
            .map(|a| deps.api.addr_validate(&a))
            .transpose()?,
        escrow: msg
            .escrow
            .map(|a| deps.api.addr_validate(&a))
            .transpose()?,
        zk_verifier: msg
            .zk_verifier
            .map(|a| deps.api.addr_validate(&a))
            .transpose()?,
        allowed_pairs,
    };

    HOST_CONFIG.save(deps.storage, &config)?;
    HOST_STATS.save(deps.storage, &HostStats::default())?;

    Ok(Response::new().add_attribute("action", "instantiate_ibc_task_host"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::JunoClawV1(op) => execute_junoclaw_v1(deps, env, info, op),
        ExecuteMsg::UpdateConfig {
            task_ledger,
            escrow,
            zk_verifier,
            allowed_pairs,
        } => execute_update_config(deps, info, task_ledger, escrow, zk_verifier, allowed_pairs),
    }
}

/// Dispatch a JunoClawV1 operation received via ICS-20+PFM memo.
fn execute_junoclaw_v1(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    op: JunoClawV1Op,
) -> Result<Response, ContractError> {
    match op {
        JunoClawV1Op::AcceptTask {
            task_id,
            agent_addr,
            agent_origin_chain,
            agent_origin_addr,
        } => execute_accept_task(
            deps, env, info, task_id, agent_addr, agent_origin_chain, agent_origin_addr,
        ),
        JunoClawV1Op::SubmitProof {
            task_id,
            proof_b64,
            public_inputs_b64,
            agent_origin_chain,
            agent_origin_addr,
        } => execute_submit_proof(
            deps, env, info, task_id, proof_b64, public_inputs_b64,
            agent_origin_chain, agent_origin_addr,
        ),
        JunoClawV1Op::ReclaimExpired {
            task_id,
            dao_origin_chain,
            dao_origin_addr,
        } => execute_reclaim_expired(deps, env, info, task_id, dao_origin_chain, dao_origin_addr),
        JunoClawV1Op::Swap {
            pair_contract,
            offer_denom,
            min_return,
            agent_origin_chain,
            agent_origin_addr,
            max_price_impact_bps,
        } => execute_swap(
            deps, env, info, pair_contract, offer_denom, min_return,
            agent_origin_chain, agent_origin_addr, max_price_impact_bps,
        ),
    }
}

/// AcceptTask — forward to task-ledger
fn execute_accept_task(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    task_id: u64,
    agent_addr: String,
    agent_origin_chain: String,
    #[allow(unused)] agent_origin_addr: String,
) -> Result<Response, ContractError> {
    let config = HOST_CONFIG.load(deps.storage)?;
    let task_ledger = config
        .task_ledger
        .ok_or(ContractError::TaskLedgerNotConfigured {})?;

    let mut stats = HOST_STATS.load(deps.storage)?;
    stats.total_accept_task += 1;
    HOST_STATS.save(deps.storage, &stats)?;

    // Forward to task-ledger via SubMsg
    let accept_msg = serde_json::json!({
        "accept_task": {
            "task_id": task_id,
            "agent_addr": agent_addr,
        }
    });

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: task_ledger.to_string(),
        msg: to_json_binary(&accept_msg)?,
        funds: vec![],
    });

    let event = Event::new("wasm-ibc_accept_task")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("agent_addr", &agent_addr)
        .add_attribute("origin_chain", &agent_origin_chain)
        .add_attribute("origin_addr", &agent_origin_addr);

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_event(event)
        .add_attribute("action", "ibc_accept_task"))
}

/// SubmitProof — forward to zk-verifier
fn execute_submit_proof(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    task_id: u64,
    proof_b64: String,
    public_inputs_b64: String,
    agent_origin_chain: String,
    agent_origin_addr: String,
) -> Result<Response, ContractError> {
    let config = HOST_CONFIG.load(deps.storage)?;
    let zk_verifier = config
        .zk_verifier
        .ok_or(ContractError::Unauthorized {
            reason: "zk-verifier not configured".into(),
        })?;

    let mut stats = HOST_STATS.load(deps.storage)?;
    stats.total_submit_proof += 1;
    HOST_STATS.save(deps.storage, &stats)?;

    let verify_msg = serde_json::json!({
        "verify_proof": {
            "task_id": task_id,
            "proof_base64": proof_b64,
            "public_inputs_base64": public_inputs_b64,
        }
    });

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: zk_verifier.to_string(),
        msg: to_json_binary(&verify_msg)?,
        funds: vec![],
    });

    let event = Event::new("wasm-ibc_submit_proof")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("origin_chain", &agent_origin_chain)
        .add_attribute("origin_addr", &agent_origin_addr);

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_event(event)
        .add_attribute("action", "ibc_submit_proof"))
}

/// ReclaimExpired — forward to escrow
fn execute_reclaim_expired(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    task_id: u64,
    dao_origin_chain: String,
    dao_origin_addr: String,
) -> Result<Response, ContractError> {
    let config = HOST_CONFIG.load(deps.storage)?;
    let escrow = config
        .escrow
        .ok_or(ContractError::EscrowNotConfigured {})?;

    let mut stats = HOST_STATS.load(deps.storage)?;
    stats.total_reclaim += 1;
    HOST_STATS.save(deps.storage, &stats)?;

    let reclaim_msg = serde_json::json!({
        "reclaim_expired": {
            "task_id": task_id,
        }
    });

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: escrow.to_string(),
        msg: to_json_binary(&reclaim_msg)?,
        funds: vec![],
    });

    let event = Event::new("wasm-ibc_reclaim_expired")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("origin_chain", &dao_origin_chain)
        .add_attribute("origin_addr", &dao_origin_addr);

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_event(event)
        .add_attribute("action", "ibc_reclaim_expired"))
}

/// Swap — validate pair whitelist, forward funds + swap instruction to junoswap-pair.
///
/// The ICS-20 transfer carries the offer tokens as `info.funds`. After the swap
/// executes, the return tokens are sent back to the agent via IBC reverse transfer
/// (handled by the ICS-20 ack / PFM return path).
fn execute_swap(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pair_contract: String,
    offer_denom: String,
    min_return: String,
    agent_origin_chain: String,
    agent_origin_addr: String,
    max_price_impact_bps: Option<u32>,
) -> Result<Response, ContractError> {
    let config = HOST_CONFIG.load(deps.storage)?;

    // Validate pair is whitelisted
    let pair_addr = deps.api.addr_validate(&pair_contract)?;
    if !config.allowed_pairs.contains(&pair_addr) {
        return Err(ContractError::InvalidPairContract {
            addr: pair_contract,
        });
    }

    // Ensure funds were attached
    if info.funds.is_empty() {
        return Err(ContractError::NoFunds {});
    }

    // Find the offer amount from attached funds
    let offer_amount = info
        .funds
        .iter()
        .find(|c| c.denom == offer_denom)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());

    if offer_amount.is_zero() {
        return Err(ContractError::NoFunds {});
    }

    let mut stats = HOST_STATS.load(deps.storage)?;
    stats.total_swap += 1;
    HOST_STATS.save(deps.storage, &stats)?;

    // Build swap message to junoswap-pair
    let min_return_u128: Uint128 = min_return
        .parse::<u128>()
        .map(Uint128::from)
        .map_err(|_| ContractError::SlippageExceeded {
            got: "0".into(),
            min_return: min_return.clone(),
        })?;

    let swap_msg = serde_json::json!({
        "swap": {
            "offer_asset": { "native": offer_denom },
            "min_return": min_return_u128.to_string(),
        }
    });

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: pair_addr.to_string(),
        msg: to_json_binary(&swap_msg)?,
        funds: coins(offer_amount.u128(), &offer_denom),
    });

    let event = Event::new("wasm-ibc_swap")
        .add_attribute("pair", &pair_contract)
        .add_attribute("offer_denom", &offer_denom)
        .add_attribute("offer_amount", offer_amount.to_string())
        .add_attribute("min_return", &min_return)
        .add_attribute("origin_chain", &agent_origin_chain)
        .add_attribute("origin_addr", &agent_origin_addr)
        .add_attribute("max_price_impact_bps",
            max_price_impact_bps.map_or("none".to_string(), |v| v.to_string()));

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_event(event)
        .add_attribute("action", "ibc_swap"))
}

/// Admin: update config
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    task_ledger: Option<String>,
    escrow: Option<String>,
    zk_verifier: Option<String>,
    allowed_pairs: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    let mut config = HOST_CONFIG.load(deps.storage)?;

    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {
            reason: "only admin can update config".into(),
        });
    }

    if let Some(tl) = task_ledger {
        config.task_ledger = Some(deps.api.addr_validate(&tl)?);
    }
    if let Some(e) = escrow {
        config.escrow = Some(deps.api.addr_validate(&e)?);
    }
    if let Some(zk) = zk_verifier {
        config.zk_verifier = Some(deps.api.addr_validate(&zk)?);
    }
    if let Some(pairs) = allowed_pairs {
        config.allowed_pairs = pairs
            .iter()
            .map(|a| deps.api.addr_validate(a))
            .collect::<StdResult<Vec<_>>>()?;
    }

    HOST_CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_ibc_task_host_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config = HOST_CONFIG.load(deps.storage)?;
            to_json_binary(&HostConfigResponse {
                admin: config.admin,
                task_ledger: config.task_ledger,
                escrow: config.escrow,
                zk_verifier: config.zk_verifier,
                allowed_pairs: config.allowed_pairs,
            })
        }
        QueryMsg::Stats {} => {
            let stats = HOST_STATS.load(deps.storage)?;
            to_json_binary(&HostStatsResponse {
                total_accept_task: stats.total_accept_task,
                total_submit_proof: stats.total_submit_proof,
                total_reclaim: stats.total_reclaim,
                total_swap: stats.total_swap,
            })
        }
    }
}
