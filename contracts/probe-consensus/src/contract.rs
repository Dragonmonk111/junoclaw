use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult,
};

use crate::error::ContractError;
use crate::msg::{
    AgreedResponse, CommitsResponse, CommitResponse, ConfigResponse, ExecuteMsg, InstantiateMsg,
    OperatorsResponse, QueryMsg,
};
use crate::state::{AgreedRoot, Commit, AGREED, ADMIN, COMMITMENTS, OPERATORS, REQUIRED};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.required == 0 {
        return Err(ContractError::InvalidParams {
            reason: "required must be >= 1".to_string(),
        });
    }
    let admin = match msg.admin {
        Some(a) => deps.api.addr_validate(&a)?,
        None => info.sender.clone(),
    };
    ADMIN.save(deps.storage, &admin)?;
    REQUIRED.save(deps.storage, &msg.required)?;

    let mut count = 0u32;
    for op in &msg.operators {
        let addr = deps.api.addr_validate(op)?;
        OPERATORS.save(deps.storage, addr, &())?;
        count += 1;
    }
    if count < msg.required {
        return Err(ContractError::InvalidParams {
            reason: format!(
                "operator set ({count}) smaller than required quorum ({})",
                msg.required
            ),
        });
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin.to_string())
        .add_attribute("required", msg.required.to_string())
        .add_attribute("operators", count.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterOperator { addr } => exec_register(deps, info, addr),
        ExecuteMsg::RemoveOperator { addr } => exec_remove(deps, info, addr),
        ExecuteMsg::Commit {
            robot_id,
            batch_height,
            merkle_root,
            cycle_count,
        } => exec_commit(
            deps,
            env,
            info,
            robot_id,
            batch_height,
            merkle_root,
            cycle_count,
        ),
        ExecuteMsg::Finalize {
            robot_id,
            batch_height,
        } => exec_finalize(deps, env, robot_id, batch_height),
        ExecuteMsg::TransferAdmin { new_admin } => exec_transfer_admin(deps, info, new_admin),
    }
}

fn exec_register(
    deps: DepsMut,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }
    let a = deps.api.addr_validate(&addr)?;
    OPERATORS.save(deps.storage, a.clone(), &())?;
    Ok(Response::new()
        .add_attribute("action", "register_operator")
        .add_attribute("operator", a.to_string()))
}

fn exec_remove(
    deps: DepsMut,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }
    let a = deps.api.addr_validate(&addr)?;
    OPERATORS.remove(deps.storage, a.clone());
    Ok(Response::new()
        .add_attribute("action", "remove_operator")
        .add_attribute("operator", a.to_string()))
}

fn exec_commit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    batch_height: u64,
    merkle_root: String,
    cycle_count: u32,
) -> Result<Response, ContractError> {
    if !OPERATORS.has(deps.storage, info.sender.clone()) {
        return Err(ContractError::NotOperator {
            addr: info.sender.to_string(),
        });
    }
    if robot_id.is_empty() || merkle_root.is_empty() || cycle_count == 0 {
        return Err(ContractError::InvalidParams {
            reason: "robot_id and merkle_root must be non-empty; cycle_count > 0".to_string(),
        });
    }
    if AGREED.has(deps.storage, (&robot_id.as_str(), batch_height)) {
        return Err(ContractError::AlreadyAgreed {
            robot_id,
            batch_height,
        });
    }
    if COMMITMENTS.has(deps.storage, (&robot_id.as_str(), batch_height, &info.sender)) {
        return Err(ContractError::AlreadyCommitted {
            robot_id,
            batch_height,
        });
    }

    COMMITMENTS.save(
        deps.storage,
        (&robot_id.as_str(), batch_height, &info.sender),
        &Commit {
            merkle_root: merkle_root.clone(),
            cycle_count,
            committed_at: env.block.height,
        },
    )?;

    let mut resp = Response::new()
        .add_attribute("action", "commit")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("operator", info.sender.to_string())
        .add_attribute("merkle_root", &merkle_root);

    // Auto-finalize when this commit completes an agreeing quorum.
    if let Some(agreed) = try_finalize(deps.as_ref(), env.block.height, &robot_id, batch_height)? {
        AGREED.save(deps.storage, (&robot_id.as_str(), batch_height), &agreed)?;
        resp = resp
            .add_attribute("action", "commit_and_finalize")
            .add_attribute("agreed_root", &agreed.merkle_root)
            .add_attribute("committers", agreed.commits.len().to_string());
    }
    Ok(resp)
}

fn exec_finalize(
    deps: DepsMut,
    env: Env,
    robot_id: String,
    batch_height: u64,
) -> Result<Response, ContractError> {
    if AGREED.has(deps.storage, (&robot_id.as_str(), batch_height)) {
        return Ok(Response::new()
            .add_attribute("action", "finalize")
            .add_attribute("result", "already_agreed")
            .add_attribute("robot_id", robot_id)
            .add_attribute("batch_height", batch_height.to_string()));
    }

    let agreed = match try_finalize(deps.as_ref(), env.block.height, &robot_id, batch_height)? {
        Some(a) => a,
        None => {
            let commits = collect_commits(deps.as_ref(), &robot_id, batch_height)?;
            let distinct = distinct_pairs(&commits);
            let required = REQUIRED.load(deps.storage)? as usize;
            return Err(if distinct > 1 {
                ContractError::CommitsDisagree {
                    distinct,
                    total: commits.len(),
                }
            } else {
                ContractError::QuorumNotReached {
                    have: commits.len(),
                    need: required as u32,
                }
            });
        }
    };

    AGREED.save(deps.storage, (&robot_id.as_str(), batch_height), &agreed)?;

    Ok(Response::new()
        .add_attribute("action", "finalize")
        .add_attribute("result", "agreed")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("merkle_root", &agreed.merkle_root)
        .add_attribute("committers", agreed.commits.len().to_string()))
}

/// Collect every commit for a batch in deterministic key order.
fn collect_commits(
    deps: Deps,
    robot_id: &str,
    batch_height: u64,
) -> StdResult<Vec<(Addr, Commit)>> {
    let mut out = Vec::new();
    for item in COMMITMENTS
        .prefix((robot_id, batch_height))
        .range(deps.storage, None, None, Order::Ascending)
    {
        let (operator, commit) = item?;
        out.push((operator, commit));
    }
    Ok(out)
}

fn distinct_pairs(commits: &[(Addr, Commit)]) -> usize {
    let mut pairs: Vec<(&str, u32)> = commits
        .iter()
        .map(|(_, c)| (c.merkle_root.as_str(), c.cycle_count))
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs.len()
}

/// Returns the AgreedRoot iff commits exist, all identical (root+cycle),
/// and the distinct-operator count meets `required`.
fn try_finalize(
    deps: Deps,
    height: u64,
    robot_id: &str,
    batch_height: u64,
) -> Result<Option<AgreedRoot>, ContractError> {
    let required = REQUIRED.load(deps.storage)?.max(1) as usize;
    let commits = collect_commits(deps, robot_id, batch_height)?;
    if commits.len() < required || commits.is_empty() {
        return Ok(None);
    }
    let first = &commits[0].1;
    if commits
        .iter()
        .any(|(_, c)| c.merkle_root != first.merkle_root || c.cycle_count != first.cycle_count)
    {
        return Ok(None); // disagreement — caller surfaces CommitsDisagree
    }
    Ok(Some(AgreedRoot {
        merkle_root: first.merkle_root.clone(),
        cycle_count: first.cycle_count,
        commits: commits.iter().map(|(a, _)| a.to_string()).collect(),
        agreed_at: height,
    }))
}

fn exec_transfer_admin(
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
        QueryMsg::GetConfig {} => {
            let admin = ADMIN.load(deps.storage)?;
            let required = REQUIRED.load(deps.storage)?;
            let operator_count = OPERATORS
                .range(deps.storage, None, None, Order::Ascending)
                .count() as u32;
            to_json_binary(&ConfigResponse {
                admin: admin.to_string(),
                required,
                operator_count,
            })
        }
        QueryMsg::GetCommit {
            robot_id,
            batch_height,
            operator,
        } => {
            let op = deps.api.addr_validate(&operator)?;
            let commit = COMMITMENTS
                .load(deps.storage, (&robot_id.as_str(), batch_height, &op))
                .map_err(|_| {
                    cosmwasm_std::StdError::not_found(format!(
                        "no commit for {} batch {} by {}",
                        robot_id, batch_height, operator
                    ))
                })?;
            to_json_binary(&CommitResponse {
                robot_id,
                batch_height,
                operator,
                merkle_root: commit.merkle_root,
                cycle_count: commit.cycle_count,
                committed_at: commit.committed_at,
            })
        }
        QueryMsg::ListCommits {
            robot_id,
            batch_height,
        } => {
            let commits = collect_commits(deps, &robot_id, batch_height)?;
            to_json_binary(&CommitsResponse {
                commits: commits
                    .into_iter()
                    .map(|(op, c)| CommitResponse {
                        robot_id: robot_id.clone(),
                        batch_height,
                        operator: op.to_string(),
                        merkle_root: c.merkle_root,
                        cycle_count: c.cycle_count,
                        committed_at: c.committed_at,
                    })
                    .collect(),
            })
        }
        QueryMsg::GetAgreed {
            robot_id,
            batch_height,
        } => {
            let agreed = AGREED
                .load(deps.storage, (&robot_id.as_str(), batch_height))
                .map_err(|_| {
                    cosmwasm_std::StdError::not_found(format!(
                        "no agreed root for {} batch {}",
                        robot_id, batch_height
                    ))
                })?;
            to_json_binary(&AgreedResponse {
                robot_id,
                batch_height,
                merkle_root: agreed.merkle_root,
                cycle_count: agreed.cycle_count,
                commits: agreed.commits,
                agreed_at: agreed.agreed_at,
            })
        }
        QueryMsg::ListOperators {} => {
            let mut ops = Vec::new();
            for item in OPERATORS.range(deps.storage, None, None, Order::Ascending) {
                let (addr, _) = item?;
                ops.push(addr.to_string());
            }
            to_json_binary(&OperatorsResponse { operators: ops })
        }
    }
}
