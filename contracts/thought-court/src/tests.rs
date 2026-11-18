use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, AppBuilder, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{CommitteeResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Attestation, AttestationStatus, Dispute, ReadoutClaims, Verdict};

const ADMIN: &str = "dao";
const OPERATOR: &str = "operator";
const CHALLENGER: &str = "challenger";
const VERIFIERS: [&str; 6] = ["v0", "v1", "v2", "v3", "v4", "v5"];

const DENOM: &str = "ujclaw";
const OP_BOND: u128 = 1_000_000;
const CH_BOND: u128 = 500_000;
const VF_BOND: u128 = 2_000_000;

fn mk(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

fn app_with_balances() -> App {
    AppBuilder::new().build(|router, api, storage| {
        for l in [ADMIN, OPERATOR, CHALLENGER]
            .iter()
            .chain(VERIFIERS.iter())
        {
            router
                .bank
                .init_balance(
                    storage,
                    &api.addr_make(l),
                    coins(100_000_000, DENOM),
                )
                .unwrap();
        }
    })
}

fn court_with(app: &mut App, drand_pubkey: Option<String>) -> Addr {
    let admin = mk(app, ADMIN);
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: Some(admin.to_string()),
            denom: None,
            min_operator_bond: Some(Uint128::new(OP_BOND)),
            min_challenger_bond: Some(Uint128::new(CH_BOND)),
            min_verifier_bond: Some(Uint128::new(VF_BOND)),
            challenge_window_secs: Some(1_000),
            verdict_window_secs: Some(500),
            committee_size: Some(3),
            quorum_bp: Some(6_700), // ceil(6700*3/10000) = 3 of 3
            challenger_reward_bp: Some(5_000),
            operator_reward_bp: Some(5_000),
            verifier_slash_bp: Some(2_000),
            drand_pubkey,
        },
        &[],
        "thought-court",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn court(app: &mut App) -> Addr {
    court_with(app, None)
}

fn register_verifiers(app: &mut App, court: &Addr) {
    for v in VERIFIERS {
        let sender = mk(app, v);
        app.execute_contract(
            sender,
            court.clone(),
            &ExecuteMsg::RegisterVerifier {},
            &coins(VF_BOND, DENOM),
        )
        .unwrap();
    }
}

fn anchor(app: &mut App, court: &Addr, id: &str) {
    app.execute_contract(
        mk(app, OPERATOR),
        court.clone(),
        &ExecuteMsg::AnchorAttestation {
            id: id.to_string(),
            activation_root: "root".into(),
            claims: None,
        },
        &coins(OP_BOND, DENOM),
    )
    .unwrap();
}

fn anchor_claims(app: &mut App, court: &Addr, id: &str, root: &str, claims: ReadoutClaims) {
    app.execute_contract(
        mk(app, OPERATOR),
        court.clone(),
        &ExecuteMsg::AnchorAttestation {
            id: id.to_string(),
            activation_root: root.to_string(),
            claims: Some(claims),
        },
        &coins(OP_BOND, DENOM),
    )
    .unwrap();
}

fn challenge(app: &mut App, court: &Addr, id: &str) -> Dispute {
    app.execute_contract(
        mk(app, CHALLENGER),
        court.clone(),
        &ExecuteMsg::OpenChallenge {
            attestation_id: id.to_string(),
        },
        &coins(CH_BOND, DENOM),
    )
    .unwrap();
    dispute(app, court, id)
}

fn dispute(app: &App, court: &Addr, id: &str) -> Dispute {
    app.wrap()
        .query_wasm_smart(
            court.clone(),
            &QueryMsg::GetDispute {
                attestation_id: id.to_string(),
            },
        )
        .unwrap()
}

fn vote(
    app: &mut App,
    court: &Addr,
    id: &str,
    verifier: &Addr,
    verdict: Verdict,
) -> Result<(), anyhow::Error> {
    app.execute_contract(
        verifier.clone(),
        court.clone(),
        &ExecuteMsg::SubmitVerdict {
            attestation_id: id.to_string(),
            verdict,
            evidence_hash: "evidence".into(),
        },
        &[],
    )
    .map(|_| ())
}

fn attestation(app: &App, court: &Addr, id: &str) -> Attestation {
    app.wrap()
        .query_wasm_smart(
            court.clone(),
            &QueryMsg::GetAttestation {
                attestation_id: id.to_string(),
            },
        )
        .unwrap()
}

fn balance(app: &App, label: &str) -> u128 {
    app.wrap()
        .query_balance(mk(app, label).to_string(), DENOM)
        .unwrap()
        .amount
        .u128()
}

// ── Machine-adjudication fixtures + stub zk-verifier ──

/// Minimal zk-verifier stand-in: answers the `Verify` smart query the
/// court issues — "bad" proofs are rejected, everything else valid.
/// The real Groth16 path is covered by zk-verifier's own tests and the
/// live g1 verification; here we exercise the court's binding logic.
mod stub_zk {
    use cosmwasm_std::{
        to_json_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
    };

    use crate::msg::{ZkVerifierQueryMsg, ZkVerifyResponse};

    pub fn instantiate(
        _d: DepsMut,
        _e: Env,
        _i: MessageInfo,
        _m: Empty,
    ) -> StdResult<Response> {
        Ok(Response::new())
    }
    pub fn execute(_d: DepsMut, _e: Env, _i: MessageInfo, _m: Empty) -> StdResult<Response> {
        Ok(Response::new())
    }
    pub fn query(_d: Deps, _e: Env, m: ZkVerifierQueryMsg) -> StdResult<Binary> {
        match m {
            ZkVerifierQueryMsg::Verify { proof_base64, .. } => {
                to_json_binary(&ZkVerifyResponse {
                    valid: proof_base64 != "bad",
                    reason: if proof_base64 == "bad" {
                        "invalid proof".into()
                    } else {
                        String::new()
                    },
                })
            }
        }
    }
}

use cosmwasm_std::Empty;

fn zk_stub(app: &mut App) -> Addr {
    let admin = mk(app, ADMIN);
    let code = ContractWrapper::new(stub_zk::execute, stub_zk::instantiate, stub_zk::query);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(code_id, admin, &Empty {}, &[], "zk-stub", None)
        .unwrap()
}

fn set_zk(app: &mut App, court: &Addr, zk: &Addr) {
    app.execute_contract(
        mk(app, ADMIN),
        court.clone(),
        &ExecuteMsg::SetZkVerifier {
            address: zk.to_string(),
        },
        &[],
    )
    .unwrap();
}

/// Readout fixture matching the jlens-readout circuit test vector:
/// acts [10..17] · bank [3..10] = 744, threshold 500 → detection.
const ACTS: [i8; 8] = [10, 11, 12, 13, 14, 15, 16, 17];
const BANK: [i8; 8] = [3, 4, 5, 6, 7, 8, 9, 10];
const THRESHOLD: u64 = 500;
const SCORE: u64 = 744;
const INTEGRITY: u64 = 999_256;

fn bank_commitment_hex() -> String {
    use sha2::{Digest, Sha256};
    let mut pre: Vec<u8> = BANK.iter().map(|b| *b as u8).collect();
    pre.extend_from_slice(&THRESHOLD.to_be_bytes());
    hex::encode(Sha256::digest(&pre))
}

fn fixture_claims() -> ReadoutClaims {
    ReadoutClaims {
        score: SCORE,
        integrity: INTEGRITY,
        detection: true,
        bank_commitment: bank_commitment_hex(),
    }
}

/// Single-leaf tree: root = sha256(int8 activation bytes), empty path.
fn single_leaf_root() -> String {
    use sha2::{Digest, Sha256};
    let bytes: Vec<u8> = ACTS.iter().map(|b| *b as u8).collect();
    hex::encode(Sha256::digest(&bytes))
}

/// 160-byte public inputs: [root_fr ‖ bank_fr ‖ score ‖ integrity ‖ det].
fn zk_inputs(root: &[u8; 32], score: u64, integrity: u64, detection: u64) -> String {
    let mut v = vec![0u8; 160];
    v[0..32].copy_from_slice(root);
    v[64..72].copy_from_slice(&score.to_le_bytes());
    v[96..104].copy_from_slice(&integrity.to_le_bytes());
    v[128..136].copy_from_slice(&detection.to_le_bytes());
    Binary::from(v).to_base64()
}

#[test]
fn honest_attestation_finalizes_after_window() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-a");

    app.update_block(|b| b.time = b.time.plus_seconds(1_001));
    app.execute_contract(
        mk(&app, OPERATOR),
        court.clone(),
        &ExecuteMsg::FinalizeAttestation {
            attestation_id: "batch-a".into(),
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "batch-a").status,
        AttestationStatus::Finalized
    );
    assert_eq!(balance(&app, OPERATOR), 100_000_000); // bond returned
}

#[test]
fn early_finalize_and_late_challenge_rejected() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-b");

    assert!(app
        .execute_contract(
            mk(&app, OPERATOR),
            court.clone(),
            &ExecuteMsg::FinalizeAttestation {
                attestation_id: "batch-b".into(),
            },
            &[],
        )
        .is_err());

    app.update_block(|b| b.time = b.time.plus_seconds(1_001));
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::OpenChallenge {
                attestation_id: "batch-b".into(),
            },
            &coins(CH_BOND, DENOM),
        )
        .is_err());
}

#[test]
fn challenge_needs_quorum_of_verifiers() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    anchor(&mut app, &court, "batch-c"); // no verifiers registered

    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::OpenChallenge {
                attestation_id: "batch-c".into(),
            },
            &coins(CH_BOND, DENOM),
        )
        .is_err());
}

#[test]
fn fraud_verdict_slashes_operator_pays_challenger_and_dao() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-d");
    let d = challenge(&mut app, &court, "batch-d");

    assert_eq!(d.committee.len(), 3);
    vote(&mut app, &court, "batch-d", &d.committee[0], Verdict::Fraud).unwrap();
    vote(&mut app, &court, "batch-d", &d.committee[1], Verdict::Fraud).unwrap();
    vote(&mut app, &court, "batch-d", &d.committee[2], Verdict::Fraud).unwrap();

    assert_eq!(
        attestation(&app, &court, "batch-d").status,
        AttestationStatus::Fraudulent
    );
    // challenger: bond back + 50% of operator bond = 500k + 500k
    assert_eq!(balance(&app, CHALLENGER), 100_000_000 + 500_000);
    // dao gets the other 500k of the operator bond
    assert_eq!(balance(&app, ADMIN), 100_000_000 + 500_000);
    assert_eq!(balance(&app, OPERATOR), 99_000_000); // bond gone
}

#[test]
fn clean_verdict_slashes_challenger_and_minority_voter() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-e");
    let d = challenge(&mut app, &court, "batch-e");

    vote(&mut app, &court, "batch-e", &d.committee[0], Verdict::Clean).unwrap();
    vote(&mut app, &court, "batch-e", &d.committee[1], Verdict::Fraud).unwrap();
    vote(&mut app, &court, "batch-e", &d.committee[2], Verdict::Clean).unwrap();

    assert_eq!(
        attestation(&app, &court, "batch-e").status,
        AttestationStatus::Finalized
    );
    // operator: bond back + 50% of challenger bond
    assert_eq!(balance(&app, OPERATOR), 100_000_000 + 250_000);
    // dao gets 250k challenger share + 20% of the minority voter's 2M = 400k
    assert_eq!(balance(&app, ADMIN), 100_000_000 + 250_000 + 400_000);
    assert_eq!(balance(&app, CHALLENGER), 99_500_000);
}

#[test]
fn non_committee_and_double_votes_rejected() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-f");
    let d = challenge(&mut app, &court, "batch-f");

    let outsider = VERIFIERS
        .iter()
        .map(|v| mk(&app, v))
        .find(|a| !d.committee.contains(a));
    if let Some(o) = outsider {
        assert!(vote(&mut app, &court, "batch-f", &o, Verdict::Clean).is_err());
    }
    vote(&mut app, &court, "batch-f", &d.committee[0], Verdict::Clean).unwrap();
    assert!(vote(&mut app, &court, "batch-f", &d.committee[0], Verdict::Fraud).is_err());
}

#[test]
fn stalled_committee_times_out_and_dao_force_resolves() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-g");
    let d = challenge(&mut app, &court, "batch-g");

    // one vote only — below the 3-vote quorum
    vote(&mut app, &court, "batch-g", &d.committee[0], Verdict::Fraud).unwrap();
    app.update_block(|b| b.time = b.time.plus_seconds(501));

    app.execute_contract(
        mk(&app, OPERATOR),
        court.clone(),
        &ExecuteMsg::ResolveTimeout {
            attestation_id: "batch-g".into(),
        },
        &[],
    )
    .unwrap();
    assert_eq!(
        attestation(&app, &court, "batch-g").status,
        AttestationStatus::Unresolved
    );
    assert_eq!(balance(&app, CHALLENGER), 100_000_000); // bond refunded

    // double-resolution protection: no more votes accepted
    assert!(vote(&mut app, &court, "batch-g", &d.committee[1], Verdict::Fraud).is_err());
    // non-admin cannot force-resolve
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ForceResolve {
                attestation_id: "batch-g".into(),
                verdict: Verdict::Fraud,
            },
            &[],
        )
        .is_err());

    app.execute_contract(
        mk(&app, ADMIN),
        court.clone(),
        &ExecuteMsg::ForceResolve {
            attestation_id: "batch-g".into(),
            verdict: Verdict::Fraud,
        },
        &[],
    )
    .unwrap();
    assert_eq!(
        attestation(&app, &court, "batch-g").status,
        AttestationStatus::Fraudulent
    );
    // challenger bond already refunded: reward only, not double bond
    assert_eq!(balance(&app, CHALLENGER), 100_000_000 + 500_000);
}

#[test]
fn verifier_cannot_unbond_while_seated() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "batch-h");
    let d = challenge(&mut app, &court, "batch-h");

    assert!(app
        .execute_contract(
            d.committee[0].clone(),
            court.clone(),
            &ExecuteMsg::RemoveVerifier {},
            &[],
        )
        .is_err());

    // unseated verifier can unbond
    let outsider = VERIFIERS
        .iter()
        .map(|v| mk(&app, v))
        .find(|a| !d.committee.contains(a));
    if let Some(o) = outsider {
        app.execute_contract(o, court.clone(), &ExecuteMsg::RemoveVerifier {}, &[])
            .unwrap();
    }
}

#[test]
fn committee_preview_is_deterministic() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    let c1: CommitteeResponse = app
        .wrap()
        .query_wasm_smart(
            court.clone(),
            &QueryMsg::CommitteePreview { seed: "abc".into() },
        )
        .unwrap();
    let c2: CommitteeResponse = app
        .wrap()
        .query_wasm_smart(
            court.clone(),
            &QueryMsg::CommitteePreview { seed: "abc".into() },
        )
        .unwrap();
    assert_eq!(c1.committee, c2.committee);
    assert_eq!(c1.committee.len(), 3);
    let c3: CommitteeResponse = app
        .wrap()
        .query_wasm_smart(
            court.clone(),
            &QueryMsg::CommitteePreview { seed: "xyz".into() },
        )
        .unwrap();
    assert_eq!(c3.committee.len(), 3);
}

#[test]
fn double_anchor_and_double_challenge_rejected() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor(&mut app, &court, "dup");
    assert!(app
        .execute_contract(
            mk(&app, OPERATOR),
            court.clone(),
            &ExecuteMsg::AnchorAttestation {
                id: "dup".into(),
                activation_root: "root2".into(),
                claims: None,
            },
            &coins(OP_BOND, DENOM),
        )
        .is_err());

    challenge(&mut app, &court, "dup");
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::OpenChallenge {
                attestation_id: "dup".into(),
            },
            &coins(CH_BOND, DENOM),
        )
        .is_err());
}

// ── Machine-adjudication tests ──

#[test]
fn proof_resolves_pending_attestation() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    let zk = zk_stub(&mut app);
    set_zk(&mut app, &court, &zk);

    let root = [0x11u8; 32];
    let root_hex = hex::encode(root);
    anchor_claims(&mut app, &court, "zk-a", &root_hex, fixture_claims());

    app.execute_contract(
        mk(&app, CHALLENGER), // anyone may submit — proof is the evidence
        court.clone(),
        &ExecuteMsg::ResolveWithProof {
            attestation_id: "zk-a".into(),
            proof_base64: "valid".into(),
            public_inputs_base64: zk_inputs(&root, SCORE, INTEGRITY, 1),
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "zk-a").status,
        AttestationStatus::Finalized
    );
    assert_eq!(balance(&app, OPERATOR), 100_000_000); // bond returned early
}

#[test]
fn proof_resolves_challenged_as_clean() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    let zk = zk_stub(&mut app);
    set_zk(&mut app, &court, &zk);

    let root = [0x22u8; 32];
    anchor_claims(&mut app, &court, "zk-b", &hex::encode(root), fixture_claims());
    let d = challenge(&mut app, &court, "zk-b");

    app.execute_contract(
        mk(&app, OPERATOR),
        court.clone(),
        &ExecuteMsg::ResolveWithProof {
            attestation_id: "zk-b".into(),
            proof_base64: "valid".into(),
            public_inputs_base64: zk_inputs(&root, SCORE, INTEGRITY, 1),
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "zk-b").status,
        AttestationStatus::Finalized
    );
    // Clean economics: operator bond + 50% challenger bond; DAO the rest.
    assert_eq!(balance(&app, OPERATOR), 100_000_000 + 250_000);
    assert_eq!(balance(&app, ADMIN), 100_000_000 + 250_000);
    // committee seats released — members can unbond
    app.execute_contract(
        d.committee[0].clone(),
        court.clone(),
        &ExecuteMsg::RemoveVerifier {},
        &[],
    )
    .unwrap();
}

#[test]
fn proof_rejections_invalid_inputs_and_binding() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    let zk = zk_stub(&mut app);
    set_zk(&mut app, &court, &zk);

    let root = [0x33u8; 32];
    anchor_claims(&mut app, &court, "zk-c", &hex::encode(root), fixture_claims());

    let try_proof = |app: &mut App, proof: &str, inputs: String| {
        app.execute_contract(
            mk(app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithProof {
                attestation_id: "zk-c".into(),
                proof_base64: proof.into(),
                public_inputs_base64: inputs,
            },
            &[],
        )
    };

    // verifier rejects the proof itself
    assert!(try_proof(&mut app, "bad", zk_inputs(&root, SCORE, INTEGRITY, 1)).is_err());
    // malformed public inputs (not 5×32 bytes)
    assert!(try_proof(&mut app, "valid", Binary::from(vec![0u8; 64]).to_base64()).is_err());
    // proof bound to a different root
    assert!(try_proof(&mut app, "valid", zk_inputs(&[0x99u8; 32], SCORE, INTEGRITY, 1)).is_err());
    // proof bound to the right root but wrong claimed score
    assert!(try_proof(&mut app, "valid", zk_inputs(&root, SCORE + 1, INTEGRITY, 1)).is_err());

    assert_eq!(
        attestation(&app, &court, "zk-c").status,
        AttestationStatus::Pending // nothing resolved
    );
}

#[test]
fn machine_adjudication_requires_claims_and_zk_config() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    anchor(&mut app, &court, "no-claims"); // legacy shape: no claims

    let inputs = zk_inputs(&[0x44u8; 32], SCORE, INTEGRITY, 1);
    // no claims → not machine-adjudicable (both paths)
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithProof {
                attestation_id: "no-claims".into(),
                proof_base64: "valid".into(),
                public_inputs_base64: inputs.clone(),
            },
            &[],
        )
        .is_err());
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithRecompute {
                attestation_id: "no-claims".into(),
                activations: ACTS.to_vec(),
                bank_vec: BANK.to_vec(),
                threshold: THRESHOLD,
                leaf_index: 0,
                merkle_path: vec![],
            },
            &[],
        )
        .is_err());

    // claims present but no zk verifier configured → proof path errors
    let root = [0x55u8; 32];
    anchor_claims(&mut app, &court, "no-zk", &hex::encode(root), fixture_claims());
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithProof {
                attestation_id: "no-zk".into(),
                proof_base64: "valid".into(),
                public_inputs_base64: zk_inputs(&root, SCORE, INTEGRITY, 1),
            },
            &[],
        )
        .is_err());
}

#[test]
fn recompute_finalizes_pending_attestation() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    anchor_claims(&mut app, &court, "rc-a", &single_leaf_root(), fixture_claims());

    app.execute_contract(
        mk(&app, CHALLENGER),
        court.clone(),
        &ExecuteMsg::ResolveWithRecompute {
            attestation_id: "rc-a".into(),
            activations: ACTS.to_vec(),
            bank_vec: BANK.to_vec(),
            threshold: THRESHOLD,
            leaf_index: 0,
            merkle_path: vec![],
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "rc-a").status,
        AttestationStatus::Finalized
    );
    assert_eq!(balance(&app, OPERATOR), 100_000_000);
}

#[test]
fn recompute_clean_resolves_challenged() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    register_verifiers(&mut app, &court);
    anchor_claims(&mut app, &court, "rc-b", &single_leaf_root(), fixture_claims());
    challenge(&mut app, &court, "rc-b");

    app.execute_contract(
        mk(&app, OPERATOR),
        court.clone(),
        &ExecuteMsg::ResolveWithRecompute {
            attestation_id: "rc-b".into(),
            activations: ACTS.to_vec(),
            bank_vec: BANK.to_vec(),
            threshold: THRESHOLD,
            leaf_index: 0,
            merkle_path: vec![],
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "rc-b").status,
        AttestationStatus::Finalized
    );
    assert_eq!(balance(&app, OPERATOR), 100_000_000 + 250_000);
    assert_eq!(balance(&app, ADMIN), 100_000_000 + 250_000);
}

#[test]
fn recompute_fraud_on_divergent_claims() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    // Operator anchored WRONG claims — committed activations don't
    // produce score 999.
    let mut bad = fixture_claims();
    bad.score = 999;
    anchor_claims(&mut app, &court, "rc-c", &single_leaf_root(), bad);

    app.execute_contract(
        mk(&app, CHALLENGER),
        court.clone(),
        &ExecuteMsg::ResolveWithRecompute {
            attestation_id: "rc-c".into(),
            activations: ACTS.to_vec(),
            bank_vec: BANK.to_vec(),
            threshold: THRESHOLD,
            leaf_index: 0,
            merkle_path: vec![],
        },
        &[],
    )
    .unwrap();

    assert_eq!(
        attestation(&app, &court, "rc-c").status,
        AttestationStatus::Fraudulent
    );
    // No challenger — proven fraud pays the bond to the DAO.
    assert_eq!(balance(&app, ADMIN), 100_000_000 + OP_BOND as u128);
    assert_eq!(balance(&app, OPERATOR), 99_000_000);
}

#[test]
fn recompute_binding_failures_rejected() {
    let mut app = app_with_balances();
    let court = court(&mut app);

    // (a) wrong bank commitment in claims
    let mut bad_bank = fixture_claims();
    bad_bank.bank_commitment = hex::encode([0u8; 32]);
    anchor_claims(&mut app, &court, "rb-a", &single_leaf_root(), bad_bank);
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithRecompute {
                attestation_id: "rb-a".into(),
                activations: ACTS.to_vec(),
                bank_vec: BANK.to_vec(),
                threshold: THRESHOLD,
                leaf_index: 0,
                merkle_path: vec![],
            },
            &[],
        )
        .is_err());

    // (b) anchored root doesn't match the leaf evidence
    anchor_claims(&mut app, &court, "rb-b", &hex::encode([0xdeu8; 32]), fixture_claims());
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::ResolveWithRecompute {
                attestation_id: "rb-b".into(),
                activations: ACTS.to_vec(),
                bank_vec: BANK.to_vec(),
                threshold: THRESHOLD,
                leaf_index: 0,
                merkle_path: vec![],
            },
            &[],
        )
        .is_err());
}

#[test]
fn already_resolved_attestation_rejects_further_proofs() {
    let mut app = app_with_balances();
    let court = court(&mut app);
    let zk = zk_stub(&mut app);
    set_zk(&mut app, &court, &zk);

    let root = [0x77u8; 32];
    anchor_claims(&mut app, &court, "zk-d", &hex::encode(root), fixture_claims());
    let msg = ExecuteMsg::ResolveWithProof {
        attestation_id: "zk-d".into(),
        proof_base64: "valid".into(),
        public_inputs_base64: zk_inputs(&root, SCORE, INTEGRITY, 1),
    };
    app.execute_contract(mk(&app, CHALLENGER), court.clone(), &msg, &[])
        .unwrap();
    // replay: already Finalized → rejected
    assert!(app
        .execute_contract(mk(&app, OPERATOR), court.clone(), &msg, &[])
        .is_err());
}

// ── Drand sortition tests (real quicknet round 5000000) ──

/// Real drand quicknet values — the contract verifies the signature
/// against this exact beacon key on-chain.
const DRAND_PUBKEY: &str = "83cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a";
const DRAND_SIG_5000000: &str = "a597f735255263bf239b481d8065e0ef91eed416290da51e3ee6f128a9be29b4429525552c32fbbdb0dee406743a863b";
const DRAND_RAND_5000000: &str = "4c6c26dc9b7f65728845b67eb8d3c4a5f0f6c5f582de936df22bea654e68d4a9";

/// Block time such that (t - 1692803367)/3 + 2 == 5000000.
const DRAND_TIME_5000000: u64 = 1_707_803_361;

#[test]
fn drand_draw_fills_committee_with_verified_signature() {
    let mut app = app_with_balances();
    let court = court_with(&mut app, Some(DRAND_PUBKEY.to_string()));
    register_verifiers(&mut app, &court);

    app.update_block(|b| {
        b.time = cosmwasm_std::Timestamp::from_seconds(DRAND_TIME_5000000)
    });
    anchor(&mut app, &court, "drand-a");
    challenge(&mut app, &court, "drand-a");

    // Pending draw: committee empty, round committed
    let d = dispute(&app, &court, "drand-a");
    assert!(d.committee.is_empty());
    assert_eq!(d.drand_round, 5_000_000);
    assert_eq!(d.eligible.len(), 6);

    // Vote before draw → not a committee member (empty committee)
    assert!(app
        .execute_contract(
            mk(&app, "v0"),
            court.clone(),
            &ExecuteMsg::SubmitVerdict {
                attestation_id: "drand-a".into(),
                verdict: Verdict::Fraud,
                evidence_hash: "e".into(),
            },
            &[],
        )
        .is_err());

    // Bad signature → on-chain BLS verification rejects it
    let bad_sig = "aa".repeat(48);
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::SubmitRandomness {
                attestation_id: "drand-a".into(),
                signature_hex: bad_sig,
            },
            &[],
        )
        .is_err());

    // The REAL round-5000000 signature verifies → committee drawn
    app.execute_contract(
        mk(&app, OPERATOR), // permissionless: anyone can submit
        court.clone(),
        &ExecuteMsg::SubmitRandomness {
            attestation_id: "drand-a".into(),
            signature_hex: DRAND_SIG_5000000.into(),
        },
        &[],
    )
    .unwrap();

    let d = dispute(&app, &court, "drand-a");
    assert_eq!(d.committee.len(), 3); // k=3 drawn from n=6
    assert_eq!(d.randomness_hex.as_deref(), Some(DRAND_RAND_5000000));
    // drawn members are a subset of the eligible snapshot
    for m in &d.committee {
        assert!(d.eligible.contains(m));
    }

    // replay → rejected
    assert!(app
        .execute_contract(
            mk(&app, CHALLENGER),
            court.clone(),
            &ExecuteMsg::SubmitRandomness {
                attestation_id: "drand-a".into(),
                signature_hex: DRAND_SIG_5000000.into(),
            },
            &[],
        )
        .is_err());

    // quorum path still works end-to-end after the draw
    vote(&mut app, &court, "drand-a", &d.committee[0], Verdict::Clean).unwrap();
    vote(&mut app, &court, "drand-a", &d.committee[1], Verdict::Clean).unwrap();
    vote(&mut app, &court, "drand-a", &d.committee[2], Verdict::Clean).unwrap();
    assert_eq!(
        attestation(&app, &court, "drand-a").status,
        AttestationStatus::Finalized
    );
}

#[test]
fn drand_mode_draws_k_of_n_and_excludes_rest() {
    let mut app = app_with_balances();
    let court = court_with(&mut app, Some(DRAND_PUBKEY.to_string()));
    register_verifiers(&mut app, &court);

    app.update_block(|b| {
        b.time = cosmwasm_std::Timestamp::from_seconds(DRAND_TIME_5000000)
    });
    anchor(&mut app, &court, "drand-b");
    challenge(&mut app, &court, "drand-b");

    app.execute_contract(
        mk(&app, CHALLENGER),
        court.clone(),
        &ExecuteMsg::SubmitRandomness {
            attestation_id: "drand-b".into(),
            signature_hex: DRAND_SIG_5000000.into(),
        },
        &[],
    )
    .unwrap();
    let d = dispute(&app, &court, "drand-b");
    // k < n demonstrated: 3 seats drawn, 3 eligible seats excluded
    assert_eq!(d.committee.len(), 3);
    let excluded: Vec<_> = d
        .eligible
        .iter()
        .filter(|a| !d.committee.contains(a))
        .collect();
    assert_eq!(excluded.len(), 3);
    // excluded seat cannot vote
    assert!(app
        .execute_contract(
            excluded[0].clone(),
            court.clone(),
            &ExecuteMsg::SubmitVerdict {
                attestation_id: "drand-b".into(),
                verdict: Verdict::Clean,
                evidence_hash: "e".into(),
            },
            &[],
        )
        .is_err());
}
