use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::*;
use crate::state::{GrantTier, SubmissionStatus, WorkSubmission};

const DENOM: &str = "ujunox";
const VALID_HASH: &str = "9d0f7354205de1fcaa41a8642ee704ed8e6201bdf8e4951b36923499a7367a3b";
const ATTEST_HASH: &str = "945a53c5c1aab2e99432e659d47633da491fffc399d95cbce66b8e88fae5c0e8";

fn mk(app: &App, label: &str) -> Addr { app.api().addr_make(label) }

struct TestEnv {
    app: App,
    contract: Addr,
    admin: Addr,
    operator: Addr,
    builder1: Addr,
    builder2: Addr,
    stranger: Addr,
}

fn setup_app() -> TestEnv {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let operator = mk(&app, "operator");
    let builder1 = mk(&app, "builder1");
    let builder2 = mk(&app, "builder2");
    let stranger = mk(&app, "stranger");

    app.init_modules(|router, _api, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(100_000_000_000, DENOM))
            .unwrap();
    });

    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));

    let contract = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                denom: DENOM.to_string(),
                operators: vec![operator.to_string()],
                agent_company: None,
            },
            &[],
            "builder-grant",
            Some(admin.to_string()),
        )
        .unwrap();

    // Fund with 50,000 JUNOX
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Fund {},
        &coins(50_000_000_000, DENOM),
    )
    .unwrap();

    TestEnv { app, contract, admin, operator, builder1, builder2, stranger }
}

#[test]
fn test_instantiate() {
    let env = setup_app();

    let resp: ConfigResponse = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetConfig {})
        .unwrap();

    assert_eq!(resp.admin, env.admin);
    assert_eq!(resp.operators, vec![env.operator]);
    assert!(resp.active);
    assert_eq!(resp.balance, 50_000_000_000);
}

#[test]
fn test_submit_and_verify_and_claim() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::ContractDeploy,
            evidence: "juno1abc...deployed_contract_address".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    let sub: WorkSubmission = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetSubmission { id: 1 })
        .unwrap();
    assert_eq!(sub.status, SubmissionStatus::Pending);
    assert_eq!(sub.builder, env.builder1);

    env.app.execute_contract(
        env.operator.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: true,
        },
        &[],
    )
    .unwrap();

    let sub: WorkSubmission = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetSubmission { id: 1 })
        .unwrap();
    assert_eq!(sub.status, SubmissionStatus::Verified);
    assert_eq!(sub.attestation_hash, Some(ATTEST_HASH.to_string()));

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::ClaimGrant { submission_id: 1 },
        &[],
    )
    .unwrap();

    let balance = env.app.wrap().query_balance(&env.builder1, DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(500_000_000u128));

    let sub: WorkSubmission = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetSubmission { id: 1 })
        .unwrap();
    assert_eq!(sub.status, SubmissionStatus::Claimed);

    let stats: StatsResponse = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_granted, 500_000_000);
    assert_eq!(stats.total_submissions, 1);
}

#[test]
fn test_unauthorized_operator_rejected() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::ContractDeploy,
            evidence: "some evidence".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    let err = env.app
        .execute_contract(
            env.stranger.clone(),
            env.contract.clone(),
            &ExecuteMsg::VerifyWork {
                submission_id: 1,
                attestation_hash: ATTEST_HASH.to_string(),
                approved: true,
            },
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("unauthorized operator"));
}

#[test]
fn test_wrong_builder_cannot_claim() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::TeeAttestation,
            evidence: "tee proof TX hash".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.operator.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: true,
        },
        &[],
    )
    .unwrap();

    let err = env.app
        .execute_contract(
            env.builder2.clone(),
            env.contract.clone(),
            &ExecuteMsg::ClaimGrant { submission_id: 1 },
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("not the builder"));
}

#[test]
fn test_rejected_submission_cannot_claim() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::ContractDeploy,
            evidence: "some evidence".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.operator.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: false,
        },
        &[],
    )
    .unwrap();

    let sub: WorkSubmission = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetSubmission { id: 1 })
        .unwrap();
    assert_eq!(sub.status, SubmissionStatus::Rejected);

    let err = env.app
        .execute_contract(
            env.builder1.clone(),
            env.contract.clone(),
            &ExecuteMsg::ClaimGrant { submission_id: 1 },
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("not verified"));
}

#[test]
fn test_grant_tiers() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::TeeAttestation,
            evidence: "attestation TX hash".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.operator.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: true,
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::ClaimGrant { submission_id: 1 },
        &[],
    )
    .unwrap();

    let balance = env.app.wrap().query_balance(&env.builder1, DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(2_000_000_000u128));
}

#[test]
fn test_custom_grant_tier() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::Custom {
                amount: 5_000_000_000,
                description: "Full DEX integration with WAVS verification".to_string(),
            },
            evidence: "multiple TX hashes and contract addresses".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.operator.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: true,
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::ClaimGrant { submission_id: 1 },
        &[],
    )
    .unwrap();

    let balance = env.app.wrap().query_balance(&env.builder1, DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(5_000_000_000u128));
}

#[test]
fn test_invalid_work_hash_rejected() {
    let mut env = setup_app();

    let err = env.app
        .execute_contract(
            env.builder1.clone(),
            env.contract.clone(),
            &ExecuteMsg::SubmitWork {
                tier: GrantTier::ContractDeploy,
                evidence: "some evidence".to_string(),
                work_hash: "tooshort".to_string(),
            },
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("invalid work hash"));
}

#[test]
fn test_admin_can_verify() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.builder1.clone(),
        env.contract.clone(),
        &ExecuteMsg::SubmitWork {
            tier: GrantTier::GovernanceParticipation,
            evidence: "proposal 1 voted".to_string(),
            work_hash: VALID_HASH.to_string(),
        },
        &[],
    )
    .unwrap();

    env.app.execute_contract(
        env.admin.clone(),
        env.contract.clone(),
        &ExecuteMsg::VerifyWork {
            submission_id: 1,
            attestation_hash: ATTEST_HASH.to_string(),
            approved: true,
        },
        &[],
    )
    .unwrap();

    let sub: WorkSubmission = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetSubmission { id: 1 })
        .unwrap();
    assert_eq!(sub.status, SubmissionStatus::Verified);
    assert_eq!(sub.verified_by, Some(env.admin.clone()));
}

#[test]
fn test_add_remove_operator() {
    let mut env = setup_app();
    let new_op = mk(&env.app, "newop");

    env.app.execute_contract(
        env.admin.clone(),
        env.contract.clone(),
        &ExecuteMsg::AddOperator {
            address: new_op.to_string(),
        },
        &[],
    )
    .unwrap();

    let config: ConfigResponse = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.operators.len(), 2);

    env.app.execute_contract(
        env.admin.clone(),
        env.contract.clone(),
        &ExecuteMsg::RemoveOperator {
            address: env.operator.to_string(),
        },
        &[],
    )
    .unwrap();

    let config: ConfigResponse = env.app
        .wrap()
        .query_wasm_smart(&env.contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.operators.len(), 1);
    assert_eq!(config.operators[0], new_op);
}

#[test]
fn test_builder_stats() {
    let mut env = setup_app();

    for _ in 0..2 {
        env.app.execute_contract(
            env.builder1.clone(),
            env.contract.clone(),
            &ExecuteMsg::SubmitWork {
                tier: GrantTier::ContractDeploy,
                evidence: "evidence".to_string(),
                work_hash: VALID_HASH.to_string(),
            },
            &[],
        )
        .unwrap();
    }

    let stats: BuilderStatsResponse = env.app
        .wrap()
        .query_wasm_smart(
            &env.contract,
            &QueryMsg::GetBuilderStats {
                address: env.builder1.to_string(),
            },
        )
        .unwrap();
    assert_eq!(stats.submissions.len(), 2);
    assert_eq!(stats.total_granted, 0);
}

#[test]
fn test_submissions_paused() {
    let mut env = setup_app();

    env.app.execute_contract(
        env.admin.clone(),
        env.contract.clone(),
        &ExecuteMsg::SetActive { active: false },
        &[],
    )
    .unwrap();

    let err = env.app
        .execute_contract(
            env.builder1.clone(),
            env.contract.clone(),
            &ExecuteMsg::SubmitWork {
                tier: GrantTier::ContractDeploy,
                evidence: "evidence".to_string(),
                work_hash: VALID_HASH.to_string(),
            },
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("paused"));
}
