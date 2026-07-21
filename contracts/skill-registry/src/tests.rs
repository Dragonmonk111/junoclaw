use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, RegistryStats, SkillEntry};

fn store_and_instantiate(app: &mut App, admin: &Addr, fee: u128) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            denom: Some("ujuno".to_string()),
            registration_fee: Uint128::from(fee),
        },
        &[],
        "skill-registry",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn publish(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    dapp_name: &str,
    funds: &[cosmwasm_std::Coin],
) -> Result<cw_multi_test::AppResponse, anyhow::Error> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::PublishSkill {
            dapp_name: dapp_name.to_string(),
            chain_id: "juno-1".to_string(),
            skill_uri: "ipfs://bafy.../SKILL.md".to_string(),
            skill_hash: "deadbeef".to_string(),
        },
        funds,
    )
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin);
    assert!(cfg.registration_fee.is_zero());
}

#[test]
fn test_publish_skill_free() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "osmosis-dex", &[]).unwrap();

    let entry: SkillEntry = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetSkill {
                dapp_name: "osmosis-dex".to_string(),
            },
        )
        .unwrap();

    assert_eq!(entry.publisher, publisher);
    assert_eq!(entry.dapp_name, "osmosis-dex");
    assert_eq!(entry.version, 1);

    let stats: RegistryStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 1);
}

#[test]
fn test_publish_skill_requires_fee() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 1_000_000);

    let err = publish(&mut app, &contract, &publisher, "levana-perps", &[]).unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::InsufficientFee { .. }));
}

#[test]
fn test_publish_skill_with_sufficient_fee_succeeds() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("dapp-team"), coins(1_000_000, "ujuno"))
            .unwrap();
    });
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 1_000_000);

    publish(
        &mut app,
        &contract,
        &publisher,
        "levana-perps",
        &coins(1_000_000, "ujuno"),
    )
    .unwrap();

    let entry: SkillEntry = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetSkill {
                dapp_name: "levana-perps".to_string(),
            },
        )
        .unwrap();
    assert_eq!(entry.dapp_name, "levana-perps");
}

#[test]
fn test_publish_duplicate_name_rejected() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher1 = Addr::unchecked("dapp-team-1");
    let publisher2 = Addr::unchecked("dapp-team-2");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher1, "osmosis-dex", &[]).unwrap();

    let err = publish(&mut app, &contract, &publisher2, "osmosis-dex", &[]).unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::NameAlreadyClaimed { .. }));
}

#[test]
fn test_update_skill_by_publisher() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "osmosis-dex", &[]).unwrap();

    app.execute_contract(
        publisher.clone(),
        contract.clone(),
        &ExecuteMsg::UpdateSkill {
            dapp_name: "osmosis-dex".to_string(),
            chain_id: None,
            skill_uri: Some("ipfs://newcid/SKILL.md".to_string()),
            skill_hash: Some("newhash".to_string()),
        },
        &[],
    )
    .unwrap();

    let entry: SkillEntry = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetSkill {
                dapp_name: "osmosis-dex".to_string(),
            },
        )
        .unwrap();
    assert_eq!(entry.skill_uri, "ipfs://newcid/SKILL.md");
    assert_eq!(entry.skill_hash, "newhash");
    assert_eq!(entry.version, 2);
}

#[test]
fn test_update_skill_by_non_publisher_rejected() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let stranger = Addr::unchecked("stranger");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "osmosis-dex", &[]).unwrap();

    let err = app
        .execute_contract(
            stranger,
            contract.clone(),
            &ExecuteMsg::UpdateSkill {
                dapp_name: "osmosis-dex".to_string(),
                chain_id: None,
                skill_uri: Some("ipfs://malicious/SKILL.md".to_string()),
                skill_hash: None,
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_admin_can_transfer_publisher_for_dispute_resolution() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let squatter = Addr::unchecked("squatter");
    let rightful_owner = app.api().addr_make("rightful_owner");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &squatter, "osmosis-dex", &[]).unwrap();

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TransferPublisher {
            dapp_name: "osmosis-dex".to_string(),
            new_publisher: rightful_owner.to_string(),
        },
        &[],
    )
    .unwrap();

    let entry: SkillEntry = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetSkill {
                dapp_name: "osmosis-dex".to_string(),
            },
        )
        .unwrap();
    assert_eq!(entry.publisher, rightful_owner);

    // Squatter can no longer update; rightful owner now can.
    let err = app
        .execute_contract(
            squatter,
            contract.clone(),
            &ExecuteMsg::UpdateSkill {
                dapp_name: "osmosis-dex".to_string(),
                chain_id: None,
                skill_uri: Some("ipfs://squatter/SKILL.md".to_string()),
                skill_hash: None,
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_admin_remove_skill() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "osmosis-dex", &[]).unwrap();

    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::RemoveSkill {
            dapp_name: "osmosis-dex".to_string(),
        },
        &[],
    )
    .unwrap();

    let err = app
        .wrap()
        .query_wasm_smart::<SkillEntry>(
            &contract,
            &QueryMsg::GetSkill {
                dapp_name: "osmosis-dex".to_string(),
            },
        )
        .unwrap_err();
    assert!(format!("{err}").contains("not found") || format!("{err}").contains("Skill not found"));

    let stats: RegistryStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 0);
}

#[test]
fn test_non_admin_cannot_remove_skill() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "osmosis-dex", &[]).unwrap();

    let err = app
        .execute_contract(
            publisher,
            contract.clone(),
            &ExecuteMsg::RemoveSkill {
                dapp_name: "osmosis-dex".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_search_by_chain() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    publish(&mut app, &contract, &publisher, "juno-dapp-a", &[]).unwrap();
    publish(&mut app, &contract, &publisher, "juno-dapp-b", &[]).unwrap();

    app.execute_contract(
        publisher.clone(),
        contract.clone(),
        &ExecuteMsg::PublishSkill {
            dapp_name: "osmosis-dapp".to_string(),
            chain_id: "osmosis-1".to_string(),
            skill_uri: "ipfs://x/SKILL.md".to_string(),
            skill_hash: "hash".to_string(),
        },
        &[],
    )
    .unwrap();

    let juno_results: Vec<SkillEntry> = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::SearchByChain {
                chain_id: "juno-1".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(juno_results.len(), 2);

    let osmosis_results: Vec<SkillEntry> = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::SearchByChain {
                chain_id: "osmosis-1".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(osmosis_results.len(), 1);
}

#[test]
fn test_list_skills() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    for i in 0..5 {
        publish(&mut app, &contract, &publisher, &format!("dapp-{i}"), &[]).unwrap();
    }

    let all: Vec<SkillEntry> = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListSkills {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(all.len(), 5);
}

#[test]
fn test_publish_rejects_empty_fields() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let publisher = Addr::unchecked("dapp-team");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    let err = app
        .execute_contract(
            publisher.clone(),
            contract.clone(),
            &ExecuteMsg::PublishSkill {
                dapp_name: "".to_string(),
                chain_id: "juno-1".to_string(),
                skill_uri: "ipfs://x".to_string(),
                skill_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::EmptyName {}));

    let err = app
        .execute_contract(
            publisher,
            contract,
            &ExecuteMsg::PublishSkill {
                dapp_name: "valid-name".to_string(),
                chain_id: "juno-1".to_string(),
                skill_uri: "".to_string(),
                skill_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::EmptyUri {}));
}

#[test]
fn test_update_config_admin_only() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let stranger = Addr::unchecked("stranger");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    let err = app
        .execute_contract(
            stranger,
            contract.clone(),
            &ExecuteMsg::UpdateConfig {
                admin: None,
                registration_fee: Some(Uint128::from(500_000u128)),
            },
            &[],
        )
        .unwrap_err();
    let contract_err: ContractError = err.downcast().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));

    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::UpdateConfig {
            admin: None,
            registration_fee: Some(Uint128::from(500_000u128)),
        },
        &[],
    )
    .unwrap();

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.registration_fee, Uint128::from(500_000u128));
}
