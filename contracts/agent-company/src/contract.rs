use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, MemberInput, MigrateMsg, ProposalKindMsg, QueryMsg,
};
use crate::state::{
    Config, Member, NoisCallback, PaymentRecord, PendingSortition, Proposal, ProposalKind,
    ProposalStatus, SortitionRound, Vote, VoteOption, WeightProposal,
    CONFIG, MEMBER_EARNINGS, PAYMENT_HISTORY, PENDING_SORTITION, PROPOSAL, PROPOSALS,
    PROPOSAL_SEQ, SORTITION_ROUNDS, SORTITION_SEQ,
};
use sha2::{Sha256, Digest};

const CONTRACT_NAME: &str = "crates.io:junoclaw-agent-company";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const TOTAL_WEIGHT: u64 = 10_000;

fn parse_members(deps: &DepsMut, inputs: Vec<MemberInput>) -> Result<Vec<Member>, ContractError> {
    if inputs.is_empty() {
        return Err(ContractError::EmptyMembers {});
    }
    let mut members = Vec::with_capacity(inputs.len());
    let mut seen = std::collections::HashSet::new();
    let mut total: u64 = 0;
    for m in inputs {
        let addr = deps.api.addr_validate(&m.addr)?;
        if !seen.insert(addr.clone()) {
            return Err(ContractError::DuplicateMember { addr: m.addr });
        }
        total = total.saturating_add(m.weight);
        members.push(Member { addr, weight: m.weight, role: m.role });
    }
    if total != TOTAL_WEIGHT {
        return Err(ContractError::InvalidWeights { got: total });
    }
    Ok(members)
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = match msg.admin {
        Some(a) => deps.api.addr_validate(&a)?,
        None => info.sender.clone(),
    };
    let governance = msg.governance
        .map(|g| deps.api.addr_validate(&g))
        .transpose()?;
    let escrow_contract = deps.api.addr_validate(&msg.escrow_contract)?;
    let agent_registry = deps.api.addr_validate(&msg.agent_registry)?;
    let members = parse_members(&deps, msg.members)?;

    let task_ledger = msg.task_ledger
        .map(|tl| deps.api.addr_validate(&tl))
        .transpose()?;

    let nois_proxy = msg.nois_proxy
        .map(|np| deps.api.addr_validate(&np))
        .transpose()?;

    CONFIG.save(deps.storage, &Config {
        name: msg.name.clone(),
        admin,
        governance,
        escrow_contract,
        agent_registry,
        task_ledger,
        members,
        total_weight: TOTAL_WEIGHT,
        denom: msg.denom.unwrap_or_else(|| "ujunox".to_string()),
        voting_period_blocks: msg.voting_period_blocks.unwrap_or(100),
        quorum_percent: msg.quorum_percent.unwrap_or(51),
        adaptive_threshold_blocks: msg.adaptive_threshold_blocks.unwrap_or(10),
        adaptive_min_blocks: msg.adaptive_min_blocks.unwrap_or(13),
        verification: msg.verification.unwrap_or_default(),
        nois_proxy,
    })?;
    PROPOSAL.save(deps.storage, &None)?;
    PROPOSAL_SEQ.save(deps.storage, &0u64)?;
    SORTITION_SEQ.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("name", msg.name)
        .add_attribute("proposal_timelock_blocks", msg.proposal_timelock_blocks.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::DistributePayment { task_id } =>
            execute_distribute(deps, env, info, task_id),
        // ── New general governance ──
        ExecuteMsg::CreateProposal { kind } =>
            execute_create_proposal(deps, env, info, kind),
        ExecuteMsg::CastVote { proposal_id, vote } =>
            execute_cast_vote(deps, env, info, proposal_id, vote),
        ExecuteMsg::ExecuteProposal { proposal_id } =>
            execute_execute_proposal(deps, env, info, proposal_id),
        ExecuteMsg::ExpireProposal { proposal_id } =>
            execute_expire_proposal(deps, env, proposal_id),
        // ── Legacy ──
        ExecuteMsg::ProposeWeightChange { members } =>
            execute_propose_weight(deps, env, info, members),
        ExecuteMsg::ExecuteWeightProposal {} =>
            execute_weight_proposal(deps, env, info),
        ExecuteMsg::CancelWeightProposal {} =>
            execute_cancel_proposal(deps, env, info),
        ExecuteMsg::UpdateMembers { members } =>
            execute_update_members(deps, env, info, members),
        ExecuteMsg::TransferAdmin { new_admin } =>
            execute_transfer_admin(deps, info, new_admin),
        // ── Randomness ──
        ExecuteMsg::NoisReceive { callback } =>
            execute_nois_receive(deps, env, info, callback),
        ExecuteMsg::SubmitRandomness { job_id, randomness_hex, attestation_hash } =>
            execute_submit_randomness(deps, env, info, job_id, randomness_hex, attestation_hash),
    }
}

fn execute_distribute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_id: u64,
) -> Result<Response, ContractError> {
    if PAYMENT_HISTORY.has(deps.storage, task_id) {
        return Err(ContractError::AlreadyDistributed { task_id });
    }

    let cfg = CONFIG.load(deps.storage)?;
    let total_amount: Uint128 = info.funds.iter()
        .filter(|c| c.denom == cfg.denom)
        .map(|c| c.amount)
        .fold(Uint128::zero(), |acc, a| acc + a);

    if total_amount.is_zero() {
        return Err(ContractError::NoFunds {});
    }
    let mut msgs: Vec<BankMsg> = Vec::new();
    let mut distributed = Uint128::zero();

    for (i, member) in cfg.members.iter().enumerate() {
        let share = if i == cfg.members.len() - 1 {
            // Last member gets remainder to avoid dust loss
            total_amount - distributed
        } else {
            total_amount.multiply_ratio(member.weight as u128, TOTAL_WEIGHT as u128)
        };

        if !share.is_zero() {
            distributed += share;
            let prev = MEMBER_EARNINGS
                .may_load(deps.storage, &member.addr)?
                .unwrap_or(Uint128::zero());
            MEMBER_EARNINGS.save(deps.storage, &member.addr, &(prev + share))?;

            msgs.push(BankMsg::Send {
                to_address: member.addr.to_string(),
                amount: vec![Coin { denom: cfg.denom.clone(), amount: share }],
            });
        }
    }

    PAYMENT_HISTORY.save(deps.storage, task_id, &PaymentRecord {
        task_id,
        total_amount,
        distributed_at: env.block.height,
    })?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "distribute_payment")
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("total_amount", total_amount))
}

// ──────────────────────────────────────────────
// General governance handlers
// ──────────────────────────────────────────────

fn execute_create_proposal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    kind_msg: ProposalKindMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Only DAO members can propose
    let member = cfg.members.iter().find(|m| m.addr == info.sender)
        .ok_or(ContractError::NotMember { addr: info.sender.to_string() })?;
    if member.weight == 0 {
        return Err(ContractError::NotMember { addr: info.sender.to_string() });
    }

    // Convert msg-level ProposalKind (strings) to state-level (validated Addrs)
    let kind = match kind_msg {
        ProposalKindMsg::WeightChange { members } => {
            let parsed = parse_members(&deps, members)?;
            ProposalKind::WeightChange { members: parsed }
        }
        ProposalKindMsg::WavsPush { task_description, execution_tier, escrow_amount } => {
            if cfg.task_ledger.is_none() {
                return Err(ContractError::NoTaskLedger {});
            }
            ProposalKind::WavsPush { task_description, execution_tier, escrow_amount }
        }
        ProposalKindMsg::ConfigChange { new_admin, new_governance } => {
            ProposalKind::ConfigChange { new_admin, new_governance }
        }
        ProposalKindMsg::FreeText { title, description } => {
            ProposalKind::FreeText { title, description }
        }
        ProposalKindMsg::OutcomeCreate { question, resolution_criteria, deadline_block } => {
            ProposalKind::OutcomeCreate { question, resolution_criteria, deadline_block }
        }
        ProposalKindMsg::OutcomeResolve { market_id, outcome, attestation_hash } => {
            ProposalKind::OutcomeResolve { market_id, outcome, attestation_hash }
        }
        ProposalKindMsg::SortitionRequest { count, purpose } => {
            let pool_size = cfg.members.len() as u32;
            if count == 0 || count > pool_size {
                return Err(ContractError::SortitionCountExceedsPool { count, pool_size });
            }
            ProposalKind::SortitionRequest { count, purpose }
        }
    };

    let seq = PROPOSAL_SEQ.load(deps.storage)? + 1;
    PROPOSAL_SEQ.save(deps.storage, &seq)?;

    let deadline = env.block.height + cfg.voting_period_blocks;
    let min_deadline = env.block.height + cfg.adaptive_min_blocks;

    let proposal = Proposal {
        id: seq,
        proposer: info.sender,
        kind,
        votes: vec![],
        yes_weight: 0,
        no_weight: 0,
        abstain_weight: 0,
        total_voted_weight: 0,
        status: ProposalStatus::Open,
        created_at_block: env.block.height,
        voting_deadline_block: deadline,
        min_deadline_block: min_deadline,
        executed: false,
    };

    PROPOSALS.save(deps.storage, seq, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "create_proposal")
        .add_attribute("proposal_id", seq.to_string())
        .add_attribute("voting_deadline_block", deadline.to_string()))
}

fn execute_cast_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote_option: VoteOption,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Only members can vote
    let member = cfg.members.iter().find(|m| m.addr == info.sender)
        .ok_or(ContractError::NotMember { addr: info.sender.to_string() })?;

    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)
        .map_err(|_| ContractError::ProposalNotFound { id: proposal_id })?;

    if proposal.status != ProposalStatus::Open {
        return Err(ContractError::ProposalNotOpen { id: proposal_id });
    }

    // Check voting deadline hasn't passed
    if env.block.height > proposal.voting_deadline_block {
        return Err(ContractError::ProposalNotOpen { id: proposal_id });
    }

    // No double-voting
    if proposal.votes.iter().any(|v| v.voter == info.sender) {
        return Err(ContractError::AlreadyVoted {
            id: proposal_id,
            addr: info.sender.to_string(),
        });
    }

    // Record the vote
    let weight = member.weight;
    proposal.votes.push(Vote {
        voter: info.sender.clone(),
        option: vote_option.clone(),
        weight,
        block_height: env.block.height,
    });

    match vote_option {
        VoteOption::Yes => proposal.yes_weight += weight,
        VoteOption::No => proposal.no_weight += weight,
        VoteOption::Abstain => proposal.abstain_weight += weight,
    }
    proposal.total_voted_weight += weight;

    // ── Adaptive block reduction ──
    // If ALL members have voted within the adaptive_threshold_blocks window,
    // shrink the deadline to max(current_block + 3, min_deadline_block)
    let blocks_since_creation = env.block.height.saturating_sub(proposal.created_at_block);
    if blocks_since_creation <= cfg.adaptive_threshold_blocks
        && proposal.total_voted_weight == cfg.total_weight
    {
        let new_deadline = std::cmp::max(
            env.block.height + 3,
            proposal.min_deadline_block,
        );
        if new_deadline < proposal.voting_deadline_block {
            proposal.voting_deadline_block = new_deadline;
        }
    }

    // Check if proposal can auto-resolve (quorum met)
    let quorum_threshold = cfg.total_weight * cfg.quorum_percent / 100;
    if proposal.total_voted_weight >= quorum_threshold {
        if proposal.yes_weight > proposal.no_weight {
            proposal.status = ProposalStatus::Passed;
        } else if proposal.no_weight >= proposal.yes_weight
            && proposal.total_voted_weight == cfg.total_weight
        {
            // All votes in and no > yes (or tied) → rejected
            proposal.status = ProposalStatus::Rejected;
        }
    }

    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "cast_vote")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("voter", info.sender.to_string())
        .add_attribute("status", format!("{:?}", proposal.status))
        .add_attribute("voting_deadline_block", proposal.voting_deadline_block.to_string()))
}

fn execute_execute_proposal(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)
        .map_err(|_| ContractError::ProposalNotFound { id: proposal_id })?;

    if proposal.executed {
        return Err(ContractError::AlreadyExecuted {});
    }

    // Must be Passed status
    if proposal.status != ProposalStatus::Passed {
        return Err(ContractError::ProposalNotPassed { id: proposal_id });
    }

    // Must be past the voting deadline
    if env.block.height < proposal.voting_deadline_block {
        return Err(ContractError::VotingNotEnded {
            deadline: proposal.voting_deadline_block,
            current: env.block.height,
        });
    }

    let mut cfg = CONFIG.load(deps.storage)?;
    let mut response = Response::new()
        .add_attribute("action", "execute_proposal")
        .add_attribute("proposal_id", proposal_id.to_string());

    match &proposal.kind {
        ProposalKind::WeightChange { members } => {
            cfg.members = members.clone();
            CONFIG.save(deps.storage, &cfg)?;
            response = response.add_attribute("kind", "weight_change");
        }
        ProposalKind::WavsPush { task_description, execution_tier, escrow_amount } => {
            let task_ledger = cfg.task_ledger.as_ref()
                .ok_or(ContractError::NoTaskLedger {})?;

            // Simple hash of task description for input_hash
            let input_hash = format!("{:016x}", {
                let bytes = task_description.as_bytes();
                let mut h: u64 = 0xcbf29ce484222325; // FNV-1a
                for &b in bytes {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
                h
            });

            // Cross-contract call: task-ledger SubmitTask
            #[derive(serde::Serialize)]
            #[serde(rename_all = "snake_case")]
            enum TaskLedgerMsg {
                SubmitTask {
                    agent_id: u64,
                    input_hash: String,
                    execution_tier: junoclaw_common::ExecutionTier,
                },
            }

            let submit_msg = WasmMsg::Execute {
                contract_addr: task_ledger.to_string(),
                msg: to_json_binary(&TaskLedgerMsg::SubmitTask {
                    agent_id: 0u64,
                    input_hash,
                    execution_tier: execution_tier.clone(),
                })?,
                funds: vec![],
            };
            response = response.add_message(submit_msg);

            // If escrow_amount > 0, record a payment obligation (non-custodial)
            if !escrow_amount.is_zero() {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum PaymentLedgerMsg {
                    Authorize {
                        task_id: u64,
                        payee: String,
                        amount: Uint128,
                    },
                }

                let authorize_msg = WasmMsg::Execute {
                    contract_addr: cfg.escrow_contract.to_string(),
                    msg: to_json_binary(&PaymentLedgerMsg::Authorize {
                        task_id: proposal_id,
                        payee: cfg.admin.to_string(),
                        amount: *escrow_amount,
                    })?,
                    funds: vec![],
                };
                response = response.add_message(authorize_msg);
            }

            response = response
                .add_attribute("kind", "wavs_push")
                .add_attribute("task_description", task_description);
        }
        ProposalKind::ConfigChange { new_admin, new_governance } => {
            if let Some(admin_str) = new_admin {
                cfg.admin = deps.api.addr_validate(admin_str)?;
            }
            if let Some(gov_str) = new_governance {
                cfg.governance = Some(deps.api.addr_validate(gov_str)?);
            }
            CONFIG.save(deps.storage, &cfg)?;
            response = response.add_attribute("kind", "config_change");
        }
        ProposalKind::FreeText { title, .. } => {
            // Signal vote — no on-chain side-effects
            response = response
                .add_attribute("kind", "free_text")
                .add_attribute("title", title);
        }
        ProposalKind::OutcomeCreate { question, deadline_block, .. } => {
            // Outcome market creation is recorded on-chain via proposal execution
            response = response
                .add_attribute("kind", "outcome_create")
                .add_attribute("question", question)
                .add_attribute("deadline_block", deadline_block.to_string());
        }
        ProposalKind::OutcomeResolve { market_id, outcome, attestation_hash } => {
            // Outcome market resolution requires WAVS attestation — recorded on-chain
            response = response
                .add_attribute("kind", "outcome_resolve")
                .add_attribute("market_id", market_id.to_string())
                .add_attribute("outcome", outcome.to_string())
                .add_attribute("attestation_hash", attestation_hash);
        }
        ProposalKind::SortitionRequest { count, purpose } => {
            let job_id = format!("sortition_{}_{}", proposal_id, env.block.height);
            let eligible: Vec<_> = cfg.members.iter().map(|m| m.addr.clone()).collect();
            let pool_size = eligible.len() as u32;

            if *count == 0 || *count > pool_size {
                return Err(ContractError::SortitionCountExceedsPool {
                    count: *count,
                    pool_size,
                });
            }

            PENDING_SORTITION.save(deps.storage, &job_id, &PendingSortition {
                proposal_id,
                count: *count,
                purpose: purpose.clone(),
                eligible,
            })?;

            // If NOIS proxy configured, send randomness request
            if let Some(nois_proxy) = &cfg.nois_proxy {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum NoisProxyMsg {
                    GetNextRandomness { job_id: String },
                }
                let nois_msg = WasmMsg::Execute {
                    contract_addr: nois_proxy.to_string(),
                    msg: to_json_binary(&NoisProxyMsg::GetNextRandomness {
                        job_id: job_id.clone(),
                    })?,
                    funds: vec![],
                };
                response = response.add_message(nois_msg);
            }

            response = response
                .add_attribute("kind", "sortition_request")
                .add_attribute("job_id", &job_id)
                .add_attribute("count", count.to_string())
                .add_attribute("purpose", purpose)
                .add_attribute("awaiting_randomness", if cfg.nois_proxy.is_some() { "nois" } else { "wavs_drand" });
        }
    }

    proposal.executed = true;
    proposal.status = ProposalStatus::Executed;
    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(response)
}

fn execute_expire_proposal(
    deps: DepsMut,
    env: Env,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)
        .map_err(|_| ContractError::ProposalNotFound { id: proposal_id })?;

    if proposal.status != ProposalStatus::Open {
        return Err(ContractError::ProposalNotOpen { id: proposal_id });
    }

    if env.block.height <= proposal.voting_deadline_block {
        return Err(ContractError::VotingNotEnded {
            deadline: proposal.voting_deadline_block,
            current: env.block.height,
        });
    }

    proposal.status = ProposalStatus::Expired;
    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "expire_proposal")
        .add_attribute("proposal_id", proposal_id.to_string()))
}

// ──────────────────────────────────────────────
// Randomness & Sortition
// ──────────────────────────────────────────────

/// Derive sub-randomness from a 32-byte seed using SHA-256(seed || counter)
fn sub_randomness(seed: &[u8; 32], counter: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update(counter.to_le_bytes());
    hasher.finalize().into()
}

/// Deterministic Fisher-Yates selection of `count` members from `eligible` using 32-byte randomness
fn select_members(
    randomness: &[u8; 32],
    eligible: &[cosmwasm_std::Addr],
    count: usize,
) -> Vec<cosmwasm_std::Addr> {
    let n = eligible.len();
    if count >= n {
        return eligible.to_vec();
    }
    let mut indices: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let sub = sub_randomness(randomness, i as u64);
        let j = u64::from_le_bytes(sub[0..8].try_into().unwrap()) as usize % (i + 1);
        indices.swap(i, j);
    }
    indices.truncate(count);
    indices.iter().map(|&i| eligible[i].clone()).collect()
}

/// Parse a hex string into a 32-byte array
fn parse_randomness_hex(hex: &str) -> Result<[u8; 32], ContractError> {
    if hex.len() != 64 {
        return Err(ContractError::InvalidRandomness { len: hex.len() });
    }
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| ContractError::InvalidRandomness { len: hex.len() })?;
    }
    Ok(bytes)
}

/// Resolve a pending sortition with the provided randomness
fn resolve_sortition(
    deps: DepsMut,
    env: &Env,
    job_id: &str,
    randomness_hex: &str,
    source: &str,
) -> Result<Response, ContractError> {
    let pending = PENDING_SORTITION.may_load(deps.storage, job_id)?
        .ok_or(ContractError::NoPendingSortition { job_id: job_id.to_string() })?;

    let randomness = parse_randomness_hex(randomness_hex)?;
    let selected = select_members(&randomness, &pending.eligible, pending.count as usize);

    let seq = SORTITION_SEQ.load(deps.storage)? + 1;
    SORTITION_SEQ.save(deps.storage, &seq)?;

    let round = SortitionRound {
        id: seq,
        proposal_id: pending.proposal_id,
        purpose: pending.purpose.clone(),
        selected: selected.clone(),
        pool_size: pending.eligible.len() as u32,
        randomness_source: source.to_string(),
        randomness_hex: randomness_hex.to_string(),
        created_at_block: env.block.height,
    };
    SORTITION_ROUNDS.save(deps.storage, seq, &round)?;
    PENDING_SORTITION.remove(deps.storage, job_id);

    let selected_addrs: Vec<String> = selected.iter().map(|a| a.to_string()).collect();

    Ok(Response::new()
        .add_attribute("action", "sortition_resolved")
        .add_attribute("round_id", seq.to_string())
        .add_attribute("job_id", job_id)
        .add_attribute("purpose", &pending.purpose)
        .add_attribute("count", pending.count.to_string())
        .add_attribute("pool_size", pending.eligible.len().to_string())
        .add_attribute("selected", selected_addrs.join(","))
        .add_attribute("source", source))
}

fn execute_nois_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    callback: NoisCallback,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Only the configured NOIS proxy can call this
    match &cfg.nois_proxy {
        Some(proxy) if *proxy == info.sender => {}
        _ => return Err(ContractError::UnauthorizedRandomness {}),
    }

    resolve_sortition(deps, &env, &callback.job_id, &callback.randomness, &format!("nois:{}", callback.job_id))
}

fn execute_submit_randomness(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: String,
    randomness_hex: String,
    attestation_hash: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Authorized submitters: admin, governance, or task_ledger (WAVS bridge)
    let is_admin = info.sender == cfg.admin;
    let is_governance = cfg.governance.as_ref().map(|g| *g == info.sender).unwrap_or(false);
    let is_wavs_bridge = cfg.task_ledger.as_ref().map(|tl| *tl == info.sender).unwrap_or(false);
    if !is_admin && !is_governance && !is_wavs_bridge {
        return Err(ContractError::UnauthorizedRandomness {});
    }

    let mut response = resolve_sortition(deps, &env, &job_id, &randomness_hex, &format!("wavs_drand:{}", job_id))?;
    response = response.add_attribute("attestation_hash", attestation_hash);
    Ok(response)
}

// ──────────────────────────────────────────────
// Legacy governance handlers
// ──────────────────────────────────────────────

fn execute_propose_weight(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    member_inputs: Vec<MemberInput>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let is_admin = info.sender == cfg.admin;
    let is_governance = cfg.governance.as_ref().map(|g| *g == info.sender).unwrap_or(false);
    if !is_admin && !is_governance {
        return Err(ContractError::Unauthorized {});
    }

    let proposed_members = parse_members(&deps, member_inputs)?;

    // Timelock: default 50 blocks (~5 min on Juno). Could be stored in config.
    let executable_after = env.block.height + 50;

    let seq = PROPOSAL_SEQ.load(deps.storage)? + 1;
    PROPOSAL_SEQ.save(deps.storage, &seq)?;

    PROPOSAL.save(deps.storage, &Some(WeightProposal {
        id: seq,
        proposer: info.sender,
        proposed_members,
        executable_after,
        executed: false,
    }))?;

    Ok(Response::new()
        .add_attribute("action", "propose_weight_change")
        .add_attribute("proposal_id", seq.to_string())
        .add_attribute("executable_after", executable_after.to_string()))
}

fn execute_weight_proposal(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let proposal = PROPOSAL.load(deps.storage)?
        .ok_or(ContractError::NoProposal {})?;

    if proposal.executed {
        return Err(ContractError::AlreadyExecuted {});
    }
    if env.block.height < proposal.executable_after {
        return Err(ContractError::TimelockNotElapsed {
            executable_after: proposal.executable_after,
            current: env.block.height,
        });
    }

    let mut cfg = CONFIG.load(deps.storage)?;
    cfg.members = proposal.proposed_members.clone();
    CONFIG.save(deps.storage, &cfg)?;

    let mut executed_proposal = proposal;
    executed_proposal.executed = true;
    PROPOSAL.save(deps.storage, &Some(executed_proposal))?;

    Ok(Response::new()
        .add_attribute("action", "execute_weight_proposal")
        .add_attribute("new_member_count", cfg.members.len().to_string()))
}

fn execute_cancel_proposal(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let is_admin = info.sender == cfg.admin;
    let is_governance = cfg.governance.as_ref().map(|g| *g == info.sender).unwrap_or(false);
    if !is_admin && !is_governance {
        return Err(ContractError::Unauthorized {});
    }

    PROPOSAL.load(deps.storage)?.ok_or(ContractError::NoProposal {})?;
    PROPOSAL.save(deps.storage, &None)?;

    Ok(Response::new().add_attribute("action", "cancel_weight_proposal"))
}

fn execute_update_members(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    member_inputs: Vec<MemberInput>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    // Direct update only allowed when no governance contract is set
    if cfg.governance.is_some() {
        return Err(ContractError::Unauthorized {});
    }
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    cfg.members = parse_members(&deps, member_inputs)?;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "update_members")
        .add_attribute("count", cfg.members.len().to_string()))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    cfg.admin = deps.api.addr_validate(&new_admin)?;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", new_admin))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetMembers {} => to_json_binary(&CONFIG.load(deps.storage)?.members),
        // ── New proposal queries ──
        QueryMsg::GetProposal { proposal_id } => {
            to_json_binary(&PROPOSALS.load(deps.storage, proposal_id)?)
        }
        QueryMsg::ListProposals { start_after, limit } => {
            let limit = limit.unwrap_or(30).min(50) as usize;
            let start = start_after.map(Bound::exclusive);
            let proposals: Vec<Proposal> = PROPOSALS
                .range(deps.storage, start, None, Order::Descending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, p)| p))
                .collect();
            to_json_binary(&proposals)
        }
        // ── Legacy ──
        QueryMsg::GetPendingProposal {} => to_json_binary(&PROPOSAL.load(deps.storage)?),
        QueryMsg::GetPaymentRecord { task_id } =>
            to_json_binary(&PAYMENT_HISTORY.load(deps.storage, task_id)?),
        QueryMsg::GetPaymentHistory { start_after, limit } => {
            let limit = limit.unwrap_or(30) as usize;
            let start = start_after.map(Bound::exclusive);
            let records: Vec<PaymentRecord> = PAYMENT_HISTORY
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .map(|r| r.map(|(_, v)| v))
                .collect::<StdResult<Vec<_>>>()?;
            to_json_binary(&records)
        }
        QueryMsg::GetMemberEarnings { addr } => {
            let addr = deps.api.addr_validate(&addr)?;
            let amount = MEMBER_EARNINGS
                .may_load(deps.storage, &addr)?
                .unwrap_or(Uint128::zero());
            to_json_binary(&amount)
        }
        // ── Sortition ──
        QueryMsg::GetSortitionRound { round_id } => {
            to_json_binary(&SORTITION_ROUNDS.load(deps.storage, round_id)?)
        }
        QueryMsg::ListSortitionRounds { start_after, limit } => {
            let limit = limit.unwrap_or(30).min(50) as usize;
            let start = start_after.map(Bound::exclusive);
            let rounds: Vec<SortitionRound> = SORTITION_ROUNDS
                .range(deps.storage, start, None, Order::Descending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, sr)| sr))
                .collect();
            to_json_binary(&rounds)
        }
        QueryMsg::GetPendingSortition { job_id } => {
            to_json_binary(&PENDING_SORTITION.may_load(deps.storage, &job_id)?)
        }
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "migrate"))
}
