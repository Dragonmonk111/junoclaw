use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{DisputeResolution, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, LedgerStats, CONFIG, LEDGER_STATS, NEXT_OBLIGATION_ID, OBLIGATIONS,
    OBLIGATIONS_BY_TASK, TX_HASHES,
};
use junoclaw_common::{
    Constraint, ContractRegistry, ObligationStatus, PaymentObligation, TaskRecord,
};

const CONTRACT_NAME: &str = "crates.io:junoclaw-payment-ledger";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_TX_HASH_LEN: usize = 128;

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

    let task_ledger_addr = deps.api.addr_validate(&msg.task_ledger)?;

    // Initialise the registry. The direct `task_ledger` field is always
    // required; we mirror it into `registry.task_ledger` so downstream
    // lookups via `config.registry.task_ledger` work without an explicit
    // `UpdateRegistry` call.
    let registry = match msg.registry {
        Some(r) => {
            let agent_registry = match r.agent_registry {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            let task_ledger = match r.task_ledger {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => Some(task_ledger_addr.clone()),
            };
            let escrow = match r.escrow {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            ContractRegistry { agent_registry, task_ledger, escrow }
        }
        None => ContractRegistry {
            agent_registry: None,
            task_ledger: Some(task_ledger_addr.clone()),
            escrow: None,
        },
    };

    let config = Config {
        admin,
        task_ledger: task_ledger_addr,
        timeout_seconds: msg.timeout_seconds,
        denom: msg.denom.unwrap_or_else(|| "ujunox".to_string()),
        registry,
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
        ExecuteMsg::ExpirePending { task_id } => execute_expire_pending(deps, env, task_id),
        ExecuteMsg::ResolveDispute { task_id, resolution } => {
            execute_resolve_dispute(deps, env, info, task_id, resolution)
        }
        ExecuteMsg::UpdateConfig {
            admin,
            task_ledger,
            timeout_seconds,
        } => execute_update_config(deps, info, admin, task_ledger, timeout_seconds),
        ExecuteMsg::UpdateRegistry {
            agent_registry,
            task_ledger,
            escrow,
        } => execute_update_registry(deps, info, agent_registry, task_ledger, escrow),
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
    let task = resolve_task(deps.as_ref(), &config.task_ledger, task_id)?;
    check_entitlement(
        &task,
        &env.contract.address,
        &info.sender,
        task_id,
        &payee_addr,
        amount,
    )?;
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

fn resolve_task(deps: Deps, task_ledger: &Addr, key: u64) -> Result<TaskRecord, ContractError> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum TaskLedgerQuery {
        GetTaskByProposal { proposal_id: u64 },
        GetTask { task_id: u64 },
    }

    let by_proposal: Option<TaskRecord> = deps
        .querier
        .query_wasm_smart(
            task_ledger.to_string(),
            &TaskLedgerQuery::GetTaskByProposal { proposal_id: key },
        )
        .map_err(|_| ContractError::TaskNotFound { task_id: key })?;
    if let Some(task) = by_proposal {
        return Ok(task);
    }

    let task: TaskRecord = deps
        .querier
        .query_wasm_smart(
            task_ledger.to_string(),
            &TaskLedgerQuery::GetTask { task_id: key },
        )
        .map_err(|_| ContractError::TaskNotFound { task_id: key })?;
    if task.proposal_id.is_some() {
        return Err(ContractError::TaskNotFound { task_id: key });
    }
    Ok(task)
}

fn check_entitlement(
    task: &TaskRecord,
    escrow: &Addr,
    sender: &Addr,
    task_id: u64,
    payee: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    let is_submitter = *sender == task.submitter;
    let mut pinned = false;
    let mut entitled = false;

    for hook in task.pre_hooks.iter().chain(task.post_hooks.iter()) {
        let Constraint::EscrowObligationConfirmed {
            escrow: pin_escrow,
            task_id: pin_task_id,
            payer: pin_payer,
            payee: pin_payee,
            min_amount: pin_min,
        } = hook
        else {
            continue;
        };
        if pin_escrow != escrow || *pin_task_id != task_id {
            continue;
        }
        pinned = true;

        let payer_ok = match pin_payer {
            Some(p) => p == sender,
            None => is_submitter,
        };
        entitled |= payer_ok;
        let payee_ok = pin_payee.as_ref().map_or(true, |p| p == payee);
        let amount_ok = pin_min.map_or(true, |m| amount >= m);
        if payer_ok && payee_ok && amount_ok {
            return Ok(());
        }
    }

    if !pinned {
        return if is_submitter {
            Ok(())
        } else {
            Err(ContractError::Unauthorized {})
        };
    }
    if entitled {
        Err(ContractError::PinMismatch { task_id })
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Payer confirms they sent funds directly to payee (off-contract).
fn execute_confirm(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    tx_hash: Option<String>,
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
    // An attested (`Verified`) obligation must still be confirmable:
    // `AttachAttestation` moves Pending -> Verified without touching the
    // pending total, so the stats below stay consistent for both origins.
    if !matches!(
        obligation.status,
        ObligationStatus::Pending | ObligationStatus::Verified
    ) {
        return Err(ContractError::NotPending { obligation_id });
    }
    if let Some(h) = &tx_hash {
        if h.is_empty() || h.len() > MAX_TX_HASH_LEN {
            return Err(ContractError::InvalidTxHash {});
        }
    }

    obligation.status = ObligationStatus::Confirmed;
    obligation.settled_at = Some(env.block.time.seconds());
    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;
    if let Some(h) = tx_hash {
        TX_HASHES.save(deps.storage, obligation_id, &h)?;
    }

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

    // The task-ledger fires `Cancel` when an operator fails or admin-cancels
    // a task (a third-party verdict, never a submitter self-declaration), the
    // same trust it already has for `Confirm`.
    if info.sender != obligation.payer
        && info.sender != config.admin
        && info.sender != config.task_ledger
    {
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

/// Cancel a `Pending` obligation whose payer never settled it. Anyone may
/// call this once `created_at + timeout_seconds` has passed.
fn execute_expire_pending(
    deps: DepsMut,
    env: Env,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.timeout_seconds == 0 {
        return Err(ContractError::TimeoutDisabled {});
    }
    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;
    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;
    if obligation.status != ObligationStatus::Pending {
        return Err(ContractError::NotPending { obligation_id });
    }

    let expires_at = obligation.created_at.saturating_add(config.timeout_seconds);
    if env.block.time.seconds() <= expires_at {
        return Err(ContractError::NotExpired { task_id, expires_at });
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
        .add_attribute("action", "expire_pending")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string()))
}

/// Admin closes a `Disputed` obligation. Without this a dispute is a
/// permanent ledger entry.
fn execute_resolve_dispute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    resolution: DisputeResolution,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let obligation_id = OBLIGATIONS_BY_TASK
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::NoObligationForTask { task_id })?;
    let mut obligation = OBLIGATIONS.load(deps.storage, obligation_id)?;
    if obligation.status != ObligationStatus::Disputed {
        return Err(ContractError::NotDisputed { obligation_id });
    }

    let outcome = match resolution {
        DisputeResolution::ConfirmObligation => {
            obligation.status = ObligationStatus::Confirmed;
            "confirmed"
        }
        DisputeResolution::CancelObligation => {
            obligation.status = ObligationStatus::Cancelled;
            "cancelled"
        }
    };
    obligation.settled_at = Some(env.block.time.seconds());
    OBLIGATIONS.save(deps.storage, obligation_id, &obligation)?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_disputed = s.total_disputed.saturating_sub(obligation.amount);
        match resolution {
            DisputeResolution::ConfirmObligation => {
                s.total_confirmed = s.total_confirmed.checked_add(obligation.amount)?;
            }
            DisputeResolution::CancelObligation => {
                s.total_cancelled = s.total_cancelled.checked_add(obligation.amount)?;
            }
        }
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "resolve_dispute")
        .add_attribute("obligation_id", obligation_id.to_string())
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("outcome", outcome))
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
    timeout_seconds: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(tl) = task_ledger {
        let validated = deps.api.addr_validate(&tl)?;
        config.task_ledger = validated.clone();
        // Mirror the change into the canonical registry pointer so callback
        // auth checks and the cross-contract pointer stay in lockstep.
        config.registry.task_ledger = Some(validated);
    }
    if let Some(ts) = timeout_seconds {
        config.timeout_seconds = ts;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Admin-only: rewire any subset of the cross-contract registry pointers.
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
        let validated = deps.api.addr_validate(a)?;
        config.registry.task_ledger = Some(validated.clone());
        // Keep the direct auth-field in lockstep.
        config.task_ledger = validated;
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
        QueryMsg::GetTxHash { task_id } => {
            let hash = match OBLIGATIONS_BY_TASK.may_load(deps.storage, task_id)? {
                Some(id) => TX_HASHES.may_load(deps.storage, id)?,
                None => None,
            };
            to_json_binary(&hash)
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
