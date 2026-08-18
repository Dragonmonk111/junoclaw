use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, EscrowStats, LeaseRequest, LeaseStatus, CONFIG, LEASES, LEASES_BY_REQUESTER,
    LEASE_COUNT, STATS,
};

const CONTRACT_NAME: &str = "crates.io:junoclaw-emergency-compute-escrow";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_DENOM: &str = "ujuno";
const DEFAULT_MIN_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_TIMEOUT_SECS: u64 = 3600;

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
        denom: msg.denom.unwrap_or_else(|| DEFAULT_DENOM.to_string()),
        max_cost_per_lease: msg.max_cost_per_lease,
        min_timeout_secs: msg.min_timeout_secs.unwrap_or(DEFAULT_MIN_TIMEOUT_SECS),
        max_timeout_secs: msg.max_timeout_secs.unwrap_or(DEFAULT_MAX_TIMEOUT_SECS),
        moultbook: msg.moultbook.map(|a| deps.api.addr_validate(&a)).transpose()?,
        task_ledger: msg.task_ledger.map(|a| deps.api.addr_validate(&a)).transpose()?,
    };
    CONFIG.save(deps.storage, &config)?;
    LEASE_COUNT.save(deps.storage, &1u64)?;
    STATS.save(deps.storage, &EscrowStats::default())?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.to_string())
        .add_attribute("max_cost_per_lease", config.max_cost_per_lease.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RequestLease {
            provider,
            task_id,
            confidence_score,
            max_cost,
            timeout_secs,
        } => execute_request_lease(deps, env, info, provider, task_id, confidence_score, max_cost, timeout_secs),
        ExecuteMsg::ConfirmLeaseActive { lease_id } => {
            execute_confirm_lease_active(deps, info, lease_id)
        }
        ExecuteMsg::CompleteLease {
            lease_id,
            actual_cost,
            payout_addr,
        } => execute_complete_lease(deps, env, info, lease_id, actual_cost, payout_addr),
        ExecuteMsg::CancelLease { lease_id } => execute_cancel_lease(deps, env, info, lease_id),
        ExecuteMsg::ExpireLease { lease_id } => execute_expire_lease(deps, env, lease_id),
        ExecuteMsg::UpdateConfig {
            max_cost_per_lease,
            min_timeout_secs,
            max_timeout_secs,
            moultbook,
            task_ledger,
        } => execute_update_config(
            deps,
            info,
            max_cost_per_lease,
            min_timeout_secs,
            max_timeout_secs,
            moultbook,
            task_ledger,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_request_lease(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    provider: String,
    task_id: Option<u64>,
    confidence_score: u8,
    max_cost: Uint128,
    timeout_secs: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if provider.trim().is_empty() {
        return Err(ContractError::EmptyProvider {});
    }
    if confidence_score > 100 {
        return Err(ContractError::InvalidConfidenceScore {});
    }
    if max_cost > config.max_cost_per_lease {
        return Err(ContractError::CostCapExceeded {
            requested: max_cost,
            max: config.max_cost_per_lease,
        });
    }
    if timeout_secs < config.min_timeout_secs || timeout_secs > config.max_timeout_secs {
        return Err(ContractError::InvalidTimeout {
            min: config.min_timeout_secs,
            max: config.max_timeout_secs,
        });
    }

    let sent: Vec<String> = info
        .funds
        .iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .collect();
    let matching = info.funds.iter().find(|c| c.denom == config.denom).cloned();
    if info.funds.len() != 1 {
        return Err(ContractError::WrongFunds {
            denom: config.denom.clone(),
            got: sent,
        });
    }
    let sent_amount = matching
        .ok_or_else(|| ContractError::WrongFunds {
            denom: config.denom.clone(),
            got: sent.clone(),
        })?
        .amount;
    if sent_amount < max_cost {
        return Err(ContractError::InsufficientFunds {
            sent: sent_amount,
            max_cost,
        });
    }

    let lease_id = LEASE_COUNT.load(deps.storage)?;
    let now = env.block.time.seconds();
    let lease = LeaseRequest {
        id: lease_id,
        requester: info.sender.clone(),
        provider,
        task_id,
        confidence_score,
        escrowed: sent_amount,
        max_cost,
        status: LeaseStatus::Pending,
        requested_at: now,
        deadline: now + timeout_secs,
        actual_cost: None,
        resolved_at: None,
    };
    LEASES.save(deps.storage, lease_id, &lease)?;
    LEASE_COUNT.save(deps.storage, &(lease_id + 1))?;
    LEASES_BY_REQUESTER.save(deps.storage, (&info.sender, lease_id), &())?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_leases += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "request_lease")
        .add_attribute("lease_id", lease_id.to_string())
        .add_attribute("requester", info.sender.to_string())
        .add_attribute("max_cost", max_cost.to_string())
        .add_attribute("deadline", lease.deadline.to_string()))
}

fn execute_confirm_lease_active(
    deps: DepsMut,
    info: MessageInfo,
    lease_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut lease = LEASES
        .may_load(deps.storage, lease_id)?
        .ok_or(ContractError::LeaseNotFound { lease_id })?;
    if !matches!(lease.status, LeaseStatus::Pending) {
        return Err(ContractError::NotPending { lease_id });
    }
    if info.sender != lease.requester && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    lease.status = LeaseStatus::Active;
    LEASES.save(deps.storage, lease_id, &lease)?;

    Ok(Response::new()
        .add_attribute("action", "confirm_lease_active")
        .add_attribute("lease_id", lease_id.to_string()))
}

fn execute_complete_lease(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lease_id: u64,
    actual_cost: Uint128,
    payout_addr: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut lease = LEASES
        .may_load(deps.storage, lease_id)?
        .ok_or(ContractError::LeaseNotFound { lease_id })?;
    if !matches!(lease.status, LeaseStatus::Active) {
        return Err(ContractError::NotActive { lease_id });
    }
    if info.sender != lease.requester && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    if actual_cost > lease.escrowed {
        return Err(ContractError::ActualCostExceedsEscrow {
            actual: actual_cost,
            escrowed: lease.escrowed,
        });
    }

    let payout = deps.api.addr_validate(&payout_addr)?;
    let refund = lease.escrowed - actual_cost;

    lease.status = LeaseStatus::Completed;
    lease.actual_cost = Some(actual_cost);
    lease.resolved_at = Some(env.block.time.seconds());
    LEASES.save(deps.storage, lease_id, &lease)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_paid_to_providers += actual_cost;
        s.total_refunded += refund;
        Ok(s)
    })?;

    let mut resp = Response::new()
        .add_attribute("action", "complete_lease")
        .add_attribute("lease_id", lease_id.to_string())
        .add_attribute("actual_cost", actual_cost.to_string())
        .add_attribute("refund", refund.to_string());

    if !actual_cost.is_zero() {
        resp = resp.add_message(BankMsg::Send {
            to_address: payout.to_string(),
            amount: vec![Coin { denom: config.denom.clone(), amount: actual_cost }],
        });
    }
    if !refund.is_zero() {
        resp = resp.add_message(BankMsg::Send {
            to_address: lease.requester.to_string(),
            amount: vec![Coin { denom: config.denom, amount: refund }],
        });
    }

    Ok(resp)
}

fn execute_cancel_lease(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lease_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut lease = LEASES
        .may_load(deps.storage, lease_id)?
        .ok_or(ContractError::LeaseNotFound { lease_id })?;
    if !matches!(lease.status, LeaseStatus::Pending) {
        return Err(ContractError::NotPending { lease_id });
    }
    if info.sender != lease.requester && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    lease.status = LeaseStatus::Cancelled;
    lease.resolved_at = Some(env.block.time.seconds());
    let amount = lease.escrowed;
    let requester = lease.requester.clone();
    LEASES.save(deps.storage, lease_id, &lease)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_refunded += amount;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: requester.to_string(),
            amount: vec![Coin { denom: config.denom, amount }],
        })
        .add_attribute("action", "cancel_lease")
        .add_attribute("lease_id", lease_id.to_string()))
}

/// Permissionless — this is the on-chain reconciliation half of the
/// reflex-tier fail-safe. The local agent does not wait on this
/// transaction: once ITS OWN watchdog timeout fires it immediately falls
/// back to its safe-state policy. This call just settles the escrow after
/// the fact so funds don't stay locked.
fn execute_expire_lease(
    deps: DepsMut,
    env: Env,
    lease_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut lease = LEASES
        .may_load(deps.storage, lease_id)?
        .ok_or(ContractError::LeaseNotFound { lease_id })?;
    if !matches!(lease.status, LeaseStatus::Pending | LeaseStatus::Active) {
        return Err(ContractError::AlreadyResolved { lease_id });
    }
    let now = env.block.time.seconds();
    if now < lease.deadline {
        return Err(ContractError::DeadlineNotReached {
            lease_id,
            now,
            deadline: lease.deadline,
        });
    }

    lease.status = LeaseStatus::Expired;
    lease.resolved_at = Some(now);
    let amount = lease.escrowed;
    let requester = lease.requester.clone();
    LEASES.save(deps.storage, lease_id, &lease)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_refunded += amount;
        s.total_expired += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: requester.to_string(),
            amount: vec![Coin { denom: config.denom, amount }],
        })
        .add_attribute("action", "expire_lease")
        .add_attribute("lease_id", lease_id.to_string()))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    max_cost_per_lease: Option<Uint128>,
    min_timeout_secs: Option<u64>,
    max_timeout_secs: Option<u64>,
    moultbook: Option<String>,
    task_ledger: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(c) = max_cost_per_lease {
        config.max_cost_per_lease = c;
    }
    if let Some(t) = min_timeout_secs {
        config.min_timeout_secs = t;
    }
    if let Some(t) = max_timeout_secs {
        config.max_timeout_secs = t;
    }
    if let Some(a) = moultbook {
        config.moultbook = Some(deps.api.addr_validate(&a)?);
    }
    if let Some(a) = task_ledger {
        config.task_ledger = Some(deps.api.addr_validate(&a)?);
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetLease { lease_id } => to_json_binary(&LEASES.load(deps.storage, lease_id)?),
        QueryMsg::ListLeasesByRequester { requester, limit } => {
            let addr = deps.api.addr_validate(&requester)?;
            let limit = limit.unwrap_or(20).min(50) as usize;
            let leases: Vec<LeaseRequest> = LEASES_BY_REQUESTER
                .prefix(&addr)
                .range(deps.storage, None, None, Order::Descending)
                .take(limit)
                .filter_map(|r| r.ok().and_then(|(id, _)| LEASES.may_load(deps.storage, id).ok().flatten()))
                .collect();
            to_json_binary(&leases)
        }
        QueryMsg::GetStats {} => to_json_binary(&STATS.load(deps.storage)?),
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
