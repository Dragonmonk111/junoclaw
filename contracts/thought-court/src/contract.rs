use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, Order, Response, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    AttestationListResponse, CommitteeResponse, ExecuteMsg, InstantiateMsg, MigrateMsg,
    QueryMsg, VerifierListResponse, ZkVerifierQueryMsg, ZkVerifierResponse, ZkVerifyResponse,
};
use crate::state::{
    Attestation, AttestationStatus, Config, Dispute, ReadoutClaims, Verdict, Verifier, Vote,
    ACTIVE_SEATS, ATTESTATIONS, CONFIG, DISPUTES, VERIFIERS, ZK_VERIFIER,
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

/// Fixed-point scale of the jlens.integrity.v1 readout — must match the
/// circuit's SCALE and the J-Lens evidence spec.
const READOUT_SCALE: u64 = 1_000_000;

/// Public inputs are arkworks-compressed BN254 Fr elements, 32 bytes each.
const FR_BYTES: usize = 32;
/// Circuit allocation order: [merkle_root, bank_commitment, score,
/// integrity, detection] — see circuits/jlens-readout/src/lib.rs.
const ZK_PUBLIC_INPUTS: usize = 5;

// ── Drand quicknet sortition (unchained G1, RFC-9380) ──
// Same beacon the agent-company sortition used via WAVS/Nois — here the
// signature is verified on-chain, so the draw needs no trusted relay.
const DRAND_GENESIS_TIME: u64 = 1_692_803_367;
const DRAND_PERIOD_SECS: u64 = 3;
/// Commit to a round this far in the future so the beacon value is
/// unobservable at challenge-open time.
const DRAND_ROUNDS_AHEAD: u64 = 2;
const DRAND_DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_";
/// Compressed BLS12-381 G2 generator point.
const BLS_G2_GENERATOR_HEX: &str = "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8";

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
            drand_pubkey: msg.drand_pubkey,
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
        ExecuteMsg::AnchorAttestation {
            id,
            activation_root,
            claims,
        } => anchor(deps, env, info, id, activation_root, claims),
        ExecuteMsg::OpenChallenge { attestation_id } => open_challenge(deps, env, info, attestation_id),
        ExecuteMsg::SubmitVerdict {
            attestation_id,
            verdict,
            evidence_hash,
        } => submit_verdict(deps, env, info, attestation_id, verdict, evidence_hash),
        ExecuteMsg::SubmitRandomness {
            attestation_id,
            signature_hex,
        } => submit_randomness(deps, info, attestation_id, signature_hex),
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
        ExecuteMsg::SetZkVerifier { address } => set_zk_verifier(deps, info, address),
        ExecuteMsg::ResolveWithProof {
            attestation_id,
            proof_base64,
            public_inputs_base64,
        } => resolve_with_proof(deps, attestation_id, proof_base64, public_inputs_base64),
        ExecuteMsg::ResolveWithRecompute {
            attestation_id,
            activations,
            bank_vec,
            threshold,
            leaf_index,
            merkle_path,
        } => resolve_with_recompute(
            deps,
            attestation_id,
            activations,
            bank_vec,
            threshold,
            leaf_index,
            merkle_path,
        ),
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
            drand_pubkey,
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
            drand_pubkey,
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
    claims: Option<ReadoutClaims>,
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
            claims,
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

    // Snapshot the verifier pool — the draw is bound to this set, so
    // registering seats after the fact can't influence it.
    let eligible: Vec<Addr> = VERIFIERS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|r| r.map(|(a, _)| Addr::unchecked(a)))
        .collect::<StdResult<Vec<_>>>()?;
    if (eligible.len() as u32) < cfg.committee_size {
        return Err(ContractError::InsufficientVerifiers {
            need: cfg.committee_size,
            have: eligible.len() as u32,
        });
    }

    let now = env.block.time.seconds();
    let mut attrs: Vec<(String, String)> = vec![
        ("action".into(), "open_challenge".into()),
        ("attestation_id".into(), attestation_id.clone()),
        ("challenger".into(), info.sender.to_string()),
    ];

    let (committee, drand_round) = if cfg.drand_pubkey.is_some() {
        // Drand mode: commit to a future quicknet round — the beacon
        // value doesn't exist yet, so nobody can grind or predict the
        // draw. The committee is filled by `SubmitRandomness` once the
        // round lands and its signature verifies on-chain.
        let round = (now - DRAND_GENESIS_TIME) / DRAND_PERIOD_SECS + DRAND_ROUNDS_AHEAD;
        attrs.push(("drand_round".into(), round.to_string()));
        (vec![], round)
    } else {
        // Legacy mode: Fiat-Shamir over public material — deterministic
        // and auditable but offline-grindable by the challenger.
        let seed = format!("{}:{}:{}", attestation_id, info.sender, env.block.height);
        let committee = sortition(deps.storage, &seed, cfg.committee_size)?;
        for seat in &committee {
            seat_bump(deps.storage, seat, 1)?;
        }
        attrs.push(("seed".into(), seed));
        (committee, 0)
    };

    DISPUTES.save(
        deps.storage,
        &attestation_id,
        &Dispute {
            attestation_id: attestation_id.clone(),
            challenger: info.sender.clone(),
            challenger_bond: bond,
            opened_at: now,
            committee,
            eligible,
            drand_round,
            randomness_hex: None,
            randomness_submitter: None,
            votes: vec![],
            verdict_deadline: now + cfg.verdict_window_secs,
        },
    )?;
    att.status = AttestationStatus::Challenged;
    ATTESTATIONS.save(deps.storage, &attestation_id, &att)?;

    Ok(Response::new().add_attributes(attrs))
}

/// Permissionless drand draw: verify the beacon signature on-chain, then
/// Fisher-Yates the verifier snapshot. Any caller may post the round's
/// signature — the check is cryptographic, so the submitter is trusted
/// for nothing.
fn submit_randomness(
    deps: DepsMut,
    info: MessageInfo,
    attestation_id: String,
    signature_hex: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let pubkey_hex = cfg
        .drand_pubkey
        .clone()
        .ok_or(ContractError::DrandNotConfigured {})?;
    let mut dispute = DISPUTES
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::DisputeNotFound {
            id: attestation_id.clone(),
        })?;
    if !dispute.committee.is_empty() {
        return Err(ContractError::SortitionAlreadyDone {
            id: attestation_id,
        });
    }

    let sig = parse_hex(&signature_hex).map_err(|reason| {
        ContractError::InvalidRandomness { reason }
    })?;
    if sig.len() != 48 {
        return Err(ContractError::InvalidRandomness {
            reason: format!("drand signature must be 48 bytes, got {}", sig.len()),
        });
    }

    // On-chain BLS verification: e(sig_G1, G2_gen) ==
    // e(H_G1(sha256(round_be)), pubkey_G2). Verified against the
    // configured beacon key — a forged submission fails here. The
    // message form is drand's unchained digest: sha256(round as BE u64).
    let pubkey = parse_hex(&pubkey_hex).map_err(|_| ContractError::InvalidRandomness {
        reason: "configured drand pubkey is not valid hex".into(),
    })?;
    let round_msg: Vec<u8> = Sha256::digest(dispute.drand_round.to_be_bytes()).to_vec();
    let hashed_round = deps
        .api
        .bls12_381_hash_to_g1(
            cosmwasm_std::HashFunction::Sha256,
            &round_msg,
            DRAND_DST,
        )
        .map_err(|_| ContractError::DrandVerificationFailed {})?;
    let g2_gen = parse_hex(BLS_G2_GENERATOR_HEX).map_err(|_| {
        ContractError::InvalidRandomness {
            reason: "g2 generator constant".into(),
        }
    })?;
    let ok = deps
        .api
        .bls12_381_pairing_equality(&sig, &g2_gen, &hashed_round, &pubkey)
        .map_err(|_| ContractError::DrandVerificationFailed {})?;
    if !ok {
        return Err(ContractError::DrandVerificationFailed {});
    }

    // Verified — the signature IS the randomness (drand derivation:
    // randomness = sha256(signature)).
    let randomness: [u8; 32] = Sha256::digest(&sig).into();
    let committee = select_members(
        &randomness,
        &dispute.eligible,
        cfg.committee_size as usize,
    );
    for seat in &committee {
        seat_bump(deps.storage, seat, 1)?;
    }
    dispute.randomness_hex = Some(hex::encode(randomness));
    dispute.randomness_submitter = Some(info.sender.clone());
    dispute.committee = committee.clone();
    DISPUTES.save(deps.storage, &attestation_id, &dispute)?;

    Ok(Response::new()
        .add_attribute("action", "submit_randomness")
        .add_attribute("attestation_id", attestation_id)
        .add_attribute("drand_round", dispute.drand_round.to_string())
        .add_attribute("submitter", info.sender)
        .add_attribute(
            "committee",
            committee
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ))
}

/// Deterministic Fisher-Yates selection of `count` members from
/// `eligible` — same construction as agent-company's sortition.
fn select_members(randomness: &[u8; 32], eligible: &[Addr], count: usize) -> Vec<Addr> {
    let n = eligible.len();
    if count >= n {
        return eligible.to_vec();
    }
    let mut indices: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let mut h = Sha256::new();
        h.update(randomness);
        h.update((i as u64).to_le_bytes());
        let sub: [u8; 32] = h.finalize().into();
        let j = u64::from_le_bytes(sub[0..8].try_into().unwrap()) as usize % (i + 1);
        indices.swap(i, j);
    }
    indices.truncate(count);
    indices.iter().map(|&i| eligible[i].clone()).collect()
}

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim_start_matches("0x");
    if s.len() % 2 != 0 {
        return Err("odd-length hex".into());
    }
    (0..s.len() / 2)
        .map(|i| {
            u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("hex: {}", e))
        })
        .collect()
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

fn set_zk_verifier(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }
    let addr = deps.api.addr_validate(&address)?;
    ZK_VERIFIER.save(deps.storage, &addr)?;
    Ok(Response::new()
        .add_attribute("action", "set_zk_verifier")
        .add_attribute("address", addr))
}

/// Terminal state for a machine-resolved attestation that has no open
/// dispute (Pending): Clean finalizes early + returns the operator bond;
/// Fraud marks the attestation and sends the bond to the DAO — the
/// evidence itself is the challenger.
fn resolve_no_dispute(
    deps: &mut DepsMut,
    cfg: &Config,
    attestation: &mut Attestation,
    verdict: Verdict,
) -> Result<Response, ContractError> {
    let mut msgs: Vec<CosmosMsg> = vec![];
    match verdict {
        Verdict::Clean => {
            attestation.status = AttestationStatus::Finalized;
            if let Some(m) = send(&cfg.denom, &attestation.operator, attestation.bond) {
                msgs.push(m);
            }
        }
        Verdict::Fraud => {
            attestation.status = AttestationStatus::Fraudulent;
            if let Some(m) = send(&cfg.denom, &cfg.admin, attestation.bond) {
                msgs.push(m);
            }
        }
    }
    ATTESTATIONS.save(deps.storage, &attestation.id, attestation)?;
    Ok(Response::new().add_messages(msgs))
}

/// Shared resolution tail for the machine-adjudication paths: picks the
/// right settlement by attestation status.
fn machine_resolve(
    mut deps: DepsMut,
    attestation_id: &str,
    mut att: Attestation,
    verdict: Verdict,
    proof_kind: &str,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    match att.status {
        AttestationStatus::Pending => {
            let mut resp = resolve_no_dispute(&mut deps, &cfg, &mut att, verdict)?;
            resp = resp
                .add_attribute("action", "resolve_with_proof")
                .add_attribute("proof_kind", proof_kind)
                .add_attribute("attestation_id", attestation_id.to_string())
                .add_attribute(
                    "verdict",
                    match verdict {
                        Verdict::Clean => "clean",
                        Verdict::Fraud => "fraud",
                    },
                );
            Ok(resp)
        }
        AttestationStatus::Challenged | AttestationStatus::Unresolved => {
            let dispute = DISPUTES
                .load(deps.storage, attestation_id)
                .map_err(|_| ContractError::DisputeNotFound {
                    id: attestation_id.to_string(),
                })?;
            let outstanding = att.status == AttestationStatus::Challenged;
            let resp = resolve(&mut deps, &cfg, &mut att, &dispute, verdict, outstanding)?;
            ATTESTATIONS.save(deps.storage, attestation_id, &att)?;
            Ok(resp.add_attribute("proof_kind", proof_kind))
        }
        _ => Err(ContractError::AttestationAlreadyResolved {
            id: attestation_id.to_string(),
        }),
    }
}

/// ZK fast-path: smart-query the configured zk-verifier; a valid proof
/// whose public inputs bind to this attestation resolves it.
fn resolve_with_proof(
    deps: DepsMut,
    attestation_id: String,
    proof_base64: String,
    public_inputs_base64: String,
) -> Result<Response, ContractError> {
    let zk = ZK_VERIFIER
        .may_load(deps.storage)?
        .ok_or(ContractError::ZkVerifierNotConfigured {})?;
    let att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    let claims = att.claims.clone().ok_or(ContractError::NotMachineAdjudicable {
        id: attestation_id.clone(),
    })?;

    // Bind public inputs to the anchored commitment + declared claims.
    let inputs = base64_decode(&public_inputs_base64).map_err(|reason| {
        ContractError::PublicInputsMalformed { reason }
    })?;
    if inputs.len() != ZK_PUBLIC_INPUTS * FR_BYTES {
        return Err(ContractError::PublicInputsMalformed {
            reason: format!(
                "expected {} bytes ({} Fr elements), got {}",
                ZK_PUBLIC_INPUTS * FR_BYTES,
                ZK_PUBLIC_INPUTS,
                inputs.len()
            ),
        });
    }
    if hex::encode(&inputs[0..FR_BYTES]) != normalize_hex(&att.activation_root) {
        return Err(ContractError::ProofBindingMismatch {});
    }
    let score = fr_le_u64(&inputs[2 * FR_BYTES..3 * FR_BYTES]);
    let integrity = fr_le_u64(&inputs[3 * FR_BYTES..4 * FR_BYTES]);
    let detection = fr_le_u64(&inputs[4 * FR_BYTES..5 * FR_BYTES]);
    if score != Some(claims.score)
        || integrity != Some(claims.integrity)
        || detection != Some(claims.detection as u64)
    {
        return Err(ContractError::ProofBindingMismatch {});
    }

    // Stateless verify against the verifier contract's stored VK.
    let verdict_resp: ZkVerifyResponse = deps
        .querier
        .query_wasm_smart(
            zk,
            &ZkVerifierQueryMsg::Verify {
                proof_base64,
                public_inputs_base64,
            },
        )
        .map_err(|e| ContractError::ProofRejected {
            reason: e.to_string(),
        })?;
    if !verdict_resp.valid {
        return Err(ContractError::ProofRejected {
            reason: verdict_resp.reason,
        });
    }

    machine_resolve(deps, &attestation_id, att, Verdict::Clean, "groth16")
}

/// Recompute fast-path: the contract re-runs the deterministic readout.
/// Inputs that bind to the anchored commitments produce a verdict —
/// Clean on match, Fraud on a proven divergence. Inputs that don't bind
/// are caller error, not a verdict.
fn resolve_with_recompute(
    deps: DepsMut,
    attestation_id: String,
    activations: Vec<i8>,
    bank_vec: Vec<i8>,
    threshold: u64,
    leaf_index: u32,
    merkle_path: Vec<String>,
) -> Result<Response, ContractError> {
    let att = ATTESTATIONS
        .load(deps.storage, &attestation_id)
        .map_err(|_| ContractError::AttestationNotFound {
            id: attestation_id.clone(),
        })?;
    let claims = att.claims.clone().ok_or(ContractError::NotMachineAdjudicable {
        id: attestation_id.clone(),
    })?;

    let binding_err = |reason: &str| ContractError::RecomputeBindingFailed {
        reason: reason.to_string(),
    };
    if activations.is_empty() || activations.len() != bank_vec.len() {
        return Err(binding_err("activations/bank length mismatch"));
    }

    // Bank binding: sha256(bank int8 bytes ‖ threshold big-endian) must
    // equal the committed bank handle.
    let mut bank_preimage: Vec<u8> = bank_vec.iter().map(|b| *b as u8).collect();
    bank_preimage.extend_from_slice(&threshold.to_be_bytes());
    let bank_hex = hex::encode(Sha256::digest(&bank_preimage));
    if bank_hex != normalize_hex(&claims.bank_commitment) {
        return Err(binding_err("bank commitment mismatch"));
    }

    // Activation binding: sha256 leaf + sha256 pair-chain walk to the
    // anchored root (same index-bit convention as the gateway tree).
    let leaf_bytes: Vec<u8> = activations.iter().map(|b| *b as u8).collect();
    let mut cur: [u8; 32] = Sha256::digest(&leaf_bytes).into();
    for (level, sib_hex) in merkle_path.iter().enumerate() {
        let sib: [u8; 32] = hex::decode(sib_hex)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| binding_err("invalid sibling hash in merkle_path"))?;
        let mut h = Sha256::new();
        if (leaf_index >> level) & 1 == 0 {
            h.update(cur);
            h.update(sib);
        } else {
            h.update(sib);
            h.update(cur);
        }
        cur = h.finalize().into();
    }
    if hex::encode(cur) != normalize_hex(&att.activation_root) {
        return Err(binding_err("merkle root mismatch"));
    }

    // Bound inputs — now the recompute is authoritative.
    let score: i64 = activations
        .iter()
        .zip(bank_vec.iter())
        .map(|(a, b)| (*a as i64) * (*b as i64))
        .sum();
    let clean = score >= 0
        && score as u64 == claims.score
        && READOUT_SCALE.saturating_sub(score as u64) == claims.integrity
        && ((score as u64) >= threshold) == claims.detection;
    let verdict = if clean { Verdict::Clean } else { Verdict::Fraud };

    machine_resolve(deps, &attestation_id, att, verdict, "recompute")
}

/// Little-endian u64 of an arkworks-compressed Fr limb — returns None
/// when the value doesn't fit (claims are u64; field elements encoding
/// larger values can never match them).
fn fr_le_u64(fr_bytes: &[u8]) -> Option<u64> {
    if fr_bytes.len() != FR_BYTES || fr_bytes[8..].iter().any(|b| *b != 0) {
        return None;
    }
    Some(u64::from_le_bytes(fr_bytes[0..8].try_into().ok()?))
}

fn normalize_hex(s: &str) -> String {
    s.trim_start_matches("0x").to_ascii_lowercase()
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    cosmwasm_std::from_json::<cosmwasm_std::Binary>(format!("\"{}\"", input).into_bytes())
        .map(|b| b.to_vec())
        .map_err(|e| format!("base64: {}", e))
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
    drand_pubkey: Option<String>,
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
    if let Some(v) = drand_pubkey {
        cfg.drand_pubkey = Some(v);
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
        QueryMsg::GetZkVerifier {} => {
            let addr = ZK_VERIFIER.may_load(deps.storage)?;
            to_json_binary(&ZkVerifierResponse {
                address: addr.map(|a| a.to_string()),
            })
        }
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}
