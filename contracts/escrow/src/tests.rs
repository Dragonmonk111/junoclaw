use cosmwasm_std::{Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::LedgerStats;
use junoclaw_common::{ObligationStatus, PaymentObligation};

const UJUNO: &str = "ujuno";

fn store_and_instantiate(app: &mut App, admin: &Addr, task_ledger: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            task_ledger: task_ledger.to_string(),
            timeout_blocks: 100,
            denom: Some(UJUNO.to_string()),
        },
        &[],
        "payment-ledger",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn make_addr(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

fn authorize(app: &mut App, sender: &Addr, contract: &Addr, task_id: u64, payee: &Addr, amount: u128) {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Authorize {
            task_id,
            payee: payee.to_string(),
            amount: Uint128::from(amount),
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_obligations, 0);
    assert!(stats.total_pending.is_zero());
    assert!(stats.total_confirmed.is_zero());
}

#[test]
fn test_authorize_and_query() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.task_id, 1);
    assert_eq!(obligation.amount, Uint128::from(1_000_000u128));
    assert_eq!(obligation.status, ObligationStatus::Pending);
    assert_eq!(obligation.payer, payer);
    assert_eq!(obligation.payee, payee);
    assert!(obligation.attestation_hash.is_none());

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_obligations, 1);
    assert_eq!(stats.total_pending, Uint128::from(1_000_000u128));
}

#[test]
fn test_authorize_zero_amount_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Authorize {
                task_id: 1,
                payee: payee.to_string(),
                amount: Uint128::zero(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ZeroAmount {}));
}

#[test]
fn test_double_authorize_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Authorize {
                task_id: 1,
                payee: payee.to_string(),
                amount: Uint128::from(500_000u128),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AlreadyAuthorized { .. }));
}

#[test]
fn test_confirm_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Confirm { task_id: 1, tx_hash: Some("ABCDEF123".to_string()) },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Confirmed);
    assert!(obligation.settled_at.is_some());

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_confirmed, Uint128::from(1_000_000u128));
    assert!(stats.total_pending.is_zero());
}

#[test]
fn test_confirm_by_task_ledger() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let task_ledger = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &task_ledger);

    authorize(&mut app, &payer, &contract, 1, &payee, 2_000_000);

    app.execute_contract(
        task_ledger.clone(),
        contract.clone(),
        &ExecuteMsg::Confirm { task_id: 1, tx_hash: None },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Confirmed);
}

#[test]
fn test_dispute_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Dispute { task_id: 1, reason: "Task not completed".to_string() },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Disputed);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_disputed, Uint128::from(1_000_000u128));
    assert!(stats.total_pending.is_zero());
}

#[test]
fn test_dispute_unauthorized_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let stranger = make_addr(&app, "stranger");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            stranger.clone(),
            contract.clone(),
            &ExecuteMsg::Dispute { task_id: 1, reason: "fraud".to_string() },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_cancel_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Cancelled);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_cancelled, Uint128::from(1_000_000u128));
}

#[test]
fn test_cancel_by_admin() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Cancelled);
}

#[test]
fn test_confirm_not_pending_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    // Cancel first
    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    // Try to confirm a cancelled obligation
    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Confirm { task_id: 1, tx_hash: None },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotPending { .. }));
}

#[test]
fn test_attach_attestation() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AttachAttestation {
            task_id: 1,
            attestation_hash: "wavs_hash_abc123".to_string(),
        },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Verified);
    assert_eq!(obligation.attestation_hash, Some("wavs_hash_abc123".to_string()));
}

#[test]
fn test_attach_attestation_unauthorized_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::AttachAttestation {
                task_id: 1,
                attestation_hash: "fake".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_list_obligations() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = make_addr(&app, "taskledger");
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);
    authorize(&mut app, &payer, &contract, 2, &payee, 2_000_000);
    authorize(&mut app, &payer, &contract, 3, &payee, 3_000_000);

    let obligations: Vec<PaymentObligation> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListObligations { start_after: None, limit: Some(10) })
        .unwrap();
    assert_eq!(obligations.len(), 3);
    assert_eq!(obligations[0].task_id, 1);
    assert_eq!(obligations[2].task_id, 3);
}
