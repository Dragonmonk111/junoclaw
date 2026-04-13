use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, LedgerStats, CONFIG, LEDGER_STATS, NEXT_TASK_ID, TASKS, TASKS_BY_AGENT,
    TASKS_BY_SUBMITTER,
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

    let config = Config {
        admin,
        agent_registry: deps.api.addr_validate(&msg.agent_registry)?,
        operators,
        registry: ContractRegistry {
            agent_registry: None,
            task_ledger: None,
            escrow: None,
        },
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
        } => execute_submit(deps, env, info, agent_id, input_hash, execution_tier),
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
        } => execute_update_config(deps, info, admin, agent_registry),
    }
}

fn execute_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    agent_id: u64,
    input_hash: String,
    execution_tier: junoclaw_common::ExecutionTier,
) -> Result<Response, ContractError> {
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

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_tasks += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "submit_task")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("agent_id", agent_id.to_string()))
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

    TASKS.update(deps.storage, task_id, |existing| match existing {
        Some(mut record) => {
            if record.submitter != info.sender && !is_authorized(&config, &info.sender) {
                return Err(ContractError::NotSubmitter {});
            }
            if record.status != TaskStatus::Running {
                return Err(ContractError::TaskNotRunning { task_id });
            }
            record.status = TaskStatus::Completed;
            record.output_hash = Some(output_hash.clone());
            record.completed_at = Some(env.block.time.seconds());
            record.cost_ujuno = cost_ujuno;
            Ok(record)
        }
        None => Err(ContractError::TaskNotFound { task_id }),
    })?;

    LEDGER_STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_completed += 1;
        Ok(s)
    })?;

    // Retrieve the task record for agent_id (needed for registry callback)
    let task = TASKS.load(deps.storage, task_id)?;

    let mut response = Response::new()
        .add_attribute("action", "complete_task")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("output_hash", output_hash);

    // ── Status coherence: atomic callbacks ──
    // Callback 1: Confirm the escrow obligation (if escrow is wired)
    if let Some(escrow_addr) = &config.registry.escrow {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum EscrowMsg { Confirm { task_id: u64, tx_hash: Option<String> } }
        let confirm_msg = WasmMsg::Execute {
            contract_addr: escrow_addr.to_string(),
            msg: to_json_binary(&EscrowMsg::Confirm { task_id, tx_hash: None })?,
            funds: vec![],
        };
        response = response.add_message(confirm_msg);
    }

    // Callback 2: Increment agent tasks in registry (if wired via ContractRegistry)
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

    Ok(response)
}

fn execute_fail(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    TASKS.update(deps.storage, task_id, |existing| match existing {
        Some(mut record) => {
            if record.submitter != info.sender && !is_authorized(&config, &info.sender) {
                return Err(ContractError::NotSubmitter {});
            }
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
    // Callback: Increment agent tasks with success=false (if wired via ContractRegistry)
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

    // Callback: Cancel the escrow obligation (if escrow is wired)
    if let Some(escrow_addr) = &config.registry.escrow {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum EscrowMsg { Cancel { task_id: u64 } }
        let cancel_msg = WasmMsg::Execute {
            contract_addr: escrow_addr.to_string(),
            msg: to_json_binary(&EscrowMsg::Cancel { task_id })?,
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
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(ar) = agent_registry {
        config.agent_registry = deps.api.addr_validate(&ar)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetTask { task_id } => to_json_binary(&TASKS.load(deps.storage, task_id)?),
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
