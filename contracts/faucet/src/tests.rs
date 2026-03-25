use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::{
    ClaimStatusResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StatsResponse,
};

const DENOM: &str = "ujunox";
const DRIP: u128 = 100_000_000; // 100 JUNOX

fn mk(app: &App, label: &str) -> Addr { app.api().addr_make(label) }

fn setup_app() -> (App, Addr, Addr, Addr, Addr) {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user1 = mk(&app, "user1");
    let user2 = mk(&app, "user2");

    app.init_modules(|router, _api, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(10_000_000_000, DENOM))
            .unwrap();
    });

    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));

    let contract_addr = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                drip_amount: DRIP,
                denom: DENOM.to_string(),
            },
            &[],
            "faucet",
            Some(admin.to_string()),
        )
        .unwrap();

    // Fund the faucet with 1000 JUNOX
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Fund {},
        &coins(1_000_000_000, DENOM),
    )
    .unwrap();

    (app, contract_addr, admin, user1, user2)
}

#[test]
fn test_instantiate() {
    let (app, contract_addr, admin, _, _) = setup_app();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetConfig {})
        .unwrap();

    assert_eq!(resp.admin, admin);
    assert_eq!(resp.drip_amount, DRIP);
    assert_eq!(resp.denom, DENOM);
    assert!(resp.active);
    assert_eq!(resp.balance, 1_000_000_000);
}

#[test]
fn test_claim_first_time() {
    let (mut app, contract_addr, _, user1, _) = setup_app();

    let resp = app
        .execute_contract(
            user1.clone(),
            contract_addr.clone(),
            &ExecuteMsg::Claim {},
            &[],
        )
        .unwrap();

    let action = resp.events.iter().find_map(|e| {
        e.attributes
            .iter()
            .find(|a| a.key == "action" && a.value == "claim")
    });
    assert!(action.is_some());

    let balance = app.wrap().query_balance(&user1, DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(DRIP));

    let status: ClaimStatusResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasClaimed {
                address: user1.to_string(),
            },
        )
        .unwrap();
    assert!(status.claimed);

    let stats: StatsResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_claims, 1);
}

#[test]
fn test_claim_twice_rejected() {
    let (mut app, contract_addr, _, user1, _) = setup_app();

    app.execute_contract(
        user1.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            user1,
            contract_addr,
            &ExecuteMsg::Claim {},
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("already claimed"));
}

#[test]
fn test_multiple_users_claim() {
    let (mut app, contract_addr, _, user1, user2) = setup_app();

    app.execute_contract(
        user1,
        contract_addr.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    app.execute_contract(
        user2,
        contract_addr.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    let stats: StatsResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_claims, 2);
    assert_eq!(stats.balance, 800_000_000);
}

#[test]
fn test_faucet_pause() {
    let (mut app, contract_addr, admin, user1, _) = setup_app();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetActive { active: false },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            user1.clone(),
            contract_addr.clone(),
            &ExecuteMsg::Claim {},
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("paused"));

    app.execute_contract(
        admin,
        contract_addr.clone(),
        &ExecuteMsg::SetActive { active: true },
        &[],
    )
    .unwrap();

    app.execute_contract(
        user1,
        contract_addr,
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();
}

#[test]
fn test_unauthorized_admin_actions() {
    let (mut app, contract_addr, _, user1, _) = setup_app();

    let err = app
        .execute_contract(
            user1.clone(),
            contract_addr.clone(),
            &ExecuteMsg::SetActive { active: false },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("unauthorized"));

    let err = app
        .execute_contract(
            user1.clone(),
            contract_addr.clone(),
            &ExecuteMsg::Withdraw { amount: None },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("unauthorized"));

    let err = app
        .execute_contract(
            user1.clone(),
            contract_addr,
            &ExecuteMsg::TransferAdmin {
                new_admin: user1.to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("unauthorized"));
}

#[test]
fn test_withdraw() {
    let (mut app, contract_addr, admin, _, _) = setup_app();

    let before = app.wrap().query_balance(&admin, DENOM).unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Withdraw {
            amount: Some(500_000_000),
        },
        &[],
    )
    .unwrap();

    let after = app.wrap().query_balance(&admin, DENOM).unwrap();
    assert_eq!(
        after.amount.u128() - before.amount.u128(),
        500_000_000
    );

    let config: ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.balance, 500_000_000);
}

#[test]
fn test_transfer_admin() {
    let (mut app, contract_addr, admin, user1, _) = setup_app();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::TransferAdmin {
            new_admin: user1.to_string(),
        },
        &[],
    )
    .unwrap();

    let config: ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr.clone(), &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.admin, user1);

    let err = app
        .execute_contract(
            admin,
            contract_addr,
            &ExecuteMsg::SetActive { active: false },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("unauthorized"));
}

#[test]
fn test_insufficient_funds() {
    let mut app = App::default();
    let admin = mk(&app, "admin2");
    let user1 = mk(&app, "claimer");

    app.init_modules(|router, _api, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(50_000_000, DENOM))
            .unwrap();
    });

    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));

    let contract_addr = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                drip_amount: DRIP,
                denom: DENOM.to_string(),
            },
            &[],
            "faucet",
            Some(admin.to_string()),
        )
        .unwrap();

    app.execute_contract(
        admin,
        contract_addr.clone(),
        &ExecuteMsg::Fund {},
        &coins(50_000_000, DENOM),
    )
    .unwrap();

    let err = app
        .execute_contract(
            user1,
            contract_addr,
            &ExecuteMsg::Claim {},
            &[],
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("insufficient"));
}

#[test]
fn test_anyone_can_fund() {
    let (mut app, contract_addr, _, user1, _) = setup_app();

    app.init_modules(|router, _api, storage| {
        router
            .bank
            .init_balance(storage, &user1, coins(500_000_000, DENOM))
            .unwrap();
    });

    app.execute_contract(
        user1,
        contract_addr.clone(),
        &ExecuteMsg::Fund {},
        &coins(500_000_000, DENOM),
    )
    .unwrap();

    let config: ConfigResponse = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.balance, 1_500_000_000);
}

#[test]
fn test_zero_drip_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin3");

    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));

    let err = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                drip_amount: 0,
                denom: DENOM.to_string(),
            },
            &[],
            "faucet",
            Some(admin.to_string()),
        )
        .unwrap_err();

    assert!(err.root_cause().to_string().contains("invalid drip"));
}
