use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, AppBuilder, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{CommitteeResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Attestation, AttestationStatus, Dispute, Verdict};

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

fn court(app: &mut App) -> Addr {
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
        },
        &[],
        "thought-court",
        Some(admin.to_string()),
    )
    .unwrap()
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
