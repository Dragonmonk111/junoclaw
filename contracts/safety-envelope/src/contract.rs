use cosmwasm_std::{
    entry_point, Addr, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, GetEnvelopeResponse, InstantiateMsg, QueryMsg,
    SafetyEnvelopeParams, VersionCountResponse,
};
use crate::state::{EnvelopeRecord, ADMIN, ENVELOPES, VERSION_COUNTS};

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
        ExecuteMsg::SetEnvelope { robot_id, params } => {
            execute_set_envelope(deps, env, info, robot_id, params)
        }
        ExecuteMsg::TightenEnvelope { robot_id, params } => {
            execute_tighten_envelope(deps, env, info, robot_id, params)
        }
        ExecuteMsg::TransferAdmin { new_admin } => {
            execute_transfer_admin(deps, info, new_admin)
        }
    }
}

fn execute_set_envelope(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    params: SafetyEnvelopeParams,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    validate_params(&params)?;

    let count = VERSION_COUNTS.may_load(deps.storage, &robot_id)?.unwrap_or(0);
    let new_version = count + 1;

    let record = EnvelopeRecord {
        params: params.clone(),
        version: new_version,
        updated_at: env.block.height,
        updated_by: info.sender.to_string(),
    };

    ENVELOPES.save(deps.storage, &robot_id, &record)?;
    VERSION_COUNTS.save(deps.storage, &robot_id, &new_version)?;

    Ok(Response::new()
        .add_attribute("action", "set_envelope")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("version", new_version.to_string())
        .add_attribute("max_speed_milli", params.max_speed_milli.to_string()))
}

fn execute_tighten_envelope(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    new_params: SafetyEnvelopeParams,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    validate_params(&new_params)?;

    let existing = ENVELOPES
        .load(deps.storage, &robot_id)
        .map_err(|_| ContractError::EnvelopeNotFound {
            robot_id: robot_id.clone(),
        })?;

    // Enforce tightening: new limits must be <= existing (safer)
    let old = &existing.params;
    if new_params.max_speed_milli > old.max_speed_milli
        || new_params.max_force_milli > old.max_force_milli
        || new_params.min_collision_distance_milli < old.min_collision_distance_milli
        || new_params.max_tilt_milli_degrees > old.max_tilt_milli_degrees
        || new_params.max_acceleration_milli > old.max_acceleration_milli
        || (new_params.human_proximity_allowed && !old.human_proximity_allowed)
    {
        return Err(ContractError::InvalidParams {
            reason: "tighten_envelope can only make limits stricter, not relax them".to_string(),
        });
    }

    let count = VERSION_COUNTS.load(deps.storage, &robot_id)?;
    let new_version = count + 1;

    let record = EnvelopeRecord {
        params: new_params.clone(),
        version: new_version,
        updated_at: env.block.height,
        updated_by: info.sender.to_string(),
    };

    ENVELOPES.save(deps.storage, &robot_id, &record)?;
    VERSION_COUNTS.save(deps.storage, &robot_id, &new_version)?;

    Ok(Response::new()
        .add_attribute("action", "tighten_envelope")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("version", new_version.to_string()))
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

fn validate_params(params: &SafetyEnvelopeParams) -> Result<(), ContractError> {
    if params.max_speed_milli == 0 {
        return Err(ContractError::InvalidParams {
            reason: "max_speed must be positive".to_string(),
        });
    }
    if params.max_force_milli == 0 {
        return Err(ContractError::InvalidParams {
            reason: "max_force must be positive".to_string(),
        });
    }
    if params.max_tilt_milli_degrees == 0 || params.max_tilt_milli_degrees > 180_000 {
        return Err(ContractError::InvalidParams {
            reason: "max_tilt_degrees must be in (0, 180]".to_string(),
        });
    }
    if params.max_acceleration_milli == 0 {
        return Err(ContractError::InvalidParams {
            reason: "max_acceleration must be positive".to_string(),
        });
    }
    Ok(())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::GetEnvelope { robot_id } => {
            let record = ENVELOPES.load(deps.storage, &robot_id).map_err(|_| {
                cosmwasm_std::StdError::not_found(format!(
                    "safety envelope not found for robot {}",
                    robot_id
                ))
            })?;
            let resp = GetEnvelopeResponse {
                robot_id,
                params: record.params,
                version: record.version,
                updated_at: record.updated_at,
                updated_by: record.updated_by,
            };
            cosmwasm_std::to_json_binary(&resp)
        }
        QueryMsg::GetAdmin {} => {
            let admin = ADMIN.load(deps.storage)?;
            let resp = AdminResponse {
                admin: admin.to_string(),
            };
            cosmwasm_std::to_json_binary(&resp)
        }
        QueryMsg::GetVersionCount { robot_id } => {
            let count = VERSION_COUNTS.may_load(deps.storage, &robot_id)?.unwrap_or(0);
            let resp = VersionCountResponse { robot_id, count };
            cosmwasm_std::to_json_binary(&resp)
        }
    }
}
