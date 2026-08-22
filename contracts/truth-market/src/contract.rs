use cosmwasm_std::{
    coins, ensure, ensure_eq, entry_point, to_json_binary, Addr, BankMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128,
};

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, EpochResponse, ExecuteMsg, FingerprintsResponse, FingerprintEntry,
    InstantiateMsg, MigrateMsg, OperatorResponse, OperatorsResponse, QueryMsg, StatsResponse,
};
use crate::state::{
    Config, EpochResult, MarketStats, Operator, RewardMode, VerdictRecord, CONFIG, EPOCHS,
    FINGERPRINT_COUNTS, MARKET_STATS, NEXT_OPERATOR_ID, OPERATORS, REWARD_POOL, VERDICTS,
};

const VALID_VERDICTS: &[&str] = &["green", "yellow", "red"];

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        admin: info.sender.clone(),
        min_stake: msg.min_stake,
        slash_percent: msg.slash_percent,
        reward_percent: msg.reward_percent,
        denom: msg.denom,
        unstake_cooldown_secs: msg.unstake_cooldown_secs,
        min_operators: msg.min_operators.unwrap_or(3),
        reward_mode: msg.reward_mode.unwrap_or_default(),
        verification_fee: msg.verification_fee.unwrap_or(Uint128::zero()),
    };
    CONFIG.save(deps.storage, &config)?;

    MARKET_STATS.save(
        deps.storage,
        &MarketStats {
            total_operators: 0,
            active_operators: 0,
            total_staked: Uint128::zero(),
            total_rewards_paid: Uint128::zero(),
            total_slashed: Uint128::zero(),
            epochs_finalized: 0,
        },
    )?;

    REWARD_POOL.save(deps.storage, &Uint128::zero())?;
    NEXT_OPERATOR_ID.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", info.sender))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterOperator { fingerprint } => execute_register(deps, env, info, fingerprint),
        ExecuteMsg::SubmitVerdict {
            batch_height,
            verdict,
            messages_hash,
        } => execute_submit_verdict(deps, info, batch_height, verdict, messages_hash),
        ExecuteMsg::FinalizeEpoch {
            batch_height,
            consensus_verdict,
            messages_hash,
        } => execute_finalize_epoch(deps, env, info, batch_height, consensus_verdict, messages_hash),
        ExecuteMsg::RequestUnstake {} => execute_request_unstake(deps, env, info),
        ExecuteMsg::WithdrawUnstake {} => execute_withdraw_unstake(deps, env, info),
        ExecuteMsg::Deactivate {} => execute_deactivate(deps, info),
        ExecuteMsg::Reactivate {} => execute_reactivate(deps, info),
        ExecuteMsg::UpdateConfig {
            min_stake,
            slash_percent,
            reward_percent,
            unstake_cooldown_secs,
            min_operators,
            reward_mode,
            verification_fee,
        } => execute_update_config(deps, info, min_stake, slash_percent, reward_percent, unstake_cooldown_secs, min_operators, reward_mode, verification_fee),
        ExecuteMsg::DepositRewards {} => execute_deposit_rewards(deps, info),
        ExecuteMsg::PayVerificationFee { batch_height, robot_id } => execute_pay_verification_fee(deps, info, batch_height, robot_id),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Try loading the current Config shape first (already migrated).
    // If that fails, fall back to OldConfig (pre-migration shape).
    #[derive(serde::Deserialize)]
    struct OldConfig {
        admin: Addr,
        min_stake: Uint128,
        slash_percent: u8,
        reward_percent: u8,
        denom: String,
        unstake_cooldown_secs: u64,
    }
    let raw = deps.storage.get(b"config");
    if let Some(data) = raw {
        // Try current shape first
        if let Ok(current) = cosmwasm_std::from_json::<Config>(&data) {
            // Already has all fields — just save it back to ensure storage format is current
            CONFIG.save(deps.storage, &current)?;
        } else if let Ok(old) = cosmwasm_std::from_json::<OldConfig>(&data) {
            // Old shape — migrate up with defaults for new fields
            let new_config = Config {
                admin: old.admin,
                min_stake: old.min_stake,
                slash_percent: old.slash_percent,
                reward_percent: old.reward_percent,
                denom: old.denom,
                unstake_cooldown_secs: old.unstake_cooldown_secs,
                min_operators: 3,
                reward_mode: RewardMode::default(),
                verification_fee: Uint128::zero(),
            };
            CONFIG.save(deps.storage, &new_config)?;
            // Patch existing operators: add fingerprint=None for old state.
            let ops: Vec<_> = OPERATORS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending).filter_map(|r| r.ok()).collect();
            for (addr, mut op) in ops {
                if op.fingerprint.is_none() {
                    op.fingerprint = None;
                    OPERATORS.save(deps.storage, &addr, &op)?;
                }
            }
        }
    }
    Ok(Response::new().add_attribute("method", "migrate"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        QueryMsg::GetOperator { address } => to_json_binary(&query_operator(deps, address)?),
        QueryMsg::ListOperators {} => to_json_binary(&query_list_operators(deps)?),
        QueryMsg::GetVerdict { batch_height, operator } => {
            to_json_binary(&query_verdict(deps, batch_height, operator)?)
        }
        QueryMsg::GetEpoch { batch_height } => to_json_binary(&query_epoch(deps, batch_height)?),
        QueryMsg::GetStats {} => to_json_binary(&query_stats(deps)?),
        QueryMsg::GetRewardPool {} => to_json_binary(&query_reward_pool(deps)?),
        QueryMsg::GetFingerprints {} => to_json_binary(&query_fingerprints(deps)?),
    }
}

fn execute_register(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    fingerprint: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Check if already registered
    if OPERATORS.has(deps.storage, &info.sender) {
        return Err(ContractError::AlreadyRegistered {
            address: info.sender.to_string(),
        });
    }

    // Check stake amount
    let sent = info
        .funds
        .iter()
        .find(|c| c.denom == config.denom)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());

    ensure!(
        sent >= config.min_stake,
        ContractError::InsufficientStake {
            required: config.min_stake,
            sent,
        }
    );

    let operator = Operator {
        address: info.sender.clone(),
        stake: sent,
        total_rewards: Uint128::zero(),
        total_slashed: Uint128::zero(),
        epochs_participated: 0,
        correct_verdicts: 0,
        incorrect_verdicts: 0,
        active: true,
        unstake_request_time: 0,
        fingerprint: fingerprint.clone(),
    };
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    if let Some(ref fp) = fingerprint {
        let count = FINGERPRINT_COUNTS.may_load(deps.storage, fp)?.unwrap_or(0) + 1;
        FINGERPRINT_COUNTS.save(deps.storage, fp, &count)?;
    }

    // Update stats
    let mut stats = MARKET_STATS.load(deps.storage)?;
    stats.total_operators += 1;
    stats.active_operators += 1;
    stats.total_staked += sent;
    MARKET_STATS.save(deps.storage, &stats)?;

    let _ = env;

    Ok(Response::new()
        .add_attribute("method", "register_operator")
        .add_attribute("operator", info.sender)
        .add_attribute("stake", sent)
        .add_attribute("fingerprint", fingerprint.unwrap_or_default()))
}

fn execute_submit_verdict(
    deps: DepsMut,
    info: MessageInfo,
    batch_height: u64,
    verdict: String,
    messages_hash: String,
) -> Result<Response, ContractError> {
    // Validate verdict
    ensure!(
        VALID_VERDICTS.contains(&verdict.as_str()),
        ContractError::InvalidVerdict { verdict }
    );

    // Check operator is registered and active
    let mut operator = OPERATORS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::OperatorNotFound {
            address: info.sender.to_string(),
        })?;

    ensure!(operator.active, ContractError::NotActive {});

    // Check no duplicate verdict for this batch
    let key = (batch_height, &info.sender);
    if VERDICTS.has(deps.storage, key) {
        return Err(ContractError::DuplicateVerdict {
            batch_height,
            operator: info.sender.to_string(),
        });
    }

    // Save verdict
    let record = VerdictRecord {
        operator: info.sender.clone(),
        verdict: verdict.clone(),
        batch_height,
    };
    VERDICTS.save(deps.storage, key, &record)?;

    operator.epochs_participated += 1;
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    Ok(Response::new()
        .add_attribute("method", "submit_verdict")
        .add_attribute("operator", info.sender)
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("verdict", verdict)
        .add_attribute("messages_hash", messages_hash))
}

fn execute_finalize_epoch(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    batch_height: u64,
    consensus_verdict: String,
    messages_hash: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only admin (relayer) can finalize
    ensure_eq!(
        info.sender,
        config.admin,
        ContractError::Unauthorized {}
    );

    // Check epoch not already finalized
    if EPOCHS.has(deps.storage, batch_height) {
        let existing = EPOCHS.load(deps.storage, batch_height)?;
        if existing.finalized {
            return Err(ContractError::EpochAlreadyFinalized { batch_height });
        }
    }

    // Collect all verdicts for this batch height
    let verdicts: Vec<(Addr, VerdictRecord)> = VERDICTS
        .prefix(batch_height)
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .filter_map(|r| r.ok())
        .collect();

    ensure!(!verdicts.is_empty(), ContractError::NoVerdicts { batch_height });

    // Enforce minimum operator count to prevent single-operator self-consensus
    let submitted = verdicts.len() as u32;
    ensure!(
        submitted >= config.min_operators,
        ContractError::InsufficientOperators {
            required: config.min_operators,
            submitted,
        }
    );

    // Validate consensus verdict
    ensure!(
        VALID_VERDICTS.contains(&consensus_verdict.as_str()),
        ContractError::InvalidVerdict {
            verdict: consensus_verdict,
        }
    );

    // Count matching and diverging operators
    let mut matching: Vec<Addr> = Vec::new();
    let mut diverging: Vec<Addr> = Vec::new();

    for (addr, record) in &verdicts {
        if record.verdict == consensus_verdict {
            matching.push(addr.clone());
        } else {
            diverging.push(addr.clone());
        }
    }

    let total_operators = verdicts.len() as u64;
    let matching_count = matching.len() as u64;
    let diverging_count = diverging.len() as u64;

    // Distribute rewards from the reward pool to matching operators
    let reward_pool = REWARD_POOL.load(deps.storage)?;
    let total_reward_pool = if matching_count > 0 && !reward_pool.is_zero() {
        reward_pool.multiply_ratio(config.reward_percent as u64, 100u64)
    } else {
        Uint128::zero()
    };

    // Calculate per-operator reward based on reward mode
    let matching_stakes: Vec<Uint128> = matching
        .iter()
        .map(|addr| OPERATORS.load(deps.storage, addr).map(|op| op.stake).unwrap_or(Uint128::zero()))
        .collect();
    let total_matching_stake: Uint128 = matching_stakes.iter().sum();

    let per_operator_rewards: Vec<Uint128> = match config.reward_mode {
        RewardMode::Equal => {
            let per = if matching_count > 0 {
                total_reward_pool / Uint128::from(matching_count)
            } else {
                Uint128::zero()
            };
            vec![per; matching.len()]
        }
        RewardMode::StakeWeighted => {
            if !total_matching_stake.is_zero() {
                matching_stakes
                    .iter()
                    .map(|&s| total_reward_pool.multiply_ratio(s.u128(), total_matching_stake.u128()))
                    .collect()
            } else {
                vec![Uint128::zero(); matching.len()]
            }
        }
        RewardMode::StakeTimesAccuracy => {
            // Laplace-smoothed accuracy: (correct + 1) / (epochs + 1)
            // New operators start at 100% (benefit of the doubt).
            let weights: Vec<u128> = matching
                .iter()
                .zip(matching_stakes.iter())
                .map(|(addr, &stake)| {
                    let op = OPERATORS.load(deps.storage, addr).unwrap_or(Operator {
                        address: addr.clone(),
                        stake,
                        total_rewards: Uint128::zero(),
                        total_slashed: Uint128::zero(),
                        epochs_participated: 0,
                        correct_verdicts: 0,
                        incorrect_verdicts: 0,
                        active: true,
                        unstake_request_time: 0,
                        fingerprint: None,
                    });
                    let correct = op.correct_verdicts as u128 + 1;
                    let epochs = op.epochs_participated as u128 + 1;
                    let accuracy = correct * 1000 / epochs; // scaled by 1000 for precision
                    stake.u128() * accuracy
                })
                .collect();
            let total_weight: u128 = weights.iter().sum();
            if total_weight > 0 {
                weights
                    .iter()
                    .map(|&w| total_reward_pool.multiply_ratio(w, total_weight))
                    .collect()
            } else {
                vec![Uint128::zero(); matching.len()]
            }
        }
    };

    let total_rewards_distributed: Uint128 = per_operator_rewards.iter().sum();

    // Slash diverging operators.
    // When verification_fee is set, slash equals the fee amount — aligning
    // the penalty with what robots pay for verification. When fee is 0
    // (open access mode), fall back to slash_percent of stake.
    let mut total_slashed = Uint128::zero();

    for addr in &diverging {
        let mut op = OPERATORS.load(deps.storage, addr)?;
        let slash_amount = if !config.verification_fee.is_zero() {
            // Slash = verification fee, capped at remaining stake
            if config.verification_fee > op.stake {
                op.stake
            } else {
                config.verification_fee
            }
        } else {
            op.stake.multiply_ratio(config.slash_percent as u64, 100u64)
        };
        op.stake = op.stake.checked_sub(slash_amount).unwrap_or(Uint128::zero());
        op.total_slashed += slash_amount;
        op.incorrect_verdicts += 1;
        OPERATORS.save(deps.storage, addr, &op)?;
        total_slashed += slash_amount;
    }

    // Reward matching operators
    let mut reward_msgs: Vec<cosmwasm_std::BankMsg> = Vec::new();
    for (i, addr) in matching.iter().enumerate() {
        let mut op = OPERATORS.load(deps.storage, addr)?;
        let reward = per_operator_rewards[i];
        op.total_rewards += reward;
        op.correct_verdicts += 1;
        OPERATORS.save(deps.storage, addr, &op)?;

        if !reward.is_zero() {
            reward_msgs.push(BankMsg::Send {
                to_address: addr.to_string(),
                amount: coins(reward.u128(), &config.denom),
            });
        }
    }

    // Deduct distributed rewards from pool
    let new_pool = reward_pool.checked_sub(total_rewards_distributed).unwrap_or(Uint128::zero());
    REWARD_POOL.save(deps.storage, &new_pool)?;

    // Add slashed amounts back to reward pool
    let new_pool = new_pool + total_slashed;
    REWARD_POOL.save(deps.storage, &new_pool)?;

    // Save epoch result
    let epoch_result = EpochResult {
        batch_height,
        consensus_verdict: consensus_verdict.clone(),
        messages_hash: messages_hash.clone(),
        total_operators,
        matching_operators: matching_count,
        diverging_operators: diverging_count,
        rewards_distributed: total_rewards_distributed,
        slashed_amount: total_slashed,
        finalized: true,
    };
    EPOCHS.save(deps.storage, batch_height, &epoch_result)?;

    // Update market stats
    let mut stats = MARKET_STATS.load(deps.storage)?;
    stats.total_rewards_paid += total_rewards_distributed;
    stats.total_slashed += total_slashed;
    stats.epochs_finalized += 1;
    stats.total_staked = stats.total_staked.checked_sub(total_slashed).unwrap_or(stats.total_staked);
    MARKET_STATS.save(deps.storage, &stats)?;

    let _ = env;

    Ok(Response::new()
        .add_attribute("method", "finalize_epoch")
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("consensus_verdict", consensus_verdict)
        .add_attribute("total_operators", total_operators.to_string())
        .add_attribute("matching", matching_count.to_string())
        .add_attribute("diverging", diverging_count.to_string())
        .add_attribute("rewards_distributed", total_rewards_distributed)
        .add_attribute("slashed", total_slashed)
        .add_messages(reward_msgs))
}

fn execute_request_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut operator = OPERATORS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::OperatorNotFound {
            address: info.sender.to_string(),
        })?;

    ensure!(
        operator.unstake_request_time == 0,
        ContractError::Unauthorized {}
    );

    operator.unstake_request_time = env.block.time.seconds();
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    Ok(Response::new()
        .add_attribute("method", "request_unstake")
        .add_attribute("operator", info.sender)
        .add_attribute("request_time", operator.unstake_request_time.to_string()))
}

fn execute_withdraw_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut operator = OPERATORS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::OperatorNotFound {
            address: info.sender.to_string(),
        })?;

    ensure!(
        operator.unstake_request_time > 0,
        ContractError::NoPendingUnstake {}
    );

    let elapsed = env.block.time.seconds() - operator.unstake_request_time;
    ensure!(
        elapsed >= config.unstake_cooldown_secs,
        ContractError::CooldownNotElapsed {}
    );

    let withdraw_amount = operator.stake;
    operator.stake = Uint128::zero();
    operator.active = false;
    operator.unstake_request_time = 0;
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    // Update stats
    let mut stats = MARKET_STATS.load(deps.storage)?;
    stats.active_operators = stats.active_operators.saturating_sub(1);
    stats.total_staked = stats.total_staked.checked_sub(withdraw_amount).unwrap_or(stats.total_staked);
    MARKET_STATS.save(deps.storage, &stats)?;

    let withdraw_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(withdraw_amount.u128(), &config.denom),
    };

    Ok(Response::new()
        .add_attribute("method", "withdraw_unstake")
        .add_attribute("operator", info.sender)
        .add_attribute("amount", withdraw_amount)
        .add_message(withdraw_msg))
}

fn execute_deactivate(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut operator = OPERATORS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::OperatorNotFound {
            address: info.sender.to_string(),
        })?;

    ensure!(operator.active, ContractError::AlreadyDeactivated {});
    operator.active = false;
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    let mut stats = MARKET_STATS.load(deps.storage)?;
    stats.active_operators = stats.active_operators.saturating_sub(1);
    MARKET_STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_attribute("method", "deactivate")
        .add_attribute("operator", info.sender))
}

fn execute_reactivate(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut operator = OPERATORS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::OperatorNotFound {
            address: info.sender.to_string(),
        })?;

    ensure!(!operator.active, ContractError::AlreadyActive {});
    ensure!(
        !operator.stake.is_zero(),
        ContractError::InsufficientStake {
            required: CONFIG.load(deps.storage)?.min_stake,
            sent: Uint128::zero(),
        }
    );

    operator.active = true;
    OPERATORS.save(deps.storage, &info.sender, &operator)?;

    let mut stats = MARKET_STATS.load(deps.storage)?;
    stats.active_operators += 1;
    MARKET_STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_attribute("method", "reactivate")
        .add_attribute("operator", info.sender))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    min_stake: Option<Uint128>,
    slash_percent: Option<u8>,
    reward_percent: Option<u8>,
    unstake_cooldown_secs: Option<u64>,
    min_operators: Option<u32>,
    reward_mode: Option<RewardMode>,
    verification_fee: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    if let Some(ms) = min_stake {
        config.min_stake = ms;
    }
    if let Some(sp) = slash_percent {
        config.slash_percent = sp;
    }
    if let Some(rp) = reward_percent {
        config.reward_percent = rp;
    }
    if let Some(cd) = unstake_cooldown_secs {
        config.unstake_cooldown_secs = cd;
    }
    if let Some(mo) = min_operators {
        config.min_operators = mo;
    }
    if let Some(rm) = reward_mode {
        config.reward_mode = rm;
    }
    if let Some(vf) = verification_fee {
        config.verification_fee = vf;
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "update_config"))
}

fn execute_deposit_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let amount = info
        .funds
        .iter()
        .find(|c| c.denom == config.denom)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());

    if amount.is_zero() {
        return Err(ContractError::InsufficientStake {
            required: Uint128::from(1u128),
            sent: Uint128::zero(),
        });
    }

    let mut pool = REWARD_POOL.load(deps.storage)?;
    pool += amount;
    REWARD_POOL.save(deps.storage, &pool)?;

    Ok(Response::new()
        .add_attribute("method", "deposit_rewards")
        .add_attribute("amount", amount)
        .add_attribute("new_pool", pool))
}

fn execute_pay_verification_fee(
    deps: DepsMut,
    info: MessageInfo,
    batch_height: u64,
    robot_id: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let amount = info
        .funds
        .iter()
        .find(|c| c.denom == config.denom)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());

    if amount.is_zero() {
        return Err(ContractError::InsufficientStake {
            required: Uint128::from(1u128),
            sent: Uint128::zero(),
        });
    }

    // If verification_fee is set, enforce exact match
    if !config.verification_fee.is_zero() && amount != config.verification_fee {
        return Err(ContractError::InsufficientStake {
            required: config.verification_fee,
            sent: amount,
        });
    }

    let mut pool = REWARD_POOL.load(deps.storage)?;
    pool += amount;
    REWARD_POOL.save(deps.storage, &pool)?;

    Ok(Response::new()
        .add_attribute("method", "pay_verification_fee")
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("amount", amount)
        .add_attribute("robot_id", robot_id.as_deref().unwrap_or("unknown"))
        .add_attribute("new_pool", pool))
}

// ──────────────────────────────────────────────
// Query handlers
// ──────────────────────────────────────────────

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        admin: config.admin.to_string(),
        min_stake: config.min_stake,
        slash_percent: config.slash_percent,
        reward_percent: config.reward_percent,
        denom: config.denom,
        unstake_cooldown_secs: config.unstake_cooldown_secs,
        min_operators: config.min_operators,
        reward_mode: config.reward_mode,
        verification_fee: config.verification_fee,
    })
}

fn query_operator(deps: Deps, address: String) -> StdResult<OperatorResponse> {
    let addr = Addr::unchecked(&address);
    let op = OPERATORS.load(deps.storage, &addr)?;
    let accuracy = if op.epochs_participated > 0 {
        op.correct_verdicts * 100 / op.epochs_participated
    } else {
        0
    };
    Ok(OperatorResponse {
        address: op.address.to_string(),
        stake: op.stake,
        total_rewards: op.total_rewards,
        total_slashed: op.total_slashed,
        epochs_participated: op.epochs_participated,
        correct_verdicts: op.correct_verdicts,
        incorrect_verdicts: op.incorrect_verdicts,
        active: op.active,
        accuracy,
        fingerprint: op.fingerprint,
    })
}

fn query_list_operators(deps: Deps) -> StdResult<OperatorsResponse> {
    let operators: Vec<OperatorResponse> = OPERATORS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .filter_map(|r| r.ok())
        .map(|(_, op)| {
            let accuracy = if op.epochs_participated > 0 {
                op.correct_verdicts * 100 / op.epochs_participated
            } else {
                0
            };
            OperatorResponse {
                address: op.address.to_string(),
                stake: op.stake,
                total_rewards: op.total_rewards,
                total_slashed: op.total_slashed,
                epochs_participated: op.epochs_participated,
                correct_verdicts: op.correct_verdicts,
                incorrect_verdicts: op.incorrect_verdicts,
                active: op.active,
                accuracy,
                fingerprint: op.fingerprint,
            }
        })
        .collect();

    Ok(OperatorsResponse { operators })
}

fn query_verdict(
    deps: Deps,
    batch_height: u64,
    operator: String,
) -> StdResult<VerdictRecord> {
    let addr = Addr::unchecked(&operator);
    VERDICTS.load(deps.storage, (batch_height, &addr))
}

fn query_epoch(deps: Deps, batch_height: u64) -> StdResult<EpochResponse> {
    let epoch = EPOCHS.load(deps.storage, batch_height)?;
    Ok(EpochResponse {
        batch_height: epoch.batch_height,
        consensus_verdict: epoch.consensus_verdict,
        messages_hash: epoch.messages_hash,
        total_operators: epoch.total_operators,
        matching_operators: epoch.matching_operators,
        diverging_operators: epoch.diverging_operators,
        rewards_distributed: epoch.rewards_distributed,
        slashed_amount: epoch.slashed_amount,
        finalized: epoch.finalized,
    })
}

fn query_stats(deps: Deps) -> StdResult<StatsResponse> {
    let stats = MARKET_STATS.load(deps.storage)?;
    let pool = REWARD_POOL.load(deps.storage)?;
    Ok(StatsResponse {
        total_operators: stats.total_operators,
        active_operators: stats.active_operators,
        total_staked: stats.total_staked,
        total_rewards_paid: stats.total_rewards_paid,
        total_slashed: stats.total_slashed,
        epochs_finalized: stats.epochs_finalized,
        reward_pool: pool,
    })
}

fn query_reward_pool(deps: Deps) -> StdResult<Uint128> {
    REWARD_POOL.load(deps.storage)
}

fn query_fingerprints(deps: Deps) -> StdResult<FingerprintsResponse> {
    let mut fingerprints: Vec<FingerprintEntry> = FINGERPRINT_COUNTS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .filter_map(|r| r.ok())
        .map(|(fp, count)| FingerprintEntry {
            fingerprint: fp,
            operator_count: count,
        })
        .collect();

    // Count operators without fingerprint
    let operators_without_fingerprint = OPERATORS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .filter_map(|r| r.ok())
        .filter(|(_, op)| op.fingerprint.is_none())
        .count() as u64;

    // Sort by operator_count descending so the most correlated fingerprints appear first
    fingerprints.sort_by(|a, b| b.operator_count.cmp(&a.operator_count));

    Ok(FingerprintsResponse {
        fingerprints,
        operators_without_fingerprint,
    })
}
