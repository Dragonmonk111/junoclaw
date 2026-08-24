use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::msg::{
    AdminResponse, ExecuteMsg, GetEnvelopeResponse, InstantiateMsg, QueryMsg,
    SafetyEnvelopeParams, VersionCountResponse,
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
        "safety-envelope",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn sample_params() -> SafetyEnvelopeParams {
    SafetyEnvelopeParams {
        max_speed_milli: 5000,
        max_force_milli: 50000,
        min_collision_distance_milli: 500,
        max_tilt_milli_degrees: 30000,
        max_acceleration_milli: 3000,
        human_proximity_allowed: true,
        max_arm_force_milli: 0,
        max_joint_torque_milli: 0,
    }
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
fn test_set_and_query_envelope() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let params = sample_params();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "delivery-bot-1".to_string(),
            params: params.clone(),
        },
        &[],
    )
    .unwrap();

    let resp: GetEnvelopeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetEnvelope {
                robot_id: "delivery-bot-1".to_string(),
            },
        )
        .unwrap();

    assert_eq!(resp.robot_id, "delivery-bot-1");
    assert_eq!(resp.params.max_speed_milli, 5000);
    assert_eq!(resp.params.max_force_milli, 50000);
    assert_eq!(resp.version, 1);
}

#[test]
fn test_version_increments() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let params = sample_params();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "robot-7".to_string(),
            params: params.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "robot-7".to_string(),
            params: params.clone(),
        },
        &[],
    )
    .unwrap();

    let resp: VersionCountResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetVersionCount {
                robot_id: "robot-7".to_string(),
            },
        )
        .unwrap();
    assert_eq!(resp.count, 2);

    let env_resp: GetEnvelopeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetEnvelope {
                robot_id: "robot-7".to_string(),
            },
        )
        .unwrap();
    assert_eq!(env_resp.version, 2);
}

#[test]
fn test_unauthorized_set() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let attacker = app.api().addr_make("attacker");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            attacker,
            contract.clone(),
            &ExecuteMsg::SetEnvelope {
                robot_id: "robot-1".to_string(),
                params: sample_params(),
            },
            &[],
        )
        .unwrap_err();
    eprintln!("DEBUG unauthorized err: {:?}", err);
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized") || err_str.contains("unauthorized"));
}

#[test]
fn test_tighten_envelope() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let params = sample_params();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "robot-1".to_string(),
            params: params.clone(),
        },
        &[],
    )
    .unwrap();

    let tighter = SafetyEnvelopeParams {
        max_speed_milli: 3000,
        ..params
    };
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TightenEnvelope {
            robot_id: "robot-1".to_string(),
            params: tighter,
        },
        &[],
    )
    .unwrap();

    let resp: GetEnvelopeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetEnvelope {
                robot_id: "robot-1".to_string(),
            },
        )
        .unwrap();
    assert_eq!(resp.params.max_speed_milli, 3000);
    assert_eq!(resp.version, 2);
}

#[test]
fn test_tighten_rejects_relax() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let params = sample_params();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "robot-1".to_string(),
            params: params.clone(),
        },
        &[],
    )
    .unwrap();

    let relaxed = SafetyEnvelopeParams {
        max_speed_milli: 10000,
        ..params
    };
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::TightenEnvelope {
                robot_id: "robot-1".to_string(),
                params: relaxed,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("stricter") || err_str.contains("InvalidParams"),
        "expected stricter error, got: {}",
        err_str
    );
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

    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::SetEnvelope {
                robot_id: "robot-1".to_string(),
                params: sample_params(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized") || err_str.contains("unauthorized"));

    app.execute_contract(
        new_admin,
        contract.clone(),
        &ExecuteMsg::SetEnvelope {
            robot_id: "robot-1".to_string(),
            params: sample_params(),
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_invalid_params() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let bad_params = SafetyEnvelopeParams {
        max_speed_milli: 0,
        max_force_milli: 50000,
        min_collision_distance_milli: 500,
        max_tilt_milli_degrees: 30000,
        max_acceleration_milli: 3000,
        human_proximity_allowed: true,
        max_arm_force_milli: 0,
        max_joint_torque_milli: 0,
    };

    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::SetEnvelope {
                robot_id: "robot-1".to_string(),
                params: bad_params,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("max_speed must be positive") || err_str.contains("InvalidParams"),
        "expected max_speed error, got: {}",
        err_str
    );
}

#[test]
fn test_envelope_not_found() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .wrap()
        .query_wasm_smart::<GetEnvelopeResponse>(
            &contract,
            &QueryMsg::GetEnvelope {
                robot_id: "nonexistent".to_string(),
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}
