use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, LedgerStats, CONFIG, LEDGER_STATS, NEXT_TASK_ID, TASKS, TASKS_BY_AGENT,
    TASKS_BY_PROPOSAL, TASKS_BY_SUBMITTER,
};
use junoclaw_common::{ContractRegistry, TaskRecord, TaskStatus};

const CONTRACT_NAME: &str = "crates.io:junoclaw-task-ledger";
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

    let operators = msg
        .operators
        .unwrap_or_default()
        .iter()
        .map(|o| deps.api.addr_validate(o))
        .collect::<Result<Vec<_>, _>>()?;

    let agent_company = msg
        .agent_company
        .map(|a| deps.api.addr_validate(&a))
        .transpose()?;

    let agent_registry_addr = deps.api.addr_validate(&msg.agent_registry)?;

    // Build ContractRegistry from explicit msg.registry, falling back to
    // populating only the agent_registry pointer we already required above.
    // This guarantees that cross-contract callbacks (to agent-registry and
    // escrow) have a non-None pointer to target the moment the contract is
    // live — or, if only agent_registry is known at instantiate time, at
    // least that pointer wires through without a mandatory `UpdateRegistry`.
    let registry = match msg.registry {
        Some(r) => {
            let agent_registry = match r.agent_registry {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => Some(agent_registry_addr.clone()),
            };
            let task_ledger = match r.task_ledger {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            let escrow = match r.escrow {
                Some(a) => Some(deps.api.addr_validate(a.as_str())?),
                None => None,
            };
            ContractRegistry { agent_registry, task_ledger, escrow }
        }
        None => ContractRegistry {
            agent_registry: Some(agent_registry_addr.clone()),
            task_ledger: None,
            escrow: None,
        },
    };

    let config = Config {
        admin,
        agent_registry: agent_registry_addr,
        operators,
        agent_company,
        registry,
    };
    CONFIG.save(deps.storage, &config)?;
    NEXT_TASK_ID.save(deps.storage, &1u64)?;
    LEDGER_STATS.save(
        deps.storage,
        &LedgerStats {
            total_tasks: 0,
            total_completed: 0,
            total_failed: 0,
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
        ExecuteMsg::SubmitTask {
            agent_id,
            input_hash,
            execution_tier,
            proposal_id,
            pre_hooks,
            post_hooks,
        } => execute_submit(
            deps,
            env,
            info,
            agent_id,
            input_hash,
            execution_tier,
            proposal_id,
            pre_hooks,
            post_hooks,
        ),
        ExecuteMsg::CompleteTask {
            task_id,
            output_hash,
            cost_ujuno,
        } => execute_complete(deps, env, info, task_id, output_hash, cost_ujuno),
        ExecuteMsg::FailTask { task_id } => execute_fail(deps, env, info, task_id),
        ExecuteMsg::CancelTask { task_id } => execute_cancel(deps, info, task_id),
        ExecuteMsg::AddOperator { operator } => execute_add_operator(deps, info, operator),
        ExecuteMsg::RemoveOperator { operator } => execute_remove_operator(deps, info, operator),
        ExecuteMsg::UpdateConfig {
            admin,
            agent_registry,
            agent_company,
        } => execute_update_config(deps, info, admin, agent_registry, agent_company),
        ExecuteMsg::UpdateRegistry {
            agent_registry,
            task_ledger,
            escrow,
        } => execute_update_registry(deps, info, agent_registry, task_ledger, escrow),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    agent_id: u64,
    input_hash: String,
    execution_tier: junoclaw_common::ExecutionTier,
    proposal_id: Option<u64>,
    pre_hooks: Vec<junoclaw_common::Constraint>,
    post_hooks: Vec<junoclaw_common::Constraint>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let is_operator = is_authorized(&config, &info.sender);
    let is_agent_company = config
        .agent_company
        .as_ref()
        .map(|addr| *addr == info.sender)
        .unwrap_or(false);

    if let Some(pid) = proposal_id {
        if !is_agent_company {
            return Err(ContractError::Unauthorized {});
        }
        if TASKS_BY_PROPOSAL.has(deps.storage, pid) {
            return Err(ContractError::ProposalTaskAlreadyExists { proposal_id: pid });
        }
    } else if agent_id == 0 && !is_operator {
        return Err(ContractError::ReservedAgentId {});
    } else if agent_id != 0 && !is_operator {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum AgentRegistryQuery {
            GetAgent { agent_id: u64 },
        }

        let profile: junoclaw_common::AgentProfile = deps
            .querier
            .query_wasm_smart(
                config.agent_registry.to_string(),
                &AgentRegistryQuery::GetAgent { agent_id },
            )
            .map_err(|_| ContractError::AgentNotOwned { agent_id })?;

        if profile.owner != info.sender || !profile.is_active {
            return Err(ContractError::AgentNotOwned { agent_id });
        }
    }

    let task_id = NEXT_TASK_ID.load(deps.storage)?;

    let record = TaskRecord {
        id: task_id,
        agent_id,
        submitter: info.sender.clone(),
        input_hash,
        output_hash: None,
        execution_tier,
        status: TaskStatus::Running,
        submitted_at: env.block.time.seconds(),
        completed_at: None,
        cost_ujuno: None,
        proposal_id,
        pre_hooks,
        post_hooks,
    };

    TASKS.save(deps.storage, task_id, &record)?;
    NEXT_TASK_ID.save(deps.storage, &(task_id + 1))?;

    TASKS_BY_AGENT.update(deps.storage, agent_id, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(task_id);
        Ok(ids)
    })?;

    TASKS_BY_SUBMITTER.update(deps.storage, &info.sender, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(task_id);
        Ok(ids)
    })?;

    // Write the reverse index so downstream contracts (agent-company's
    // attestation coherence check, for instance) can resolve the real task_id
    // from the proposal_id that governance used.
    if let Some(pid) = proposal_id {
        TASKS_BY_PROPOSAL.save(deps.storage, pid, &task_id)?;
    }

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_tasks += 1;
        Ok(s)
    })?;

    let mut response = Response::new()
        .add_attribute("action", "submit_task")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("agent_id", agent_id.to_string());
    if let Some(pid) = proposal_id {
        response = response.add_attribute("proposal_id", pid.to_string());
    }
    Ok(response)
}

fn is_authorized(config: &Config, sender: &cosmwasm_std::Addr) -> bool {
    sender == config.admin || config.operators.contains(sender)
}

fn execute_complete(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
    output_hash: String,
    cost_ujuno: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // ── Completion is an attestation, not a self-declaration ──
    // Only operators (incl. admin) or the configured `agent-company` may
    // mark a task Completed. Allowing the submitter to self-complete let
    // any public `SubmitTask` caller trigger the atomic
    // `escrow::Confirm` callback and mark their own obligation paid
    // without any funds ever moving — a full bypass of escrow's payment
    // journal. Agent-company is permitted so governance-initiated tasks
    // can be marked complete when the DAO (not an operator) is the
    // settlement authority.
    if !is_authorized(&config, &info.sender)
        && config.agent_company.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }

    // ── v7: pre-completion hook evaluation ──
    // Read the task first without mutating so we can evaluate pre_hooks
    // against the *current* chain state. A violation reverts the whole
    // completion tx atomically, leaving the task Running and firing
    // neither the escrow nor the registry callback.
    let existing = TASKS
        .may_load(deps.storage, task_id)?
        .ok_or(ContractError::TaskNotFound { task_id })?;
    if existing.status != TaskStatus::Running {
        return Err(ContractError::TaskNotRunning { task_id });
    }

    let registry_addr = config.registry.agent_registry.as_ref();
    if !existing.pre_hooks.is_empty() {
        junoclaw_common::evaluate_all(
            deps.as_ref(),
            &env,
            &existing.pre_hooks,
            registry_addr,
        )
        .map_err(|reason| ContractError::ConstraintViolated {
            reason: format!("pre_hook: {}", reason),
        })?;
    }

    // ── State transition ──
    let mut record = existing;
    record.status = TaskStatus::Completed;
    record.output_hash = Some(output_hash.clone());
    record.completed_at = Some(env.block.time.seconds());
    record.cost_ujuno = cost_ujuno;
    TASKS.save(deps.storage, task_id, &record)?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_completed += 1;
        Ok(s)
    })?;

    // ── v7: post-completion hook evaluation ──
    // Evaluated after the status transition but *before* the escrow /
    // registry sub-messages are appended to the Response. Sub-messages
    // haven't fired yet at this point so post_hooks see:
    //   • the task's new `Completed` status (visible via self-query)
    //   • pre-callback escrow/registry state (their Increment/Confirm
    //     will fire only if we return Ok below)
    // This is intentional: post_hooks should express invariants on
    // state that is within this contract's write scope, not on the
    // downstream sub-message effects. For those, use a reply-based
    // hook in a future v8 pattern.
    if !record.post_hooks.is_empty() {
        junoclaw_common::evaluate_all(
            deps.as_ref(),
            &env,
            &record.post_hooks,
            registry_addr,
        )
        .map_err(|reason| ContractError::ConstraintViolated {
            reason: format!("post_hook: {}", reason),
        })?;
    }

    // Alias so the downstream callback assembly keeps its original name.
    let task = record;

    let mut response = Response::new()
        .add_attribute("action", "complete_task")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("output_hash", output_hash);

    // ── Status coherence: atomic callbacks ──
    // Escrow callback keys the obligation by whatever id the *payer* used at
    // Authorize-time. For governance-initiated tasks the payer was
    // `agent-company` and the id is the proposal_id; for daemon-initiated
    // tasks the id is the local task_id. Using `proposal_id.unwrap_or(task_id)`
    // keeps both flows coherent without forcing escrow to learn about either
    // layer's id space.
    let escrow_key = task.proposal_id.unwrap_or(task_id);

    // Callback 1: Confirm the escrow obligation (if escrow is wired)
    if let Some(escrow_addr) = &config.registry.escrow {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum EscrowMsg { Confirm { task_id: u64, tx_hash: Option<String> } }
        let confirm_msg = WasmMsg::Execute {
            contract_addr: escrow_addr.to_string(),
            msg: to_json_binary(&EscrowMsg::Confirm { task_id: escrow_key, tx_hash: None })?,
            funds: vec![],
        };
        response = response.add_message(confirm_msg);
    }

    // Callback 2: Increment agent tasks in registry (if wired via ContractRegistry).
    // `agent_id == 0` is a reserved sentinel meaning "no specific agent" —
    // `agent-company::WavsPush` uses it for governance-initiated tasks that
    // are dispatched to the WAVS layer without binding to any registered
    // agent. For such tasks there's no trust-score to bump, so skipping the
    // callback keeps the CompleteTask atomic (agent-registry would otherwise
    // reject `agent_id: 0` with `AgentNotFound` and revert the whole tx).
    if task.agent_id != 0 {
        if let Some(registry_addr) = &config.registry.agent_registry {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "snake_case")]
            enum RegistryMsg { IncrementTasks { agent_id: u64, success: bool } }
            let increment_msg = WasmMsg::Execute {
                contract_addr: registry_addr.to_string(),
                msg: to_json_binary(&RegistryMsg::IncrementTasks {
                    agent_id: task.agent_id,
                    success: true,
                })?,
                funds: vec![],
            };
            response = response.add_message(increment_msg);
        }
    }

    Ok(response)
}

fn execute_fail(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // ── Failure is a verdict, not a self-declaration ──
    // Same invariant as `execute_complete`: a `Failed` task fires the
    // atomic `escrow::Cancel` callback, which would otherwise let the
    // obligation's payer unilaterally void their debt by submitting a
    // task and marking it Failed. The submitter path is therefore
    // closed; only operators/admin/agent-company may finalise a task.
    // Users retain the ability to withdraw *their own* pending task via
    // `CancelTask`, which has no escrow side-effect.
    if !is_authorized(&config, &info.sender)
        && config.agent_company.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }

    TASKS.update(deps.storage, task_id, |existing| match existing {
        Some(mut record) => {
            if record.status != TaskStatus::Running {
                return Err(ContractError::TaskNotRunning { task_id });
            }
            record.status = TaskStatus::Failed;
            record.completed_at = Some(env.block.time.seconds());
            Ok(record)
        }
        None => Err(ContractError::TaskNotFound { task_id }),
    })?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_failed += 1;
        Ok(s)
    })?;

    // Retrieve the task record for agent_id (needed for registry callback)
    let task = TASKS.load(deps.storage, task_id)?;

    let mut response = Response::new()
        .add_attribute("action", "fail_task")
        .add_attribute("task_id", task_id.to_string());

    // ── Status coherence: atomic callbacks ──
    // Callback: Increment agent tasks with success=false (if wired via ContractRegistry).
    // Skipped for the `agent_id == 0` sentinel (governance-initiated tasks
    // with no bound agent) — see the comment in `execute_complete`.
    if task.agent_id != 0 {
        if let Some(registry_addr) = &config.registry.agent_registry {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "snake_case")]
            enum RegistryMsg { IncrementTasks { agent_id: u64, success: bool } }
            let increment_msg = WasmMsg::Execute {
                contract_addr: registry_addr.to_string(),
                msg: to_json_binary(&RegistryMsg::IncrementTasks {
                    agent_id: task.agent_id,
                    success: false,
                })?,
                funds: vec![],
            };
            response = response.add_message(increment_msg);
        }
    }

    // Callback: Cancel the escrow obligation (if escrow is wired).
    // Route via proposal_id when set so governance-originated obligations
    // (keyed by proposal_id in escrow) can be found.
    if let Some(escrow_addr) = &config.registry.escrow {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum EscrowMsg { Cancel { task_id: u64 } }
        let escrow_key = task.proposal_id.unwrap_or(task_id);
        let cancel_msg = WasmMsg::Execute {
            contract_addr: escrow_addr.to_string(),
            msg: to_json_binary(&EscrowMsg::Cancel { task_id: escrow_key })?,
            funds: vec![],
        };
        response = response.add_message(cancel_msg);
    }

    Ok(response)
}

fn execute_cancel(
    deps: DepsMut,
    info: MessageInfo,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    TASKS.update(deps.storage, task_id, |existing| match existing {
        Some(mut record) => {
            if record.submitter != info.sender && !is_authorized(&config, &info.sender) {
                return Err(ContractError::NotSubmitter {});
            }
            if record.status == TaskStatus::Completed {
                return Err(ContractError::TaskAlreadyCompleted { task_id });
            }
            record.status = TaskStatus::Cancelled;
            Ok(record)
        }
        None => Err(ContractError::TaskNotFound { task_id }),
    })?;

    Ok(Response::new()
        .add_attribute("action", "cancel_task")
        .add_attribute("task_id", task_id.to_string()))
}

fn execute_add_operator(
    deps: DepsMut,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let addr = deps.api.addr_validate(&operator)?;
    if !config.operators.contains(&addr) {
        config.operators.push(addr.clone());
        CONFIG.save(deps.storage, &config)?;
    }
    Ok(Response::new()
        .add_attribute("action", "add_operator")
        .add_attribute("operator", addr.to_string()))
}

fn execute_remove_operator(
    deps: DepsMut,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let addr = deps.api.addr_validate(&operator)?;
    config.operators.retain(|o| *o != addr);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "remove_operator")
        .add_attribute("operator", addr.to_string()))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    agent_registry: Option<String>,
    agent_company: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(ar) = agent_registry {
        let validated = deps.api.addr_validate(&ar)?;
        config.agent_registry = validated.clone();
        // Keep the canonical registry pointer in lockstep with the legacy
        // direct field so callbacks remain coherent even if admin only uses
        // the old UpdateConfig call.
        config.registry.agent_registry = Some(validated);
    }
    if let Some(ac) = agent_company {
        config.agent_company = Some(deps.api.addr_validate(&ac)?);
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Admin-only: rewire any subset of the cross-contract registry pointers.
/// A missing field leaves that pointer untouched; supplying an invalid
/// address rejects with the underlying `addr_validate` error.
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
        let validated = deps.api.addr_validate(a)?;
        config.registry.agent_registry = Some(validated.clone());
        // Mirror into the legacy direct field for consumers that still read it.
        config.agent_registry = validated;
    }
    if let Some(a) = task_ledger.as_ref() {
        config.registry.task_ledger = Some(deps.api.addr_validate(a)?);
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
        QueryMsg::GetTask { task_id } => to_json_binary(&TASKS.load(deps.storage, task_id)?),
        QueryMsg::GetTaskByProposal { proposal_id } => {
            let task = match TASKS_BY_PROPOSAL.may_load(deps.storage, proposal_id)? {
                Some(task_id) => TASKS.may_load(deps.storage, task_id)?,
                None => None,
            };
            to_json_binary(&task)
        }
        QueryMsg::GetTasksByAgent { agent_id, limit } => {
            let limit = limit.unwrap_or(20).min(50) as usize;
            let ids = TASKS_BY_AGENT
                .may_load(deps.storage, agent_id)?
                .unwrap_or_default();
            let tasks: Vec<TaskRecord> = ids
                .iter()
                .rev()
                .take(limit)
                .filter_map(|id| TASKS.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&tasks)
        }
        QueryMsg::GetTasksBySubmitter { submitter, limit } => {
            let addr = deps.api.addr_validate(&submitter)?;
            let limit = limit.unwrap_or(20).min(50) as usize;
            let ids = TASKS_BY_SUBMITTER
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            let tasks: Vec<TaskRecord> = ids
                .iter()
                .rev()
                .take(limit)
                .filter_map(|id| TASKS.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&tasks)
        }
        QueryMsg::GetStats {} => to_json_binary(&LEDGER_STATS.load(deps.storage)?),
        QueryMsg::ListTasks { start_after, limit } => {
            let limit = limit.unwrap_or(20).min(50) as usize;
            let start = start_after.map(cw_storage_plus::Bound::exclusive);
            let tasks: Vec<TaskRecord> = TASKS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, t)| t))
                .collect();
            to_json_binary(&tasks)
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
