use cosmwasm_std::{Addr, Empty};
use cw_multi_test::{App, ContractWrapper, Executor, IntoAddr};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{
    ExecuteMsg, InstantiateMsg, ListMembersResponse, MayoPkHashResponse, MemberResponse, MigrateMsg,
    QueryMsg, SunsetStatusResponse, TotalWeightResponse,
};
use crate::state::MemberRole;

fn contract() -> impl cw_multi_test::Contract<Empty> {
    ContractWrapper::new(execute, instantiate, query).with_migrate(migrate)
}

fn setup_app() -> (App, Addr, Addr, Addr) {
    let mut app = App::default();
    let code_id = app.store_code(Box::new(contract()));
    let owner = "owner".into_addr();
    let genesis = "genesis".into_addr();

    let contract = app
        .instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                admin: Some(owner.to_string()),
                genesis: Some(genesis.to_string()),
                sunset_grace_seconds: 10,
            },
            &[],
            "jclaw-credential",
            None,
        )
        .unwrap();

    (app, contract, owner, genesis)
}

// ═════════════════════════════════════════════════════════════════════════════
// Instantiation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn instantiate_creates_genesis_with_full_weight() {
    let (app, contract, _owner, genesis) = setup_app();

    let resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();

    assert_eq!(resp.addr, genesis.to_string());
    assert_eq!(resp.weight, 10_000);
    assert_eq!(resp.role, MemberRole::Genesis);
    assert_eq!(resp.parent, None);
    assert_eq!(resp.depth, 0);
}

#[test]
fn instantiate_sets_total_weight() {
    let (app, contract, _owner, genesis) = setup_app();

    let resp: TotalWeightResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::TotalWeight {})
        .unwrap();

    assert_eq!(resp.weight, 10_000);
}

// ═════════════════════════════════════════════════════════════════════════════
// Bud
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn bud_splits_weight_from_parent() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let genesis_resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();
    assert_eq!(genesis_resp.weight, 7_000);

    let bud1: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: "bud1".into_addr().to_string() })
        .unwrap();
    assert_eq!(bud1.weight, 3_000);
    assert_eq!(bud1.role, MemberRole::Bud);
    assert_eq!(bud1.parent, Some(genesis.to_string()));
    assert_eq!(bud1.depth, 1);
}

#[test]
fn bud_chain_creates_depth() {
    let (mut app, contract, owner, genesis) = setup_app();

    // Genesis -> Bud1 -> Bud2
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 5_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: "bud1".into_addr().to_string(),
            child: "bud2".into_addr().to_string(),
            child_weight: 2_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let bud2: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: "bud2".into_addr().to_string() })
        .unwrap();
    assert_eq!(bud2.depth, 2);
    assert_eq!(bud2.parent, Some("bud1".into_addr().to_string()));
}

#[test]
fn bud_rejects_duplicate() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Duplicate member"));
}

#[test]
fn bud_rejects_insufficient_parent_weight() {
    let (mut app, contract, owner, genesis) = setup_app();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 20_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Invalid weights"));
}

#[test]
fn bud_rejects_non_admin() {
    let (mut app, contract, _owner, genesis) = setup_app();
    let rando = "rando".into_addr();

    let err = app
        .execute_contract(
            rando,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}

// ═════════════════════════════════════════════════════════════════════════════
// BreakChannel
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn break_channel_prunes_subtree_and_returns_weight_to_genesis() {
    let (mut app, contract, owner, genesis) = setup_app();

    // Genesis (10k) -> Bud1 (3k) -> Bud2 (1k)
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: "bud1".into_addr().to_string(),
            child: "bud2".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    // Break bud1 (should also remove bud2)
    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::BreakChannel {
            addr: "bud1".into_addr().to_string(),
        },
        &[],
    )
    .unwrap();

    // bud1 and bud2 gone
    let err = app
        .wrap()
        .query_wasm_smart::<MemberResponse>(&contract, &QueryMsg::Member { addr: "bud1".into_addr().to_string() });
    assert!(err.is_err());

    let err = app
        .wrap()
        .query_wasm_smart::<MemberResponse>(&contract, &QueryMsg::Member { addr: "bud2".into_addr().to_string() });
    assert!(err.is_err());

    // Genesis regained 3k + 1k = 4k -> back to 10k
    let genesis_resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();
    assert_eq!(genesis_resp.weight, 10_000);
}

#[test]
fn break_channel_rejects_root() {
    let (mut app, contract, owner, genesis) = setup_app();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::BreakChannel {
                addr: genesis.to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Cannot break channel on root member"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Sunset
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn sunset_blocked_if_members_have_children() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::InitiateSunset {},
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("has not passed their bud"));
}

#[test]
fn sunset_success_when_all_leaves() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::InitiateSunset {},
        &[],
    )
    .unwrap();

    let status: SunsetStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::SunsetStatus {})
        .unwrap();
    assert!(status.initiated);
    assert!(!status.executed);
}

#[test]
fn execute_sunset_after_grace_period() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::InitiateSunset {},
        &[],
    )
    .unwrap();

    // Too early
    let err = app
        .execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::ExecuteSunset {},
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Sunset grace period"));

    // Advance blocks past grace period (10 blocks)
    app.update_block(|b| b.height += 20);

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::ExecuteSunset {},
        &[],
    )
    .unwrap();

    let status: SunsetStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::SunsetStatus {})
        .unwrap();
    assert!(status.executed);
}

// ═════════════════════════════════════════════════════════════════════════════
// ═════════════════════════════════════════════════════════════════════════════
// MAYO post-quantum attestation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn mayo_bud_stores_pk_hash() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    let resp: crate::msg::MayoPkHashResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::MayoPkHash {
                addr: "bud_mayo".into_addr().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        resp.mayo_pk_hash,
        Some(crate::mayo_vectors::PK_HASH.to_string())
    );
}

#[test]
fn mayo_verify_valid_signature() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        "bud_mayo".into_addr(),
        contract,
        &ExecuteMsg::VerifyMayoAttestation {
            addr: "bud_mayo".into_addr().to_string(),
            message: crate::mayo_vectors::MSG.to_vec(),
            signature: sig,
            public_key: pk,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mayo_verify_invalid_signature() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            "bud_mayo".into_addr(),
            contract,
            &ExecuteMsg::VerifyMayoAttestation {
                addr: "bud_mayo".into_addr().to_string(),
                message: b"tampered message".to_vec(),
                signature: sig,
                public_key: pk,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("MAYO signature verification failed"));
}

#[test]
fn mayo_verify_hash_mismatch() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    // Mutate the last byte of the public key so the hash no longer matches
    let mut bad_pk = pk.clone();
    let last = bad_pk.len() - 1;
    bad_pk[last] = bad_pk[last].wrapping_add(1);

    let err = app
        .execute_contract(
            "bud_mayo".into_addr(),
            contract,
            &ExecuteMsg::VerifyMayoAttestation {
                addr: "bud_mayo".into_addr().to_string(),
                message: crate::mayo_vectors::MSG.to_vec(),
                signature: sig,
                public_key: bad_pk,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("MAYO public key hash mismatch"));
}

// ═════════════════════════════════════════════════════════════════════════════
// ListMembers & TotalWeight (cw4-compatible)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn list_members_paginates() {
    let (mut app, contract, owner, genesis) = setup_app();

    for i in 1..=5 {
        let label = format!("bud{}", i);
        app.execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: label.as_str().into_addr().to_string(),
                child_weight: 100,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap();
    }

    let resp: ListMembersResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListMembers { start_after: None, limit: Some(3) })
        .unwrap();
    assert_eq!(resp.members.len(), 3);
}

#[test]
fn total_weight_unchanged_after_bud() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let resp: TotalWeightResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::TotalWeight {})
        .unwrap();
    assert_eq!(resp.weight, 10_000);
}

// ═════════════════════════════════════════════════════════════════════════════
// Admin transfer
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn transfer_admin_works() {
    let (mut app, contract, owner, genesis) = setup_app();
    let new_admin = "new_admin".into_addr();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::TransferAdmin {
            new_admin: new_admin.to_string(),
        },
        &[],
    )
    .unwrap();

    // Old owner can no longer bud
    let err = app
        .execute_contract(
            "owner".into_addr(),
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}
