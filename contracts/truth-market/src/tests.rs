use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

const UJUNO: &str = "ujuno";

fn setup_app() -> App {
    App::new(|router, _, storage| {
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("admin"),
                coins(10_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("op1"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("opA"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("opB"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("opC"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("op2"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("op3"),
                coins(5_000_000, UJUNO),
            )
            .unwrap();
    })
}

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));

    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            min_stake: Uint128::from(1_000_000u128),
            slash_percent: 10,
            reward_percent: 80,
            denom: UJUNO.to_string(),
            unstake_cooldown_secs: 86400,
            min_operators: None,
        },
        &[],
        "truth-market",
        None,
    )
    .unwrap()
}

fn make_addr(_app: &App, label: &str) -> Addr {
    Addr::unchecked(label)
}

#[test]
fn test_instantiate() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let config: crate::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();

    assert_eq!(config.admin, admin.to_string());
    assert_eq!(config.min_stake, Uint128::from(1_000_000u128));
    assert_eq!(config.slash_percent, 10);
    assert_eq!(config.reward_percent, 80);
    assert_eq!(config.denom, UJUNO);
    assert_eq!(config.min_operators, 3);

    let stats: crate::msg::StatsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_operators, 0);
    assert_eq!(stats.reward_pool, Uint128::zero());
}

#[test]
fn test_register_operator() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    let op: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: operator.to_string() })
        .unwrap();
    assert_eq!(op.stake, Uint128::from(1_000_000u128));
    assert!(op.active);
    assert_eq!(op.correct_verdicts, 0);

    let stats: crate::msg::StatsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_operators, 1);
    assert_eq!(stats.active_operators, 1);
    assert_eq!(stats.total_staked, Uint128::from(1_000_000u128));
}

#[test]
fn test_register_insufficient_stake() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    let err = app
        .execute_contract(
            operator,
            contract,
            &ExecuteMsg::RegisterOperator { fingerprint: None },
            &coins(500_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::InsufficientStake { .. }
    ));
}

#[test]
fn test_register_duplicate() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    let err = app
        .execute_contract(
            operator,
            contract,
            &ExecuteMsg::RegisterOperator { fingerprint: None },
            &coins(1_000_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::AlreadyRegistered { .. }
    ));
}

#[test]
fn test_submit_verdict() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 42,
            verdict: "green".to_string(),
            messages_hash: "abcd1234".to_string(),
        },
        &[],
    )
    .unwrap();

    let verdict: crate::state::VerdictRecord = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetVerdict {
                batch_height: 42,
                operator: operator.to_string(),
            },
        )
        .unwrap();
    assert_eq!(verdict.verdict, "green");
    assert_eq!(verdict.batch_height, 42);
}

#[test]
fn test_submit_verdict_not_registered() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    let err = app
        .execute_contract(
            operator,
            contract,
            &ExecuteMsg::SubmitVerdict {
                batch_height: 1,
                verdict: "green".to_string(),
                messages_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::OperatorNotFound { .. }
    ));
}

#[test]
fn test_submit_verdict_invalid() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    let err = app
        .execute_contract(
            operator,
            contract,
            &ExecuteMsg::SubmitVerdict {
                batch_height: 1,
                verdict: "purple".to_string(),
                messages_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::InvalidVerdict { .. }
    ));
}

#[test]
fn test_finalize_epoch_rewards_and_slashes() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // Register 3 operators
    let op_a = make_addr(&app, "opA");
    let op_b = make_addr(&app, "opB");
    let op_c = make_addr(&app, "opC");

    for op in [&op_a, &op_b, &op_c] {
        app.execute_contract(
            op.clone(),
            contract.clone(),
            &ExecuteMsg::RegisterOperator { fingerprint: None },
            &coins(1_000_000, UJUNO),
        )
        .unwrap();
    }

    // Deposit rewards into pool
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DepositRewards {},
        &coins(300_000, UJUNO),
    )
    .unwrap();

    // Submit verdicts: A and B say green, C says red
    app.execute_contract(
        op_a.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 100,
            verdict: "green".to_string(),
            messages_hash: "hash100".to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        op_b.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 100,
            verdict: "green".to_string(),
            messages_hash: "hash100".to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        op_c.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 100,
            verdict: "red".to_string(),
            messages_hash: "hash100".to_string(),
        },
        &[],
    )
    .unwrap();

    // Finalize epoch — consensus is green
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::FinalizeEpoch {
            batch_height: 100,
            consensus_verdict: "green".to_string(),
            messages_hash: "hash100".to_string(),
        },
        &[],
    )
    .unwrap();

    // Check epoch result
    let epoch: crate::msg::EpochResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEpoch { batch_height: 100 })
        .unwrap();
    assert!(epoch.finalized);
    assert_eq!(epoch.total_operators, 3);
    assert_eq!(epoch.matching_operators, 2);
    assert_eq!(epoch.diverging_operators, 1);

    // Check operator A got rewarded
    let op_a_resp: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: op_a.to_string() })
        .unwrap();
    assert_eq!(op_a_resp.correct_verdicts, 1);
    assert_eq!(op_a_resp.incorrect_verdicts, 0);
    assert!(op_a_resp.total_rewards > Uint128::zero());

    // Check operator C got slashed
    let op_c_resp: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: op_c.to_string() })
        .unwrap();
    assert_eq!(op_c_resp.correct_verdicts, 0);
    assert_eq!(op_c_resp.incorrect_verdicts, 1);
    assert!(op_c_resp.total_slashed > Uint128::zero());
    // 10% slash of 1_000_000 = 100_000
    assert_eq!(op_c_resp.total_slashed, Uint128::from(100_000u128));
    assert_eq!(op_c_resp.stake, Uint128::from(900_000u128));
}

#[test]
fn test_finalize_epoch_unauthorized() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 1,
            verdict: "green".to_string(),
            messages_hash: "hash".to_string(),
        },
        &[],
    )
    .unwrap();

    // Non-admin tries to finalize
    let err = app
        .execute_contract(
            operator,
            contract,
            &ExecuteMsg::FinalizeEpoch {
                batch_height: 1,
                consensus_verdict: "green".to_string(),
                messages_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(contract_err, crate::error::ContractError::Unauthorized {}));
}

#[test]
fn test_finalize_epoch_no_verdicts() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            admin,
            contract,
            &ExecuteMsg::FinalizeEpoch {
                batch_height: 999,
                consensus_verdict: "green".to_string(),
                messages_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::NoVerdicts { .. }
    ));
}

#[test]
fn test_deactivate_and_reactivate() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    // Deactivate
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::Deactivate {},
        &[],
    )
    .unwrap();

    let op: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: operator.to_string() })
        .unwrap();
    assert!(!op.active);

    // Reactivate
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::Reactivate {},
        &[],
    )
    .unwrap();

    let op: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: operator.to_string() })
        .unwrap();
    assert!(op.active);
}

#[test]
fn test_deposit_rewards() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DepositRewards {},
        &coins(500_000, UJUNO),
    )
    .unwrap();

    let pool: Uint128 = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetRewardPool {})
        .unwrap();
    assert_eq!(pool, Uint128::from(500_000u128));
}

#[test]
fn test_list_operators() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    for label in ["op1", "op2", "op3"] {
        let op = make_addr(&app, label);
        app.execute_contract(
            op,
            contract.clone(),
            &ExecuteMsg::RegisterOperator { fingerprint: None },
            &coins(1_000_000, UJUNO),
        )
        .unwrap();
    }

    let resp: crate::msg::OperatorsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListOperators {})
        .unwrap();
    assert_eq!(resp.operators.len(), 3);
}

#[test]
fn test_finalize_epoch_insufficient_operators() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // Register only 1 operator (below default min_operators=3)
    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 1,
            verdict: "green".to_string(),
            messages_hash: "hash".to_string(),
        },
        &[],
    )
    .unwrap();

    // Admin tries to finalize with only 1 operator — should fail
    let err = app
        .execute_contract(
            admin,
            contract,
            &ExecuteMsg::FinalizeEpoch {
                batch_height: 1,
                consensus_verdict: "green".to_string(),
                messages_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<crate::error::ContractError>().unwrap();
    assert!(matches!(
        contract_err,
        crate::error::ContractError::InsufficientOperators { required: 3, submitted: 1 }
    ));
}

#[test]
fn test_finalize_epoch_min_operators_configurable() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");

    // Instantiate with min_operators = 1
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));
    let contract = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                min_stake: Uint128::from(1_000_000u128),
                slash_percent: 10,
                reward_percent: 80,
                denom: UJUNO.to_string(),
                unstake_cooldown_secs: 86400,
                min_operators: Some(1),
            },
            &[],
            "truth-market",
            None,
        )
        .unwrap();

    // Register 1 operator
    let operator = make_addr(&app, "op1");
    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    app.execute_contract(
        operator.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitVerdict {
            batch_height: 1,
            verdict: "green".to_string(),
            messages_hash: "hash".to_string(),
        },
        &[],
    )
    .unwrap();

    // Finalize with 1 operator should succeed when min_operators=1
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::FinalizeEpoch {
            batch_height: 1,
            consensus_verdict: "green".to_string(),
            messages_hash: "hash".to_string(),
        },
        &[],
    )
    .unwrap();

    let epoch: crate::msg::EpochResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEpoch { batch_height: 1 })
        .unwrap();
    assert!(epoch.finalized);
    assert_eq!(epoch.total_operators, 1);
}

#[test]
fn test_update_config_min_operators() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // Update min_operators to 5
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::UpdateConfig {
            min_stake: None,
            slash_percent: None,
            reward_percent: None,
            unstake_cooldown_secs: None,
            min_operators: Some(5),
        },
        &[],
    )
    .unwrap();

    let config: crate::msg::ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.min_operators, 5);
}

#[test]
fn test_register_with_fingerprint() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let op_a = make_addr(&app, "opA");
    let op_b = make_addr(&app, "opB");

    let fp = "qwen25-14b-host01".to_string();

    app.execute_contract(
        op_a.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: Some(fp.clone()) },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    app.execute_contract(
        op_b.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: Some(fp.clone()) },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    // Query operator should return fingerprint
    let resp_a: crate::msg::OperatorResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetOperator { address: op_a.to_string() })
        .unwrap();
    assert_eq!(resp_a.fingerprint, Some(fp.clone()));

    // Query fingerprints should show count=2 for this fingerprint
    let fps: crate::msg::FingerprintsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetFingerprints {})
        .unwrap();
    assert_eq!(fps.fingerprints.len(), 1);
    assert_eq!(fps.fingerprints[0].fingerprint, fp);
    assert_eq!(fps.fingerprints[0].operator_count, 2);
    assert_eq!(fps.operators_without_fingerprint, 0);
}

#[test]
fn test_fingerprints_mixed_and_none() {
    let mut app = setup_app();
    let admin = make_addr(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // op1 with fingerprint
    let op1 = make_addr(&app, "op1");
    app.execute_contract(
        op1,
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: Some("llama31-70b-gpu0".to_string()) },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    // op2 without fingerprint
    let op2 = make_addr(&app, "op2");
    app.execute_contract(
        op2,
        contract.clone(),
        &ExecuteMsg::RegisterOperator { fingerprint: None },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    let fps: crate::msg::FingerprintsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetFingerprints {})
        .unwrap();
    assert_eq!(fps.fingerprints.len(), 1);
    assert_eq!(fps.fingerprints[0].operator_count, 1);
    assert_eq!(fps.operators_without_fingerprint, 1);
}
