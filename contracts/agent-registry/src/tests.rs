use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use junoclaw_common::AgentProfile;

fn store_and_instantiate(app: &mut App, admin: &Addr, fee: u128) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            max_agents: 100,
            registration_fee_ujuno: Uint128::from(fee),
            denom: Some("ujuno".to_string()),
            registry: None,
        },
        &[],
        "agent-registry",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn register_agent(app: &mut App, contract: &Addr, sender: &Addr, funds: &[cosmwasm_std::Coin]) -> Result<cw_multi_test::AppResponse, anyhow::Error> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::RegisterAgent {
            name: "TestAgent".to_string(),
            description: "A test agent".to_string(),
            capabilities_hash: "abc123".to_string(),
            model: "llama3.2:3b".to_string(),
        },
        funds,
    )
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.max_agents, 100);
    assert!(cfg.registration_fee_ujuno.is_zero());
}

#[test]
fn test_register_agent_free() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();

    assert_eq!(profile.owner, user);
    assert_eq!(profile.name, "TestAgent");
    assert!(profile.is_active);
    assert_eq!(profile.trust_score, 0);
    assert_eq!(profile.total_tasks, 0);
}

#[test]
fn test_register_agent_with_fee() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("user1"), coins(5_000_000, "ujuno"))
            .unwrap();
    });
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 1_000_000);

    register_agent(&mut app, &contract, &user, &coins(1_000_000, "ujuno")).unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.name, "TestAgent");
}

#[test]
fn test_register_agent_insufficient_fee_fails() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("user1"), coins(5_000_000, "ujuno"))
            .unwrap();
    });
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 1_000_000);

    let err = register_agent(&mut app, &contract, &user, &coins(500_000, "ujuno")).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::InsufficientFee { .. }));
}

#[test]
fn test_update_agent() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::UpdateAgent {
            agent_id: 1,
            name: Some("UpdatedAgent".to_string()),
            description: None,
            capabilities_hash: None,
            model: None,
        },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.name, "UpdatedAgent");
}

#[test]
fn test_update_agent_wrong_owner_fails() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let other = Addr::unchecked("user2");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    let err = app
        .execute_contract(
            other.clone(),
            contract.clone(),
            &ExecuteMsg::UpdateAgent {
                agent_id: 1,
                name: Some("Hacked".to_string()),
                description: None,
                capabilities_hash: None,
                model: None,
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotOwner {}));
}

#[test]
fn test_deactivate_agent() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::DeactivateAgent { agent_id: 1 },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert!(!profile.is_active);
}

#[test]
fn test_increment_tasks_updates_trust_score() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    // Successful task increments trust_score
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::IncrementTasks { agent_id: 1, success: true },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.total_tasks, 1);
    assert_eq!(profile.successful_tasks, 1);
    assert_eq!(profile.trust_score, 1);

    // Failed task does not increment trust_score
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::IncrementTasks { agent_id: 1, success: false },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.total_tasks, 2);
    assert_eq!(profile.successful_tasks, 1);
    assert_eq!(profile.trust_score, 1);
}

#[test]
fn test_slash_agent_decrements_trust_score() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    // Build up 10 trust score
    for _ in 0..10 {
        app.execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::IncrementTasks { agent_id: 1, success: true },
            &[],
        )
        .unwrap();
    }

    // Slash removes 5
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SlashAgent {
            agent_id: 1,
            reason: "Misbehaviour".to_string(),
        },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.trust_score, 5);
}

#[test]
fn test_slash_agent_saturates_at_zero() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    // trust_score starts at 0; slashing should not underflow
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SlashAgent {
            agent_id: 1,
            reason: "Test".to_string(),
        },
        &[],
    )
    .unwrap();

    let profile: AgentProfile = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAgent { agent_id: 1 })
        .unwrap();
    assert_eq!(profile.trust_score, 0);
}

#[test]
fn test_slash_unauthorized_fails() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let user = Addr::unchecked("user1");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &user, &[]).unwrap();

    let err = app
        .execute_contract(
            user.clone(),
            contract.clone(),
            &ExecuteMsg::SlashAgent { agent_id: 1, reason: "hack".to_string() },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_agent_limit() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    let contract = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                admin: None,
                max_agents: 2,
                registration_fee_ujuno: Uint128::zero(),
                denom: Some("ujuno".to_string()),
                registry: None,
            },
            &[],
            "registry",
            None,
        )
        .unwrap();

    register_agent(&mut app, &contract, &Addr::unchecked("u1"), &[]).unwrap();
    register_agent(&mut app, &contract, &Addr::unchecked("u2"), &[]).unwrap();

    let err = register_agent(&mut app, &contract, &Addr::unchecked("u3"), &[]).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AgentLimitReached { .. }));
}

#[test]
fn test_list_agents() {
    let mut app = App::default();
    let admin = Addr::unchecked("admin");
    let contract = store_and_instantiate(&mut app, &admin, 0);

    register_agent(&mut app, &contract, &Addr::unchecked("u1"), &[]).unwrap();
    register_agent(&mut app, &contract, &Addr::unchecked("u2"), &[]).unwrap();

    let agents: Vec<AgentProfile> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListAgents { start_after: None, limit: None })
        .unwrap();
    assert_eq!(agents.len(), 2);
}
