use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, LedgerStats, CONFIG, LEDGER_STATS, NEXT_OBLIGATION_ID, OBLIGATIONS,
    OBLIGATIONS_BY_TASK,
};
use junoclaw_common::{ContractRegistry, ObligationStatus, PaymentObligation};

const CONTRACT_NAME: &str = "crates.io:junoclaw-payment-ledger";
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
        task_ledger: deps.api.addr_validate(&msg.task_ledger)?,
        timeout_blocks: msg.timeout_blocks,
        denom: msg.denom.unwrap_or_else(|| "ujunox".to_string()),
        registry: ContractRegistry {
            agent_registry: None,
            task_ledger: None,
            escrow: None,
        },
    };
    CONFIG.save(deps.storage, &config)?;
    NEXT_OBLIGATION_ID.save(deps.storage, &1u64)?;
    LEDGER_STATS.save(
        deps.storage,
        &LedgerStats {
            total_obligations: 0,
            total_pending: Uint128::zero(),
            total_confirmed: Uint128::zero(),
            total_disputed: Uint128::zero(),
            total_cancelled: Uint128::zero(),
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Authorize {
            task_id,
            payee,
            amount,
        } => execute_authorize(deps, env, info, task_id, payee, amount),
        ExecuteMsg::Confirm { task_id, tx_hash } => {
            execute_confirm(deps, env, info, task_id, tx_hash)
        }
        ExecuteMsg::Dispute { task_id, reason } => {
            execute_dispute(deps, env, info, task_id, reason)
        }
        ExecuteMsg::Cancel { task_id } => execute_cancel(deps, env, info, task_id),
        ExecuteMsg::AttachAttestation {
            task_id,
            attestation_hash,
        } => execute_attach_attestation(deps, info, task_id, attestation_hash),
        ExecuteMsg::UpdateConfig {
            admin,
            task_ledger,
            timeout_blocks,
        } => execute_update_config(deps, info, admin, task_ledger, timeout_blocks),
    }
}

/// Record a payment obligation. No funds are sent to the contract.
fn execute_authorize(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    payee: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if OBLIGATIONS_BY_TASK.has(deps.storage, task_id) {
        return Err(ContractError::AlreadyAuthorized { task_id });
    }
    if amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    let config = CONFIG.load(deps.storage)?;
    let payee_addr = deps.api.addr_validate(&payee).map_err(|_| ContractError::InvalidPayee {})?;
    let obligation_id = NEXT_OBLIGATION_ID.load(deps.storage)?;

    let obligation = PaymentObligation {
        id: obligation_id,
        payer: info.sender.clone(),
        payee: payee_addr,
        task_id,
        amount,
        denom: config.denom,
        status: ObligationStatus::Pending,
        created_at: env.block.time.seconds(),
        settled_at: None,
        attestation_hash: None,
    };

    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;
    OBLIGATIONS_BY_TASK.save(deps.storage, task_id, &obligation_id)?;
    NEXT_OBLIGATION_ID.save(deps.storage, &(obligation_id + 1))?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_obligations += 1;
        s.total_pending = s.total_pending.checked_add(amount)?;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "authorize")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("amount", amount.to_string()))
}

/// Payer confirms they sent funds directly to payee (off-contract).
fn execute_confirm(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    _tx_hash: Option<String>,
) -> Result<Response, ContractError> {
    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;

    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;

    // Only the payer, admin, or task_ledger can confirm
    let config = CONFIG.load(deps.storage)?;
    if info.sender != obligation.payer
        && info.sender != config.admin
        && info.sender != config.task_ledger
    {
        return Err(ContractError::Unauthorized {});
    }
    if obligation.status != ObligationStatus::Pending {
        return Err(ContractError::NotPending { obligation_id });
    }

    obligation.status = ObligationStatus::Confirmed;
    obligation.settled_at = Some(env.block.time.seconds());
    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_confirmed = s.total_confirmed.checked_add(obligation.amount)?;
        s.total_pending = s.total_pending.saturating_sub(obligation.amount);
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "confirm")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("amount", obligation.amount.to_string()))
}

/// Payer disputes the obligation.
fn execute_dispute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    reason: String,
) -> Result<Response, ContractError> {
    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;

    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;

    if info.sender != obligation.payer {
        return Err(ContractError::Unauthorized {});
    }
    if obligation.status != ObligationStatus::Pending {
        return Err(ContractError::NotPending { obligation_id });
    }

    obligation.status = ObligationStatus::Disputed;
    obligation.settled_at = Some(env.block.time.seconds());
    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_disputed = s.total_disputed.checked_add(obligation.amount)?;
        s.total_pending = s.total_pending.saturating_sub(obligation.amount);
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "dispute")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("reason", reason))
}

/// Cancel a pending obligation.
fn execute_cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;

    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;

    if info.sender != obligation.payer && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    if obligation.status != ObligationStatus::Pending {
        return Err(ContractError::NotPending { obligation_id });
    }

    obligation.status = ObligationStatus::Cancelled;
    obligation.settled_at = Some(env.block.time.seconds());
    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_cancelled = s.total_cancelled.checked_add(obligation.amount)?;
        s.total_pending = s.total_pending.saturating_sub(obligation.amount);
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "cancel")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string()))
}

/// Attach a WAVS attestation hash to a pending obligation.
fn execute_attach_attestation(
    deps: DepsMut,
    info: MessageInfo,
    task_id: u64,
    attestation_hash: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin && info.sender != config.task_ledger {
        return Err(ContractError::Unauthorized {});
    }

    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;

    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;
    obligation.attestation_hash = Some(attestation_hash.clone());

    if obligation.status == ObligationStatus::Pending {
        obligation.status = ObligationStatus::Verified;
    }

    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;

    Ok(Response::new()
        .add_attribute("action", "attach_attestation")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("attestation_hash", attestation_hash))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    task_ledger: Option<String>,
    timeout_blocks: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(tl) = task_ledger {
        config.task_ledger = deps.api.addr_validate(&tl)?;
    }
    if let Some(tb) = timeout_blocks {
        config.timeout_blocks = tb;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetObligation { obligation_id } => {
            to_json_binary(&OBLIGATIONS.load(deps.storage, obligation_id)?)
        }
        QueryMsg::GetObligationByTask { task_id } => {
            let obligation_id = OBLIGATIONS_BY_TASK.may_load(deps.storage, task_id)?;
            match obligation_id {
                Some(id) => to_json_binary(&Some(OBLIGATIONS.load(deps.storage, id)?)),
                None => to_json_binary(&None::<PaymentObligation>),
            }
        }
        QueryMsg::GetStats {} => to_json_binary(&LEDGER_STATS.load(deps.storage)?),
        QueryMsg::ListObligations { start_after, limit } => {
            let limit = limit.unwrap_or(20).min(50) as usize;
            let start = start_after.map(cw_storage_plus::Bound::exclusive);
            let obligations: Vec<PaymentObligation> = OBLIGATIONS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, o)| o))
                .collect();
            to_json_binary(&obligations)
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
