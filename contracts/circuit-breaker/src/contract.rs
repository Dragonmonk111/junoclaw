use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, GetBreakerResponse, InstantiateMsg, IsLockedResponse, QueryMsg,
};
use crate::state::{BreakerRecord, ADMIN, BREAKERS};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.save(deps.storage, &admin)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::TripBreaker {
            robot_id,
            reason,
            cause_ref,
        } => execute_trip_breaker(deps, env, info, robot_id, reason, cause_ref),
        ExecuteMsg::ResetBreaker { robot_id, reset_by } => {
            execute_reset_breaker(deps, env, info, robot_id, reset_by)
        }
        ExecuteMsg::TransferAdmin { new_admin } => {
            execute_transfer_admin(deps, info, new_admin)
        }
    }
}

fn execute_trip_breaker(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    reason: String,
    cause_ref: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    if reason.is_empty() {
        return Err(ContractError::InvalidParams {
            reason: "reason must not be empty".to_string(),
        });
    }

    let existing = BREAKERS.may_load(deps.storage, &robot_id)?;
    if let Some(ref record) = existing {
        if record.is_tripped() {
            return Err(ContractError::AlreadyTripped { robot_id });
        }
    }

    let record = BreakerRecord {
        state: "tripped".to_string(),
        reason: Some(reason.clone()),
        tripped_at: Some(env.block.height),
        cause_ref: Some(cause_ref.clone()),
        reset_at: None,
        reset_by: None,
    };

    BREAKERS.save(deps.storage, &robot_id, &record)?;

    Ok(Response::new()
        .add_attribute("action", "trip_breaker")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("reason", &reason)
        .add_attribute("cause_ref", &cause_ref)
        .add_attribute("block_height", env.block.height.to_string()))
}

fn execute_reset_breaker(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    reset_by: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let existing = BREAKERS
        .load(deps.storage, &robot_id)
        .map_err(|_| ContractError::BreakerNotFound {
            robot_id: robot_id.clone(),
        })?;

    if !existing.is_tripped() {
        return Err(ContractError::NotTripped { robot_id });
    }

    let record = BreakerRecord {
        state: "reset".to_string(),
        reason: existing.reason,
        tripped_at: existing.tripped_at,
        cause_ref: existing.cause_ref,
        reset_at: Some(env.block.height),
        reset_by: Some(reset_by.clone()),
    };

    BREAKERS.save(deps.storage, &robot_id, &record)?;

    Ok(Response::new()
        .add_attribute("action", "reset_breaker")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("reset_by", &reset_by)
        .add_attribute("block_height", env.block.height.to_string()))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let new_addr = deps.api.addr_validate(&new_admin)?;
    ADMIN.save(deps.storage, &new_addr)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", new_addr.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBreaker { robot_id } => {
            let record = BREAKERS.load(deps.storage, &robot_id).map_err(|_| {
                cosmwasm_std::StdError::not_found(format!(
                    "circuit breaker not found for robot {}",
                    robot_id
                ))
            })?;
            let resp = GetBreakerResponse {
                robot_id,
                state: record.state,
                reason: record.reason,
                tripped_at: record.tripped_at,
                cause_ref: record.cause_ref,
                reset_at: record.reset_at,
                reset_by: record.reset_by,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetAdmin {} => {
            let admin = ADMIN.load(deps.storage)?;
            let resp = AdminResponse {
                admin: admin.to_string(),
            };
            to_json_binary(&resp)
        }
        QueryMsg::IsLocked { robot_id } => {
            let record = BREAKERS.may_load(deps.storage, &robot_id)?;
            let (is_locked, reason) = match record {
                Some(r) if r.is_tripped() => (true, r.reason),
                _ => (false, None),
            };
            let resp = IsLockedResponse {
                robot_id,
                is_locked,
                reason,
            };
            to_json_binary(&resp)
        }
    }
}
