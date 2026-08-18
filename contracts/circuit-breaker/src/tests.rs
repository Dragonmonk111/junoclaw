use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::msg::{
    AdminResponse, ExecuteMsg, GetBreakerResponse, InstantiateMsg, IsLockedResponse, QueryMsg,
};

fn setup_contract(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));

    let msg = InstantiateMsg {
        admin: admin.to_string(),
    };
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &msg,
        &[],
        "circuit-breaker",
        Some(admin.to_string()),
    )
    .unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let resp: AdminResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAdmin {})
        .unwrap();
    assert_eq!(resp.admin, admin.to_string());
}

#[test]
fn test_trip_and_query_breaker() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TripBreaker {
            robot_id: "robot-1".to_string(),
            reason: "safety invariant violated: max_speed exceeded".to_string(),
            cause_ref: "attestation-0xabc123".to_string(),
        },
        &[],
    )
    .unwrap();

    let resp: GetBreakerResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetBreaker {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();

    assert_eq!(resp.robot_id, "robot-1");
    assert_eq!(resp.state, "tripped");
    assert_eq!(
        resp.reason.unwrap(),
        "safety invariant violated: max_speed exceeded"
    );
    assert!(resp.tripped_at.is_some());
    assert_eq!(resp.cause_ref.unwrap(), "attestation-0xabc123");
}

#[test]
fn test_is_locked() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    // Not locked initially (no breaker record)
    let resp: IsLockedResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::IsLocked {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();
    assert!(!resp.is_locked);

    // Trip it
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TripBreaker {
            robot_id: "robot-1".to_string(),
            reason: "collision avoidance failed".to_string(),
            cause_ref: "attestation-0xdef456".to_string(),
        },
        &[],
    )
    .unwrap();

    // Now locked
    let resp: IsLockedResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::IsLocked {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();
    assert!(resp.is_locked);
    assert_eq!(resp.reason.unwrap(), "collision avoidance failed");
}

#[test]
fn test_reset_breaker() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    // Trip
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TripBreaker {
            robot_id: "robot-1".to_string(),
            reason: "tilt exceeded".to_string(),
            cause_ref: "attestation-0x001".to_string(),
        },
        &[],
    )
    .unwrap();

    // Reset
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ResetBreaker {
            robot_id: "robot-1".to_string(),
            reset_by: "operator-alice".to_string(),
        },
        &[],
    )
    .unwrap();

    let resp: GetBreakerResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetBreaker {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();

    assert_eq!(resp.state, "reset");
    assert!(resp.reset_at.is_some());
    assert_eq!(resp.reset_by.unwrap(), "operator-alice");
    // Original trip data preserved
    assert_eq!(resp.reason.unwrap(), "tilt exceeded");

    // Not locked after reset
    let locked: IsLockedResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::IsLocked {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();
    assert!(!locked.is_locked);
}

#[test]
fn test_unauthorized_trip() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let attacker = app.api().addr_make("attacker");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            attacker,
            contract.clone(),
            &ExecuteMsg::TripBreaker {
                robot_id: "robot-1".to_string(),
                reason: "malicious trip".to_string(),
                cause_ref: "fake".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized") || err_str.contains("unauthorized"));
}

#[test]
fn test_double_trip_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TripBreaker {
            robot_id: "robot-1".to_string(),
            reason: "first violation".to_string(),
            cause_ref: "ref-1".to_string(),
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::TripBreaker {
                robot_id: "robot-1".to_string(),
                reason: "second violation".to_string(),
                cause_ref: "ref-2".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("AlreadyTripped") || err_str.contains("already tripped"));
}

#[test]
fn test_reset_not_tripped_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    // Try to reset without tripping first
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::ResetBreaker {
                robot_id: "robot-1".to_string(),
                reset_by: "operator".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("BreakerNotFound") || err_str.contains("not found"));
}

#[test]
fn test_transfer_admin() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let new_admin = app.api().addr_make("new_gov");
    let contract = setup_contract(&mut app, &admin);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TransferAdmin {
            new_admin: new_admin.to_string(),
        },
        &[],
    )
    .unwrap();

    let resp: AdminResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAdmin {})
        .unwrap();
    assert_eq!(resp.admin, new_admin.to_string());

    // Old admin can't trip
    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::TripBreaker {
                robot_id: "robot-1".to_string(),
                reason: "test".to_string(),
                cause_ref: "ref".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized") || err_str.contains("unauthorized"));

    // New admin can
    app.execute_contract(
        new_admin,
        contract.clone(),
        &ExecuteMsg::TripBreaker {
            robot_id: "robot-1".to_string(),
            reason: "test".to_string(),
            cause_ref: "ref".to_string(),
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_empty_reason_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::TripBreaker {
                robot_id: "robot-1".to_string(),
                reason: "".to_string(),
                cause_ref: "ref".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("InvalidParams") || err_str.contains("empty"));
}

#[test]
fn test_breaker_not_found_query() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .wrap()
        .query_wasm_smart::<GetBreakerResponse>(
            &contract,
            &QueryMsg::GetBreaker {
                robot_id: "nonexistent".to_string(),
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}
