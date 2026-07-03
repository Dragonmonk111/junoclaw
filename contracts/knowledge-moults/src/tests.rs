use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, MoultsResponse, QueryMsg, StatsResponse};
use crate::state::KnowledgeMoult;

const MOTHER_MOULT_ID: &str = "moult:mother-genesis";

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: Some(admin.to_string()),
            mother_moult_id: MOTHER_MOULT_ID.to_string(),
            max_summary_len: 4096,
            max_source_moults: 32,
        },
        &[],
        "knowledge-moults",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn mint(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    agent: &str,
    owner: Option<String>,
) -> Result<cw_multi_test::AppResponse, anyhow::Error> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Mint {
            agent: agent.to_string(),
            motive: "A18c-3-ui-decision".to_string(),
            knowledge_summary: "Commonwealth UI shipped in JunoClaw/Qu-Zeno.".to_string(),
            source_moults: vec!["moult:abc".to_string(), "moult:def".to_string()],
            owner,
        },
        &[],
    )
}

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

    let cfg: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin.to_string());
    assert_eq!(cfg.mother_moult_id, MOTHER_MOULT_ID);

    let stats: StatsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_minted, 0);
}

#[test]
fn test_mint_defaults_owner_to_sender() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    let id = extract_id(&resp);
    assert!(id.starts_with("kmoult:"));

    let moult: KnowledgeMoult = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetMoult { id: id.clone() })
        .unwrap();
    assert_eq!(moult.owner, hermes);
    assert_eq!(moult.minter, hermes);
    assert_eq!(moult.agent, "hermes");
    assert_eq!(moult.mother_moult_id, MOTHER_MOULT_ID);
    assert_eq!(moult.source_moults.len(), 2);

    let stats: StatsResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_minted, 1);
}

#[test]
fn test_mint_permissionless_to_explicit_owner() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let minter = app.api().addr_make("relayer");
    let beneficiary = app.api().addr_make("dragonmonk111-bot");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = mint(
        &mut app,
        &contract,
        &minter,
        "dragonmonk111-bot",
        Some(beneficiary.to_string()),
    )
    .unwrap();
    let id = extract_id(&resp);

    let moult: KnowledgeMoult = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetMoult { id })
        .unwrap();
    assert_eq!(moult.owner, beneficiary);
    assert_eq!(moult.minter, minter);
}

#[test]
fn test_mint_rejects_empty_agent() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let sender = app.api().addr_make("sender");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = mint(&mut app, &contract, &sender, "  ", None).unwrap_err();
    assert!(err.root_cause().to_string().contains("agent alias"));
}

#[test]
fn test_mint_rejects_summary_too_long() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let sender = app.api().addr_make("sender");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            sender.clone(),
            contract.clone(),
            &ExecuteMsg::Mint {
                agent: "hermes".to_string(),
                motive: "m".to_string(),
                knowledge_summary: "x".repeat(5000),
                source_moults: vec![],
                owner: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("too long"));
}

#[test]
fn test_mint_dedupes_byte_identical_calls_in_same_block() {
    // Same sender + identical fields + same block time = same deterministic
    // id. This mirrors Moultbook's Post (id = hash(commitment, author, time)):
    // a genuine re-submission of the exact same artifact is rejected as a
    // duplicate rather than silently minting a second copy.
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let contract = store_and_instantiate(&mut app, &admin);

    mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    let err = mint(&mut app, &contract, &hermes, "hermes", None).unwrap_err();
    assert!(err.root_cause().to_string().contains("Duplicate"));
}

#[test]
fn test_transfer_success() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let recipient = app.api().addr_make("recipient");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    let id = extract_id(&resp);

    app.execute_contract(
        hermes.clone(),
        contract.clone(),
        &ExecuteMsg::Transfer {
            id: id.clone(),
            recipient: recipient.to_string(),
        },
        &[],
    )
    .unwrap();

    let moult: KnowledgeMoult = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetMoult { id: id.clone() })
        .unwrap();
    assert_eq!(moult.owner, recipient);

    let owned_by_recipient: MoultsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByOwner {
                owner: recipient.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(owned_by_recipient.moults.len(), 1);

    let owned_by_hermes: MoultsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByOwner {
                owner: hermes.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(owned_by_hermes.moults.len(), 0);
}

#[test]
fn test_transfer_unauthorized() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let attacker = app.api().addr_make("attacker");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    let id = extract_id(&resp);

    let err = app
        .execute_contract(
            attacker.clone(),
            contract.clone(),
            &ExecuteMsg::Transfer {
                id,
                recipient: attacker.to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}

#[test]
fn test_list_by_agent() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let dragonmonk = app.api().addr_make("dragonmonk111-bot");
    let contract = store_and_instantiate(&mut app, &admin);

    mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    mint(&mut app, &contract, &dragonmonk, "dragonmonk111-bot", None).unwrap();
    // A second, distinct moult from hermes — real artifacts never share the
    // exact same summary/refs, so this must not collide with the first.
    app.execute_contract(
        hermes.clone(),
        contract.clone(),
        &ExecuteMsg::Mint {
            agent: "hermes".to_string(),
            motive: "A18c-4-mother-moult-publish".to_string(),
            knowledge_summary: "Mother-Moult v1.0 published under A18c-4.".to_string(),
            source_moults: vec!["moult:ghi".to_string()],
            owner: None,
        },
        &[],
    )
    .unwrap();

    let hermes_moults: MoultsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByAgent {
                agent: "hermes".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(hermes_moults.moults.len(), 2);
}

#[test]
fn test_update_mother_moult_admin_only() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let attacker = app.api().addr_make("attacker");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            attacker,
            contract.clone(),
            &ExecuteMsg::UpdateMotherMoult {
                mother_moult_id: "moult:mother-v2".to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));

    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::UpdateMotherMoult {
            mother_moult_id: "moult:mother-v2".to_string(),
        },
        &[],
    )
    .unwrap();

    let cfg: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.mother_moult_id, "moult:mother-v2");
}

#[test]
fn test_past_mints_unaffected_by_mother_moult_update() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let hermes = app.api().addr_make("hermes");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = mint(&mut app, &contract, &hermes, "hermes", None).unwrap();
    let id = extract_id(&resp);

    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::UpdateMotherMoult {
            mother_moult_id: "moult:mother-v2".to_string(),
        },
        &[],
    )
    .unwrap();

    // The already-minted moult still points at the Mother-Moult version that
    // was canonical when it was minted — history is not rewritten.
    let moult: KnowledgeMoult = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetMoult { id })
        .unwrap();
    assert_eq!(moult.mother_moult_id, MOTHER_MOULT_ID);
}
