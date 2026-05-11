use cosmwasm_std::{Addr, Binary};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{EntriesResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, MoultEntry, Stats, Visibility};

const MAX_SIZE: u64 = 1_048_576;
const MAX_REFS: u32 = 8;
const MAX_CONTENT_TYPE: u32 = 64;

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: admin.to_string(),
            whoami_contract: None,
            max_size_bytes: MAX_SIZE,
            max_refs: MAX_REFS,
            max_content_type_len: MAX_CONTENT_TYPE,
        },
        &[],
        "moultbook-v0",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn make_commitment(byte: u8) -> Binary {
    Binary::from(vec![byte; 32])
}

fn post_entry(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    commitment_byte: u8,
    refs: Vec<String>,
) -> Result<cw_multi_test::AppResponse, anyhow::Error> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Post {
            commitment: make_commitment(commitment_byte),
            content_type: "text/markdown".to_string(),
            size_bytes: 256,
            attestation_ref: None,
            visibility: Visibility::Public,
            refs,
        },
        &[],
    )
}

/// Pull the entry id out of the post response's "id" attribute.
fn extract_id(resp: &cw_multi_test::AppResponse) -> String {
    for ev in &resp.events {
        for attr in &ev.attributes {
            if attr.key == "id" {
                return attr.value.clone();
            }
        }
    }
    panic!("no id attribute in response");
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
    assert!(cfg.whoami_contract.is_none());
    assert_eq!(cfg.max_size_bytes, MAX_SIZE);

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.total_active, 0);
    assert_eq!(stats.total_redacted, 0);
}

#[test]
fn test_post_happy_path() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x42, vec![]).unwrap();
    let id = extract_id(&resp);
    assert!(id.starts_with("moult:"));
    assert_eq!(id.len(), "moult:".len() + 64);

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id: id.clone() })
        .unwrap();
    assert_eq!(entry.author, alice);
    assert_eq!(entry.commitment.len(), 32);
    assert_eq!(entry.content_type, "text/markdown");
    assert_eq!(entry.size_bytes, 256);
    assert!(entry.refs.is_empty());
    assert!(entry.redacted_at.is_none());

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.total_active, 1);
}

#[test]
fn test_post_invalid_commitment_length() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::Post {
                commitment: Binary::from(vec![0u8; 16]),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                attestation_ref: None,
                visibility: Visibility::Public,
                refs: vec![],
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(
        cerr,
        ContractError::InvalidCommitmentLength { got: 16 }
    ));
}

#[test]
fn test_post_size_too_large() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::Post {
                commitment: make_commitment(0x01),
                content_type: "text/plain".to_string(),
                size_bytes: MAX_SIZE + 1,
                attestation_ref: None,
                visibility: Visibility::Public,
                refs: vec![],
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::SizeTooLarge { .. }));
}

#[test]
fn test_post_with_invalid_ref() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = post_entry(
        &mut app,
        &contract,
        &alice,
        0x01,
        vec!["moult:does_not_exist".to_string()],
    )
    .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::InvalidRef { .. }));
}

#[test]
fn test_post_with_valid_refs_indexes_correctly() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    // Alice posts the foundational entry.
    let r1 = post_entry(&mut app, &contract, &alice, 0xAA, vec![]).unwrap();
    let id_a = extract_id(&r1);

    // Bob cites it.
    let r2 = post_entry(&mut app, &contract, &bob, 0xBB, vec![id_a.clone()]).unwrap();
    let id_b = extract_id(&r2);

    // ListByRef(id_a) should return Bob's entry.
    let resp: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByRef {
                ref_id: id_a.clone(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(resp.entries.len(), 1);
    assert_eq!(resp.entries[0].id, id_b);
    assert_eq!(resp.entries[0].author, bob);
    assert_eq!(resp.entries[0].refs, vec![id_a]);
}

#[test]
fn test_redact_by_author() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x33, vec![]).unwrap();
    let id = extract_id(&resp);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::Redact { id: id.clone() },
        &[],
    )
    .unwrap();

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id })
        .unwrap();
    assert_eq!(entry.commitment.len(), 0, "commitment should be cleared");
    assert!(entry.redacted_at.is_some());
    assert_eq!(entry.author, alice);

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.total_active, 0);
    assert_eq!(stats.total_redacted, 1);
}

#[test]
fn test_redact_by_admin() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x44, vec![]).unwrap();
    let id = extract_id(&resp);

    // Admin (not author) can redact.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Redact { id: id.clone() },
        &[],
    )
    .unwrap();

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id })
        .unwrap();
    assert!(entry.redacted_at.is_some());
}

#[test]
fn test_redact_by_stranger_unauthorized() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let mallory = app.api().addr_make("mallory");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x55, vec![]).unwrap();
    let id = extract_id(&resp);

    let err = app
        .execute_contract(
            mallory,
            contract,
            &ExecuteMsg::Redact { id },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::Unauthorized {}));
}

#[test]
fn test_update_visibility_cannot_widen_to_public() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    // Post as Owner-scope.
    let resp = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::Post {
                commitment: make_commitment(0x77),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                attestation_ref: None,
                visibility: Visibility::Owner,
                refs: vec![],
            },
            &[],
        )
        .unwrap();
    let id = extract_id(&resp);

    // Cannot widen to Public.
    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::UpdateVisibility {
                id,
                visibility: Visibility::Public,
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::CannotWidenVisibility {}));
}

#[test]
fn test_list_by_author_returns_owned_entries() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    post_entry(&mut app, &contract, &alice, 0xA1, vec![]).unwrap();
    post_entry(&mut app, &contract, &alice, 0xA2, vec![]).unwrap();
    post_entry(&mut app, &contract, &bob, 0xB1, vec![]).unwrap();

    let alice_entries: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByAuthor {
                author: alice.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(alice_entries.entries.len(), 2);
    for e in &alice_entries.entries {
        assert_eq!(e.author, alice);
    }

    let bob_entries: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByAuthor {
                author: bob.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(bob_entries.entries.len(), 1);
    assert_eq!(bob_entries.entries[0].author, bob);
}

#[test]
fn test_update_config_admin_only() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let mallory = app.api().addr_make("mallory");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            mallory,
            contract.clone(),
            &ExecuteMsg::UpdateConfig {
                admin: None,
                whoami_contract: None,
                max_size_bytes: Some(1),
                max_refs: None,
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::Unauthorized {}));

    // Admin can update.
    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::UpdateConfig {
            admin: None,
            whoami_contract: None,
            max_size_bytes: Some(2_000_000),
            max_refs: None,
        },
        &[],
    )
    .unwrap();

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.max_size_bytes, 2_000_000);
}
