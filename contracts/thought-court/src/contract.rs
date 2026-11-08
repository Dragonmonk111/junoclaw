use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, Order, Response, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    AttestationListResponse, CommitteeResponse, ExecuteMsg, InstantiateMsg, MigrateMsg,
    QueryMsg, VerifierListResponse,
};
use crate::state::{
    Attestation, AttestationStatus, Config, Dispute, Verdict, Verifier, Vote, ACTIVE_SEATS,
    ATTESTATIONS, CONFIG, DISPUTES, VERIFIERS,
};

const CONTRACT_NAME: &str = "crates.io:junoclaw-thought-court";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_DENOM: &str = "ujclaw";
const DEFAULT_CHALLENGE_WINDOW_SECS: u64 = 86_400;
const DEFAULT_VERDICT_WINDOW_SECS: u64 = 43_200;
const DEFAULT_COMMITTEE_SIZE: u32 = 5;
const DEFAULT_QUORUM_BP: u32 = 6_700;
const DEFAULT_CHALLENGER_REWARD_BP: u32 = 5_000;
const DEFAULT_OPERATOR_REWARD_BP: u32 = 5_000;
const DEFAULT_VERIFIER_SLASH_BP: u32 = 2_000;

const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = match msg.admin {
        Some(a) => deps.api.addr_validate(&a)?,
        None => info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(
        deps.storage,
        &Config {
            admin,
            denom: msg.denom.unwrap_or_else(|| DEFAULT_DENOM.into()),
            min_operator_bond: msg.min_operator_bond.unwrap_or_else(|| Uint128::new(1_000_000)),
            min_challenger_bond: msg
                .min_challenger_bond
                .unwrap_or_else(|| Uint128::new(500_000)),
            min_verifier_bond: msg.min_verifier_bond.unwrap_or_else(|| Uint128::new(2_000_000)),
            challenge_window_secs: msg
                .challenge_window_secs
                .unwrap_or(DEFAULT_CHALLENGE_WINDOW_SECS),
            verdict_window_secs: msg
                .verdict_window_secs
                .unwrap_or(DEFAULT_VERDICT_WINDOW_SECS),
            committee_size: msg.committee_size.unwrap_or(DEFAULT_COMMITTEE_SIZE),
            quorum_bp: msg.quorum_bp.unwrap_or(DEFAULT_QUORUM_BP),
            challenger_reward_bp: msg
                .challenger_reward_bp
                .unwrap_or(DEFAULT_CHALLENGER_REWARD_BP),
            operator_reward_bp: msg
                .operator_reward_bp
                .unwrap_or(DEFAULT_OPERATOR_REWARD_BP),
            verifier_slash_bp: msg
                .verifier_slash_bp
                .unwrap_or(DEFAULT_VERIFIER_SLASH_BP),
        },
    )?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

/// Exact-denom payment check: exactly one coin of `cfg.denom` with
/// amount >= `min`. Returns the paid amount.
fn take_bond(info: &MessageInfo, cfg: &Config, min: Uint128) -> Result<Uint128, ContractError> {
    let sent: Vec<String> = info
        .funds
        .iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .collect();
    let amount = info
        .funds
        .iter()
        .find(|c| c.denom == cfg.denom)
        .map(|c| c.amount)
        .unwrap_or_default();
    if info.funds.len() != 1 || amount < min {
        return Err(ContractError::WrongFunds {
            denom: cfg.denom.clone(),
            expected: min,
            sent,
        });
    }
    Ok(amount)
}

fn send(denom: &str, to: &Addr, amount: Uint128) -> Option<CosmosMsg> {
    if amount.is_zero() {
        return None;
    }
    Some(BankMsg::Send {
        to_address: to.to_string(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount,
        }],
    }
    .into())
}

/// Fiat-Shamir sortition: rank every registered verifier by
/// sha256(seed || addr) and seat the `k` smallest digests. Anyone can
/// re-derive the committee from the public seed — no trusted draw.
fn sortition(
    storage: &dyn Storage,
    seed: &str,
    k: u32,
) -> StdResult<Vec<Addr>> {
    let mut ranked: Vec<(Vec<u8>, Addr)> = VERIFIERS
        .range(storage, None, None, Order::Ascending)
        .map(|r| {
            r.map(|(a, _)| {
                let mut h = Sha256::new();
                h.update(seed.as_bytes());
                h.update(a.as_bytes());
                (h.finalize().to_vec(), Addr::unchecked(a))
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    ranked.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(ranked.into_iter().take(k as usize).map(|(_, a)| a).collect())
}

fn seat_bump(storage: &mut dyn Storage, addr: &Addr, delta: i64) -> StdResult<()> {
    let cur = ACTIVE_SEATS
        .may_load(storage, addr.as_str())?
        .unwrap_or_default();
    let next = if delta < 0 {
        cur.saturating_sub((-delta) as u64)
    } else {
        cur + delta as u64
    };
    if next == 0 {
        ACTIVE_SEATS.remove(storage, addr.as_str());
    } else {
        ACTIVE_SEATS.save(storage, addr.as_str(), &next)?;
    }
    Ok(())
}

/// Votes needed for a verdict: ceil(quorum_bp * committee_size / 10000).
fn quorum_needed(cfg: &Config) -> u32 {
    (cfg.quorum_bp * cfg.committee_size).div_ceil(10_000)
}

/// Slash minority voters' registration bonds to the DAO; returns the
/// payout messages.
fn slash_minority(
    deps: &mut DepsMut,
    cfg: &Config,
    dispute: &Dispute,
    winning: Verdict,
    msgs: &mut Vec<CosmosMsg>,
) -> Result<(), ContractError> {
    for vote in &dispute.votes {
        if vote.verdict == winning {
            continue;
        }
        if let Some(mut v) = VERIFIERS.may_load(deps.storage, vote.verifier.as_str())? {
            let slash = v.bond * Uint128::new(cfg.verifier_slash_bp as u128)
                / Uint128::new(10_000u128);
            if !slash.is_zero() {
                v.bond -= slash;
                VERIFIERS.save(deps.storage, vote.verifier.as_str(), &v)?;
                if let Some(m) = send(&cfg.denom, &cfg.admin, slash) {
                    msgs.push(m);
                }
            }
        }
    }
    Ok(())
}

/// Terminal resolution shared by quorum verdicts and DAO force-resolve.
/// `challenger_bond_outstanding` is false when ResolveTimeout already
/// refunded it.
fn resolve(
    deps: &mut DepsMut,
    cfg: &Config,
    attestation: &mut Attestation,
    dispute: &Dispute,
    verdict: Verdict,
    challenger_bond_outstanding: bool,
) -> Result<Response, ContractError> {
    let mut msgs: Vec<CosmosMsg> = vec![];
    let admin = cfg.admin.clone();

    match verdict {
        Verdict::Fraud => {
            attestation.status = AttestationStatus::Fraudulent;
            if challenger_bond_outstanding {
                if let Some(m) = send(&cfg.denom, &dispute.challenger, dispute.challenger_bond)
                {
                    msgs.push(m);
                }
            }
            let reward = attestation.bond * Uint128::new(cfg.challenger_reward_bp as u128)
                / Uint128::new(10_000u128);
            let rest = attestation.bond - reward;
            if let Some(m) = send(&cfg.denom, &dispute.challenger, reward) {
                msgs.push(m);
            }
            if let Some(m) = send(&cfg.denom, &admin, rest) {
                msgs.push(m);
            }
            slash_minority(deps, cfg, dispute, Verdict::Fraud, &mut msgs)?;
        }
        Verdict::Clean => {
            attestation.status = AttestationStatus::Finalized;
            let reward = dispute.challenger_bond
                * Uint128::new(cfg.operator_reward_bp as u128)
                / Uint128::new(10_000u128);
            let rest = dispute.challenger_bond - reward;
            if challenger_bond_outstanding {
                if let Some(m) = send(&cfg.denom, &attestation.operator, attestation.bond + reward)
                {
                    msgs.push(m);
                }
                if let Some(m) = send(&cfg.denom, &admin, rest) {
                    msgs.push(m);
                }
            } else if let Some(m) = send(&cfg.denom, &attestation.operator, attestation.bond) {
                msgs.push(m);
            }
            slash_minority(deps, cfg, dispute, Verdict::Clean, &mut msgs)?;
        }
    }

    for seat in &dispute.committee {
        seat_bump(deps.storage, seat, -1)?;
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "resolve")
        .add_attribute(
            "verdict",
            match verdict {
                Verdict::Fraud => "fraud",
                Verdict::Clean => "clean",
            },
        )
        .add_attribute("attestation_id", attestation.id.clone()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterVerifier {} => register_verifier(deps, env, info),
        ExecuteMsg::RemoveVerifier {} => remove_verifier(deps, info),
        ExecuteMsg::AnchorAttestation { id, activation_root } => {
            anchor(deps, env, info, id, activation_root)
        }
        ExecuteMsg::OpenChallenge { attestation_id } => open_challenge(deps, env, info, attestation_id),
        ExecuteMsg::SubmitVerdict {
            attestation_id,
            verdict,
            evidence_hash,
        } => submit_verdict(deps, env, info, attestation_id, verdict, evidence_hash),
        ExecuteMsg::FinalizeAttestation { attestation_id } => {
            finalize(deps, env, info, attestation_id)
        }
        ExecuteMsg::ResolveTimeout { attestation_id } => {
            resolve_timeout(deps, env, info, attestation_id)
        }
        ExecuteMsg::ForceResolve {
            attestation_id,
            verdict,
        } => force_resolve(deps, info, attestation_id, verdict),
        ExecuteMsg::UpdateConfig {
            admin,
            min_operator_bond,
            min_challenger_bond,
            min_verifier_bond,
            challenge_window_secs,
            verdict_window_secs,
            committee_size,
            quorum_bp,
            challenger_reward_bp,
            operator_reward_bp,
            verifier_slash_bp,
        } => update_config(
            deps,
            info,
            admin,
            min_operator_bond,
            min_challenger_bond,
            min_verifier_bond,
            challenge_window_secs,
            verdict_window_secs,
            committee_size,
            quorum_bp,
            challenger_reward_bp,
            operator_reward_bp,
            verifier_slash_bp,
        ),
    }
}

fn register_verifier(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if VERIFIERS.has(deps.storage, info.sender.as_str()) {
        return Err(ContractError::VerifierExists {});
    }
    let bond = take_bond(&info, &cfg, cfg.min_verifier_bond)?;
    VERIFIERS.save(
        deps.storage,
        info.sender.as_str(),
        &Verifier {
            addr: info.sender.clone(),
            bond,
            registered_at: env.block.time.seconds(),
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "register_verifier")
        .add_attribute("verifier", info.sender)
        .add_attribute("bond", bond))
}

fn remove_verifier(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let v = VERIFIERS
        .may_load(deps.storage, info.sender.as_str())?
        .ok_or(ContractError::VerifierNotFound {})?;
    let busy = ACTIVE_SEATS
        .may_load(deps.storage, info.sender.as_str())?
        .unwrap_or_default();
    if busy > 0 {
        return Err(ContractError::VerifierBusy {});
    }
    VERIFIERS.remove(deps.storage, info.sender.as_str());
    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: cfg.denom,
                amount: v.bond,
            }],
        })
        .add_attribute("action", "remove_verifier")
        .add_attribute("verifier", info.sender)
        .add_attribute("returned", v.bond))
}

fn anchor(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
    activation_root: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if ATTESTATIONS.has(deps.storage, &id) {
        return Err(ContractError::AttestationExists { id });
    }
    let bond = take_bond(&info, &cfg, cfg.min_operator_bond)?;
    let now = env.block.time.seconds();
    ATTESTATIONS.save(
        deps.storage,
        &id,
        &Attestation {
            id: id.clone(),
            operator: info.sender.clone(),
            activation_root,
            bond,
            anchored_at: now,
            challenge_deadline: now + cfg.challenge_window_secs,
            status: AttestationStatus::Pending,
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "anchor_attestation")
        .add_attribute("id", id)
        .add_attribute("operator", info.sender)
        .add_attribute("bond", bond))
}

fn open_challenge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    attestation_id: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    if att.status != AttestationStatus::Pending {
        return Err(ContractError::AttestationNotPending {
            id: attestation_id,
        });
    }
    if env.block.time.seconds() >= att.challenge_deadline {
        return Err(ContractError::ChallengeWindowClosed {
            id: attestation_id,
        });
    }
    let bond = take_bond(&info, &cfg, cfg.min_challenger_bond)?;

    // Dispute seed: Fiat-Shamir over public, uncontrollable-by-one-party
    // material — attestation id + challenger + anchor block height.
    let seed = format!(
        "{}:{}:{}",
        attestation_id,
        info.sender,
        env.block.height
    );
    let n_verifiers = VERIFIERS
        .range(deps.storage, None, None, Order::Ascending)
        .count() as u32;
    if n_verifiers < cfg.committee_size {
        return Err(ContractError::InsufficientVerifiers {
            need: cfg.committee_size,
            have: n_verifiers,
        });
    }
    let committee = sortition(deps.storage, &seed, cfg.committee_size)?;
    for seat in &committee {
        seat_bump(deps.storage, seat, 1)?;
    }

    let now = env.block.time.seconds();
    DISPUTES.save(
        deps.storage,
        &attestation_id,
        &Dispute {
            attestation_id: attestation_id.clone(),
            challenger: info.sender.clone(),
            challenger_bond: bond,
            opened_at: now,
            committee,
            votes: vec![],
            verdict_deadline: now + cfg.verdict_window_secs,
        },
    )?;
    att.status = AttestationStatus::Challenged;
    ATTESTATIONS.save(deps.storage, &attestation_id, &att)?;

    Ok(Response::new()
        .add_attribute("action", "open_challenge")
        .add_attribute("attestation_id", attestation_id)
        .add_attribute("challenger", info.sender)
        .add_attribute("seed", seed))
}

fn submit_verdict(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    attestation_id: String,
    verdict: Verdict,
    evidence_hash: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    if att.status != AttestationStatus::Challenged {
        return Err(ContractError::AttestationNotPending {
            id: attestation_id,
        });
    }
    let mut dispute = DISPUTES
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::DisputeNotFound {
            id: attestation_id.clone(),
        })?;
    if env.block.time.seconds() >= dispute.verdict_deadline {
        return Err(ContractError::VerdictWindowClosed {});
    }
    if !dispute.committee.contains(&info.sender) {
        return Err(ContractError::NotCommittee {});
    }
    if dispute.votes.iter().any(|v| v.verifier == info.sender) {
        return Err(ContractError::AlreadyVoted {});
    }
    dispute.votes.push(Vote {
        verifier: info.sender.clone(),
        verdict,
        evidence_hash,
        voted_at: env.block.time.seconds(),
    });

    let needed = quorum_needed(&cfg) as usize;
    let clean = dispute
        .votes
        .iter()
        .filter(|v| v.verdict == Verdict::Clean)
        .count();
    let fraud = dispute.votes.len() - clean;
    // Early resolution at the quorum supermajority; once every committee
    // member has voted, a simple majority decides. An exact tie stays
    // open until the verdict deadline -> Unresolved -> DAO fallback.
    let decided = if fraud >= needed {
        Some(Verdict::Fraud)
    } else if clean >= needed {
        Some(Verdict::Clean)
    } else if dispute.votes.len() == dispute.committee.len() {
        match fraud.cmp(&clean) {
            std::cmp::Ordering::Greater => Some(Verdict::Fraud),
            std::cmp::Ordering::Less => Some(Verdict::Clean),
            std::cmp::Ordering::Equal => None,
        }
    } else {
        None
    };

    if let Some(v) = decided {
        let resp = resolve(&mut deps, &cfg, &mut att, &dispute, v, true)?;
        ATTESTATIONS.save(deps.storage, &attestation_id, &att)?;
        DISPUTES.save(deps.storage, &attestation_id, &dispute)?;
        Ok(resp)
    } else {
        DISPUTES.save(deps.storage, &attestation_id, &dispute)?;
        Ok(Response::new()
            .add_attribute("action", "submit_verdict")
            .add_attribute("attestation_id", attestation_id)
            .add_attribute("verifier", info.sender)
            .add_attribute("votes", dispute.votes.len().to_string())
            .add_attribute("needed", needed.to_string()))
    }
}

fn finalize(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    attestation_id: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    if att.status != AttestationStatus::Pending {
        return Err(ContractError::AttestationNotPending {
            id: attestation_id,
        });
    }
    if env.block.time.seconds() < att.challenge_deadline {
        return Err(ContractError::ChallengeWindowOpen { id: att.id });
    }
    att.status = AttestationStatus::Finalized;
    ATTESTATIONS.save(deps.storage, &attestation_id, &att)?;
    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: att.operator.to_string(),
            amount: vec![Coin {
                denom: cfg.denom,
                amount: att.bond,
            }],
        })
        .add_attribute("action", "finalize_attestation")
        .add_attribute("id", attestation_id))
}

fn resolve_timeout(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    attestation_id: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    if att.status != AttestationStatus::Challenged {
        return Err(ContractError::AttestationNotPending {
            id: attestation_id,
        });
    }
    let dispute = DISPUTES
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::DisputeNotFound {
            id: attestation_id.clone(),
        })?;
    if env.block.time.seconds() < dispute.verdict_deadline {
        return Err(ContractError::VerdictWindowOpen {});
    }
    // Committee stalled: refund the challenger, lock the operator bond,
    // hand the decision to the DAO (ForceResolve).
    att.status = AttestationStatus::Unresolved;
    ATTESTATIONS.save(deps.storage, &attestation_id, &att)?;
    for seat in &dispute.committee {
        seat_bump(deps.storage, seat, -1)?;
    }
    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: dispute.challenger.to_string(),
            amount: vec![Coin {
                denom: cfg.denom,
                amount: dispute.challenger_bond,
            }],
        })
        .add_attribute("action", "resolve_timeout")
        .add_attribute("id", attestation_id))
}

fn force_resolve(
    deps: DepsMut,
    info: MessageInfo,
    attestation_id: String,
    verdict: Verdict,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    let mut att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    if att.status != AttestationStatus::Unresolved {
        return Err(ContractError::DisputeNotUnresolved {});
    }
    let dispute = DISPUTES
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::DisputeNotFound {
            id: attestation_id.clone(),
        })?;
    let mut d = deps;
    let resp = resolve(&mut d, &cfg, &mut att, &dispute, verdict, false)?;
    ATTESTATIONS.save(d.storage, &attestation_id, &att)?;
    Ok(resp)
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    min_operator_bond: Option<Uint128>,
    min_challenger_bond: Option<Uint128>,
    min_verifier_bond: Option<Uint128>,
    challenge_window_secs: Option<u64>,
    verdict_window_secs: Option<u64>,
    committee_size: Option<u32>,
    quorum_bp: Option<u32>,
    challenger_reward_bp: Option<u32>,
    operator_reward_bp: Option<u32>,
    verifier_slash_bp: Option<u32>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(a) = admin {
        cfg.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(v) = min_operator_bond {
        cfg.min_operator_bond = v;
    }
    if let Some(v) = min_challenger_bond {
        cfg.min_challenger_bond = v;
    }
    if let Some(v) = min_verifier_bond {
        cfg.min_verifier_bond = v;
    }
    if let Some(v) = challenge_window_secs {
        cfg.challenge_window_secs = v;
    }
    if let Some(v) = verdict_window_secs {
        cfg.verdict_window_secs = v;
    }
    if let Some(v) = committee_size {
        cfg.committee_size = v;
    }
    if let Some(v) = quorum_bp {
        cfg.quorum_bp = v;
    }
    if let Some(v) = challenger_reward_bp {
        cfg.challenger_reward_bp = v;
    }
    if let Some(v) = operator_reward_bp {
        cfg.operator_reward_bp = v;
    }
    if let Some(v) = verifier_slash_bp {
        cfg.verifier_slash_bp = v;
    }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetAttestation { attestation_id } => {
            to_json_binary(&ATTESTATIONS.load(deps.storage, &attestation_id)?)
        }
        QueryMsg::GetDispute { attestation_id } => {
            to_json_binary(&DISPUTES.load(deps.storage, &attestation_id)?)
        }
        QueryMsg::GetVerifier { addr } => {
            let a = deps.api.addr_validate(&addr)?;
            to_json_binary(&VERIFIERS.load(deps.storage, a.as_str())?)
        }
        QueryMsg::ListVerifiers { start_after, limit } => {
            let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
            let verifiers: Vec<Verifier> = VERIFIERS
                .range(deps.storage, None, None, Order::Ascending)
                .filter_map(|r| r.ok())
                .map(|(_, v)| v)
                .skip_while(|v| {
                    start_after
                        .as_ref()
                        .is_some_and(|s| v.addr.as_str() == s.as_str())
                })
                .skip(if start_after.is_some() { 1 } else { 0 })
                .take(limit)
                .collect();
            to_json_binary(&VerifierListResponse { verifiers })
        }
        QueryMsg::ListAttestations {
            status,
            start_after,
            limit,
        } => {
            let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
            let attestations: Vec<Attestation> = ATTESTATIONS
                .range(deps.storage, None, None, Order::Ascending)
                .filter_map(|r| r.ok())
                .filter(|(_, a)| status.as_ref().map_or(true, |s| &a.status == s))
                .skip_while(|(id, _)| start_after.as_ref() == Some(id))
                .skip(if start_after.is_some() { 1 } else { 0 })
                .take(limit)
                .map(|(_, a)| a)
                .collect();
            to_json_binary(&AttestationListResponse { attestations })
        }
        QueryMsg::CommitteePreview { seed } => {
            let cfg = CONFIG.load(deps.storage)?;
            let committee = sortition(deps.storage, &seed, cfg.committee_size)?
                .into_iter()
                .map(|a| a.to_string())
                .collect();
            to_json_binary(&CommitteeResponse { committee })
        }
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}
