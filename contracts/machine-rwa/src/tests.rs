use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{
    ExecuteMsg, FractionalOwnersResponse, InstantiateMsg, MachinesResponse, OwnerFractionResponse,
    QueryMsg,
};
use crate::state::{Config, Machine};

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: admin.to_string(),
            moultbook_contract: None,
        },
        &[],
        "machine-rwa",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn mint_machine(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    model: &str,
    serial: &str,
) -> String {
    let resp = app
        .execute_contract(
            sender.clone(),
            contract.clone(),
            &ExecuteMsg::Mint {
                model: model.to_string(),
                serial_number: serial.to_string(),
                sensor_suite: "LiDAR+IMU+stereo".to_string(),
                ipfs_metadata: "ipfs://test".to_string(),
                moultbook_author: sender.to_string(),
            },
            &[],
        )
        .unwrap();

    for ev in &resp.events {
        for attr in &ev.attributes {
            if attr.key == "token_id" {
                return attr.value.clone();
            }
        }
    }
    panic!("no token_id in response");
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin);
    assert!(cfg.moultbook_contract.is_none());
}

#[test]
fn test_mint() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Unitree Go2", "SN-001");

    let machine: Machine = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetMachine {
                token_id: token_id.clone(),
            },
        )
        .unwrap();
    assert_eq!(machine.token_id, "machine-0");
    assert_eq!(machine.model, "Unitree Go2");
    assert_eq!(machine.serial_number, "SN-001");
    assert_eq!(machine.minter, alice);
    assert!(!machine.burned);

    // Alice should own 10000 BP
    let frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id: token_id.clone(),
                owner: alice.to_string(),
            },
        )
        .unwrap();
    assert_eq!(frac.basis_points, 10_000);
}

#[test]
fn test_transfer() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Spot", "SN-002");

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::Transfer {
            token_id: token_id.clone(),
            to: bob.to_string(),
        },
        &[],
    )
    .unwrap();

    let alice_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id: token_id.clone(),
                owner: alice.to_string(),
            },
        )
        .unwrap();
    assert_eq!(alice_frac.basis_points, 0);

    let bob_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id,
                owner: bob.to_string(),
            },
        )
        .unwrap();
    assert_eq!(bob_frac.basis_points, 10_000);
}

#[test]
fn test_fractionalize() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let carol = app.api().addr_make("carol");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-003");

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::Fractionalize {
            token_id: token_id.clone(),
            recipients: vec![
                (alice.to_string(), 4000),
                (bob.to_string(), 3500),
                (carol.to_string(), 2500),
            ],
        },
        &[],
    )
    .unwrap();

    let ownership: FractionalOwnersResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnership {
                token_id: token_id.clone(),
            },
        )
        .unwrap();
    assert_eq!(ownership.owners.len(), 3);

    let alice_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id: token_id.clone(),
                owner: alice.to_string(),
            },
        )
        .unwrap();
    assert_eq!(alice_frac.basis_points, 4000);

    let bob_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id: token_id.clone(),
                owner: bob.to_string(),
            },
        )
        .unwrap();
    assert_eq!(bob_frac.basis_points, 3500);

    let carol_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id,
                owner: carol.to_string(),
            },
        )
        .unwrap();
    assert_eq!(carol_frac.basis_points, 2500);
}

#[test]
fn test_fractionalize_invalid_sum() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-004");

    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::Fractionalize {
                token_id,
                recipients: vec![(bob.to_string(), 5000)],
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("InvalidBasisPointsSum") || err_str.contains("10000"));
}

#[test]
fn test_transfer_fraction() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-005");

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::TransferFraction {
            token_id: token_id.clone(),
            to: bob.to_string(),
            basis_points: 3000,
        },
        &[],
    )
    .unwrap();

    let alice_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id: token_id.clone(),
                owner: alice.to_string(),
            },
        )
        .unwrap();
    assert_eq!(alice_frac.basis_points, 7000);

    let bob_frac: OwnerFractionResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetOwnerFraction {
                token_id,
                owner: bob.to_string(),
            },
        )
        .unwrap();
    assert_eq!(bob_frac.basis_points, 3000);
}

#[test]
fn test_transfer_fraction_insufficient() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-006");

    // Alice gives bob 3000 BP
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::TransferFraction {
            token_id: token_id.clone(),
            to: bob.to_string(),
            basis_points: 3000,
        },
        &[],
    )
    .unwrap();

    // Bob tries to transfer 5000 BP (only owns 3000)
    let err = app
        .execute_contract(
            bob.clone(),
            contract,
            &ExecuteMsg::TransferFraction {
                token_id,
                to: alice.to_string(),
                basis_points: 5000,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("InsufficientFraction") || err_str.contains("5000"));
}

#[test]
fn test_burn_admin_only() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-007");

    // Alice (not admin) cannot burn
    let err = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::Burn {
                token_id: token_id.clone(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized"));

    // Admin can burn
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Burn {
            token_id: token_id.clone(),
        },
        &[],
    )
    .unwrap();

    let machine: Machine = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetMachine {
                token_id,
            },
        )
        .unwrap();
    assert!(machine.burned);
}

#[test]
fn test_list_machines() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    mint_machine(&mut app, &contract, &alice, "Go2", "SN-A");
    mint_machine(&mut app, &contract, &alice, "Spot", "SN-B");
    mint_machine(&mut app, &contract, &alice, "UR5", "SN-C");

    let resp: MachinesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListMachines {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(resp.machines.len(), 3);
}

#[test]
fn test_list_by_owner() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    mint_machine(&mut app, &contract, &alice, "Go2", "SN-A");
    mint_machine(&mut app, &contract, &bob, "Spot", "SN-B");

    let alice_machines: MachinesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByOwner {
                owner: alice.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(alice_machines.machines.len(), 1);

    let bob_machines: MachinesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByOwner {
                owner: bob.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(bob_machines.machines.len(), 1);
}

#[test]
fn test_mint_empty_model_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::Mint {
                model: "".to_string(),
                serial_number: "SN-X".to_string(),
                sensor_suite: "none".to_string(),
                ipfs_metadata: "ipfs://x".to_string(),
                moultbook_author: "someone".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("EmptyModel") || err_str.contains("model must not be empty"),
        "expected EmptyModel error, got: {}",
        err_str
    );
}

#[test]
fn test_transfer_not_full_owner() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let carol = app.api().addr_make("carol");
    let contract = store_and_instantiate(&mut app, &admin);

    let token_id = mint_machine(&mut app, &contract, &alice, "Go2", "SN-008");

    // Alice fractionalizes to alice(60%) + bob(40%)
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::Fractionalize {
            token_id: token_id.clone(),
            recipients: vec![
                (alice.to_string(), 6000),
                (bob.to_string(), 4000),
            ],
        },
        &[],
    )
    .unwrap();

    // Alice (60% owner) cannot Transfer (requires 100%)
    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::Transfer {
                token_id,
                to: carol.to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("NotFullOwner") || err_str.contains("6000"));
}
