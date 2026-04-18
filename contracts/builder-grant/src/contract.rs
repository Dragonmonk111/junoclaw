use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    BuilderStatsResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    StatsResponse, SubmissionsResponse,
};
use crate::state::{
    Config, SubmissionStatus, WorkSubmission, BUILDER_TOTALS, CONFIG, SUBMISSIONS,
    SUBMISSION_SEQ, WORK_HASH_USED,
};

const CONTRACT_NAME: &str = "crates.io:junoclaw-builder-grant";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn is_valid_hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut operators = Vec::new();
    for op in &msg.operators {
        operators.push(deps.api.addr_validate(op)?);
    }

    let agent_company = match msg.agent_company {
        Some(addr) => Some(deps.api.addr_validate(&addr)?),
        None => None,
    };

    let config = Config {
        admin: info.sender.clone(),
        operators,
        agent_company,
        denom: msg.denom.clone(),
        active: true,
        total_granted: 0,
        total_submissions: 0,
    };
    CONFIG.save(deps.storage, &config)?;
    SUBMISSION_SEQ.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", info.sender)
        .add_attribute("denom", msg.denom))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SubmitWork {
            tier,
            evidence,
            work_hash,
        } => execute_submit_work(deps, env, info, tier, evidence, work_hash),
        ExecuteMsg::VerifyWork {
            submission_id,
            attestation_hash,
            approved,
        } => execute_verify_work(deps, env, info, submission_id, attestation_hash, approved),
        ExecuteMsg::ClaimGrant { submission_id } => {
            execute_claim_grant(deps, env, info, submission_id)
        }
        ExecuteMsg::Fund {} => execute_fund(deps, info),
        ExecuteMsg::AddOperator { address } => execute_add_operator(deps, info, address),
        ExecuteMsg::RemoveOperator { address } => execute_remove_operator(deps, info, address),
        ExecuteMsg::SetActive { active } => execute_set_active(deps, info, active),
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, env, info, amount),
    }
}

fn execute_submit_work(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    tier: crate::state::GrantTier,
    evidence: String,
    work_hash: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if !config.active {
        return Err(ContractError::SubmissionsPaused {});
    }

    if evidence.is_empty() {
        return Err(ContractError::EmptyEvidence {});
    }

    if !is_valid_hex64(&work_hash) {
        return Err(ContractError::InvalidWorkHash {});
    }

    // ── v6 F3: work_hash is the SHA-256 of the actual work output, so a
    // duplicate hash is by definition the same work. Allowing multiple
    // submissions of the same hash would let an attacker spam the
    // pending-verification queue with structurally-identical entries
    // (different `evidence` strings, same output hash) to bloat state,
    // confuse the TEE verifier about which record is canonical, and
    // potentially race the legitimate builder's own claim. We index
    // `work_hash -> submission_id` and reject duplicates up front.
    if let Some(existing) = WORK_HASH_USED.may_load(deps.storage, work_hash.as_str())? {
        return Err(ContractError::DuplicateWorkHash {
            existing_submission_id: existing,
        });
    }

    let seq = SUBMISSION_SEQ.load(deps.storage)? + 1;
    SUBMISSION_SEQ.save(deps.storage, &seq)?;

    let submission = WorkSubmission {
        id: seq,
        builder: info.sender.clone(),
        tier: tier.clone(),
        evidence: evidence.clone(),
        work_hash: work_hash.clone(),
        status: SubmissionStatus::Pending,
        submitted_at_block: env.block.height,
        attestation_hash: None,
        verified_by: None,
    };

    SUBMISSIONS.save(deps.storage, seq, &submission)?;
    WORK_HASH_USED.save(deps.storage, work_hash.as_str(), &seq)?;
    config.total_submissions += 1;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "submit_work")
        .add_attribute("submission_id", seq.to_string())
        .add_attribute("builder", info.sender)
        .add_attribute("reward_amount", tier.reward_amount().to_string())
        .add_attribute("work_hash", work_hash))
}

fn execute_verify_work(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    submission_id: u64,
    attestation_hash: String,
    approved: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Check sender is a registered operator or admin or agent_company
    let is_operator = config.operators.contains(&info.sender);
    let is_admin = info.sender == config.admin;
    let is_governance = config
        .agent_company
        .as_ref()
        .is_some_and(|ac| *ac == info.sender);

    if !is_operator && !is_admin && !is_governance {
        return Err(ContractError::UnauthorizedOperator {});
    }

    if !is_valid_hex64(&attestation_hash) {
        return Err(ContractError::InvalidAttestationHash {});
    }

    let mut submission = SUBMISSIONS
        .may_load(deps.storage, submission_id)?
        .ok_or(ContractError::SubmissionNotFound { id: submission_id })?;

    if submission.status != SubmissionStatus::Pending {
        return Err(ContractError::SubmissionNotPending {
            id: submission_id,
            status: format!("{:?}", submission.status),
        });
    }

    submission.attestation_hash = Some(attestation_hash.clone());
    submission.verified_by = Some(info.sender.clone());

    if approved {
        submission.status = SubmissionStatus::Verified;
    } else {
        submission.status = SubmissionStatus::Rejected;
    }

    SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    Ok(Response::new()
        .add_attribute("action", "verify_work")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("approved", approved.to_string())
        .add_attribute("attestation_hash", attestation_hash)
        .add_attribute("operator", info.sender))
}

fn execute_claim_grant(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_id: u64,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    let mut submission = SUBMISSIONS
        .may_load(deps.storage, submission_id)?
        .ok_or(ContractError::SubmissionNotFound { id: submission_id })?;

    if submission.builder != info.sender {
        return Err(ContractError::NotBuilder {});
    }

    if submission.status != SubmissionStatus::Verified {
        return Err(ContractError::SubmissionNotVerified { id: submission_id });
    }

    let reward = submission.tier.reward_amount();

    // Check contract balance
    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    if balance.amount.u128() < reward {
        return Err(ContractError::InsufficientFunds {
            balance: balance.amount.u128(),
            reward,
        });
    }

    submission.status = SubmissionStatus::Claimed;
    SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    // Update builder totals
    let prev = BUILDER_TOTALS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(0);
    BUILDER_TOTALS.save(deps.storage, &info.sender, &(prev + reward))?;

    config.total_granted += reward;
    CONFIG.save(deps.storage, &config)?;

    let send_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.denom,
            amount: Uint128::from(reward),
        }],
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "claim_grant")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("builder", info.sender)
        .add_attribute("reward", reward.to_string()))
}

fn execute_fund(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let total_funded: u128 = info
        .funds
        .iter()
        .filter(|c| c.denom == config.denom)
        .map(|c| c.amount.u128())
        .sum();

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("funder", info.sender)
        .add_attribute("amount", total_funded.to_string()))
}

fn execute_add_operator(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let validated = deps.api.addr_validate(&address)?;
    if !config.operators.contains(&validated) {
        config.operators.push(validated.clone());
        CONFIG.save(deps.storage, &config)?;
    }

    Ok(Response::new()
        .add_attribute("action", "add_operator")
        .add_attribute("operator", validated))
}

fn execute_remove_operator(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let validated = deps.api.addr_validate(&address)?;
    config.operators.retain(|op| *op != validated);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "remove_operator")
        .add_attribute("operator", validated))
}

fn execute_set_active(
    deps: DepsMut,
    info: MessageInfo,
    active: bool,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    config.active = active;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "set_active")
        .add_attribute("active", active.to_string()))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let validated = deps.api.addr_validate(&new_admin)?;
    config.admin = validated.clone();
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", validated))
}

fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<u128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;
    let withdraw_amount = amount.unwrap_or(balance.amount.u128());

    let send_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.denom,
            amount: Uint128::from(withdraw_amount),
        }],
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("amount", withdraw_amount.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps, env)?),
        QueryMsg::GetSubmission { id } => to_json_binary(&query_submission(deps, id)?),
        QueryMsg::ListSubmissions {
            status,
            start_after,
            limit,
        } => to_json_binary(&query_list_submissions(deps, status, start_after, limit)?),
        QueryMsg::GetBuilderStats { address } => {
            to_json_binary(&query_builder_stats(deps, address)?)
        }
        QueryMsg::GetStats {} => to_json_binary(&query_stats(deps, env)?),
    }
}

fn query_config(deps: Deps, env: Env) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    Ok(ConfigResponse {
        admin: config.admin,
        operators: config.operators,
        agent_company: config.agent_company,
        denom: config.denom,
        active: config.active,
        balance: balance.amount.u128(),
    })
}

fn query_submission(deps: Deps, id: u64) -> StdResult<WorkSubmission> {
    SUBMISSIONS.load(deps.storage, id)
}

fn query_list_submissions(
    deps: Deps,
    status: Option<SubmissionStatus>,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<SubmissionsResponse> {
    let limit = limit.unwrap_or(20).min(100) as usize;
    let start = start_after.map(cw_storage_plus::Bound::exclusive);

    let submissions: Vec<WorkSubmission> = SUBMISSIONS
        .range(deps.storage, start, None, Order::Ascending)
        .filter_map(|item| {
            let (_, sub) = item.ok()?;
            match &status {
                Some(s) if sub.status != *s => None,
                _ => Some(sub),
            }
        })
        .take(limit)
        .collect();

    Ok(SubmissionsResponse { submissions })
}

fn query_builder_stats(deps: Deps, address: String) -> StdResult<BuilderStatsResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let total = BUILDER_TOTALS
        .may_load(deps.storage, &addr)?
        .unwrap_or(0);

    let submissions: Vec<WorkSubmission> = SUBMISSIONS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (_, sub) = item.ok()?;
            if sub.builder == addr {
                Some(sub)
            } else {
                None
            }
        })
        .collect();

    Ok(BuilderStatsResponse {
        address: addr,
        total_granted: total,
        submissions,
    })
}

fn query_stats(deps: Deps, env: Env) -> StdResult<StatsResponse> {
    let config = CONFIG.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    let pending_count: u64 = SUBMISSIONS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (_, sub) = item.ok()?;
            if sub.status == SubmissionStatus::Pending {
                Some(1u64)
            } else {
                None
            }
        })
        .sum();

    Ok(StatsResponse {
        total_granted: config.total_granted,
        total_submissions: config.total_submissions,
        balance: balance.amount.u128(),
        pending_count,
    })
}
