use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, EscrowStats, LeaseRequest, LeaseStatus};

const UJUNO: &str = "ujuno";

fn mk_addr(label: &str) -> Addr {
    cosmwasm_std::testing::MockApi::default().addr_make(label)
}

fn setup_app(requester: &Addr) -> App {
    let requester = requester.clone();
    App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &requester, coins(10_000_000, UJUNO))
            .unwrap();
    })
}

fn store_and_instantiate(app: &mut App, admin: &Addr, max_cost_per_lease: u128) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            denom: Some(UJUNO.to_string()),
            max_cost_per_lease: Uint128::new(max_cost_per_lease),
            min_timeout_secs: None,
            max_timeout_secs: None,
            moultbook: None,
            task_ledger: None,
        },
        &[],
        "emergency-compute-escrow",
        Some(admin.to_string()),
    )
    .unwrap()
}

struct Fixture {
    app: App,
    admin: Addr,
    requester: Addr,
    provider_payout: Addr,
    contract: Addr,
}

fn fixture() -> Fixture {
    let admin = mk_addr("admin");
    let requester = mk_addr("edge-agent-1");
    let provider_payout = mk_addr("akash-provider-payout");
    let mut app = setup_app(&requester);
    let contract = store_and_instantiate(&mut app, &admin, 2_000_000);
    Fixture { app, admin, requester, provider_payout, contract }
}

fn request_lease(f: &mut Fixture, max_cost: u128, timeout_secs: u64) -> u64 {
    f.app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::RequestLease {
                provider: "akash1provider".to_string(),
                task_id: Some(42),
                confidence_score: 35,
                max_cost: Uint128::new(max_cost),
                timeout_secs,
            },
            &coins(max_cost, UJUNO),
        )
        .unwrap();
    let stats: EscrowStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetStats {})
        .unwrap();
    stats.total_leases
}

#[test]
fn test_instantiate() {
    let f = fixture();
    let config: Config = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.admin, f.admin);
    assert_eq!(config.max_cost_per_lease, Uint128::new(2_000_000));
    assert_eq!(config.denom, UJUNO);
}

#[test]
fn test_request_lease_escrows_funds() {
    let mut f = fixture();
    let lease_id = request_lease(&mut f, 500_000, 120);
    assert_eq!(lease_id, 1);

    let lease: LeaseRequest = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetLease { lease_id: 1 })
        .unwrap();
    assert_eq!(lease.requester, f.requester);
    assert_eq!(lease.escrowed, Uint128::new(500_000));
    assert!(matches!(lease.status, LeaseStatus::Pending));

    let contract_balance = f.app.wrap().query_balance(&f.contract, UJUNO).unwrap();
    assert_eq!(contract_balance.amount, Uint128::new(500_000));
}

#[test]
fn test_request_lease_exceeds_cost_cap_fails() {
    let mut f = fixture();
    let err = f
        .app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::RequestLease {
                provider: "akash1provider".to_string(),
                task_id: None,
                confidence_score: 20,
                max_cost: Uint128::new(3_000_000),
                timeout_secs: 120,
            },
            &coins(3_000_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::CostCapExceeded { .. }));
}

#[test]
fn test_request_lease_wrong_funds_fails() {
    let mut f = fixture();
    let err = f
        .app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::RequestLease {
                provider: "akash1provider".to_string(),
                task_id: None,
                confidence_score: 20,
                max_cost: Uint128::new(500_000),
                timeout_secs: 120,
            },
            &coins(100_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::InsufficientFunds { .. }));
}

#[test]
fn test_request_lease_invalid_timeout_fails() {
    let mut f = fixture();
    let err = f
        .app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::RequestLease {
                provider: "akash1provider".to_string(),
                task_id: None,
                confidence_score: 20,
                max_cost: Uint128::new(500_000),
                timeout_secs: 5, // below default min_timeout_secs (30)
            },
            &coins(500_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::InvalidTimeout { .. }));
}

#[test]
fn test_confirm_and_complete_lease_pays_provider_and_refunds_remainder() {
    let mut f = fixture();
    request_lease(&mut f, 500_000, 120);

    f.app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::ConfirmLeaseActive { lease_id: 1 },
            &[],
        )
        .unwrap();

    let lease: LeaseRequest = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetLease { lease_id: 1 })
        .unwrap();
    assert!(matches!(lease.status, LeaseStatus::Active));

    f.app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::CompleteLease {
                lease_id: 1,
                actual_cost: Uint128::new(300_000),
                payout_addr: f.provider_payout.to_string(),
            },
            &[],
        )
        .unwrap();

    let lease: LeaseRequest = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetLease { lease_id: 1 })
        .unwrap();
    assert!(matches!(lease.status, LeaseStatus::Completed));
    assert_eq!(lease.actual_cost, Some(Uint128::new(300_000)));

    let provider_balance = f.app.wrap().query_balance(&f.provider_payout, UJUNO).unwrap();
    assert_eq!(provider_balance.amount, Uint128::new(300_000));

    let stats: EscrowStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_paid_to_providers, Uint128::new(300_000));
    assert_eq!(stats.total_refunded, Uint128::new(200_000));
}

#[test]
fn test_complete_lease_before_active_fails() {
    let mut f = fixture();
    request_lease(&mut f, 500_000, 120);

    let err = f
        .app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::CompleteLease {
                lease_id: 1,
                actual_cost: Uint128::new(100_000),
                payout_addr: f.provider_payout.to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotActive { .. }));
}

#[test]
fn test_cancel_lease_refunds_requester() {
    let mut f = fixture();
    request_lease(&mut f, 500_000, 120);

    let balance_before = f.app.wrap().query_balance(&f.requester, UJUNO).unwrap();

    f.app
        .execute_contract(
            f.requester.clone(),
            f.contract.clone(),
            &ExecuteMsg::CancelLease { lease_id: 1 },
            &[],
        )
        .unwrap();

    let lease: LeaseRequest = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetLease { lease_id: 1 })
        .unwrap();
    assert!(matches!(lease.status, LeaseStatus::Cancelled));

    let balance_after = f.app.wrap().query_balance(&f.requester, UJUNO).unwrap();
    assert_eq!(balance_after.amount, balance_before.amount + Uint128::new(500_000));
}

#[test]
fn test_expire_lease_before_deadline_fails() {
    let mut f = fixture();
    request_lease(&mut f, 500_000, 120);

    let err = f
        .app
        .execute_contract(
            f.admin.clone(),
            f.contract.clone(),
            &ExecuteMsg::ExpireLease { lease_id: 1 },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::DeadlineNotReached { .. }));
}

#[test]
fn test_expire_lease_after_deadline_refunds_fail_safe() {
    let mut f = fixture();
    request_lease(&mut f, 500_000, 60);

    f.app.update_block(|block| {
        block.time = block.time.plus_seconds(61);
    });

    let balance_before = f.app.wrap().query_balance(&f.requester, UJUNO).unwrap();

    // Permissionless: anyone (here, an unrelated address) can trigger expiry.
    let bystander = mk_addr("watchdog-relayer");
    f.app
        .execute_contract(
            bystander,
            f.contract.clone(),
            &ExecuteMsg::ExpireLease { lease_id: 1 },
            &[],
        )
        .unwrap();

    let lease: LeaseRequest = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetLease { lease_id: 1 })
        .unwrap();
    assert!(matches!(lease.status, LeaseStatus::Expired));

    let balance_after = f.app.wrap().query_balance(&f.requester, UJUNO).unwrap();
    assert_eq!(balance_after.amount, balance_before.amount + Uint128::new(500_000));

    let stats: EscrowStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_expired, 1);
}

#[test]
fn test_list_leases_by_requester() {
    let mut f = fixture();
    request_lease(&mut f, 200_000, 120);
    request_lease(&mut f, 300_000, 120);

    let leases: Vec<LeaseRequest> = f
        .app
        .wrap()
        .query_wasm_smart(
            &f.contract,
            &QueryMsg::ListLeasesByRequester {
                requester: f.requester.to_string(),
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(leases.len(), 2);
}
