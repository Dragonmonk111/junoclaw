use cosmwasm_std::{
    to_json_binary, Addr, Binary, CustomMsg, CustomQuery, Deps, DepsMut, Empty, Env, MessageInfo,
    Reply, Response, Uint128,
};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

fn mk(app: &App, label: &str) -> Addr { app.api().addr_make(label) }

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::LedgerStats;
use junoclaw_common::{ExecutionTier, TaskRecord, TaskStatus};

// ── Test-only stub "agent-registry" ──
// task-ledger now wires the `IncrementTasks` callback by default at
// instantiate (C1/C2: registry.agent_registry mirrors msg.agent_registry).
// The unit tests therefore need a real contract at that address, not a
// bare `Addr`. This stub implements `Contract` at the raw `Vec<u8>` msg
// level so it can accept *any* structured execute message (including
// `IncrementTasks`) without having to expose its full schema or pull in
// the real `agent-registry` crate as a dev-dep.
struct StubRegistry;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubRegistry
where
    ExecC: CustomMsg + 'static,
    QueryC: CustomQuery + serde::de::DeserializeOwned + 'static,
{
    fn execute(
        &self,
        _deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn instantiate(
        &self,
        _deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn query(
        &self,
        _deps: Deps<QueryC>,
        _env: Env,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Binary> {
        Ok(to_json_binary(&()).unwrap())
    }
    fn sudo(
        &self,
        _deps: DepsMut<QueryC>,
        _env: Env,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn reply(
        &self,
        _deps: DepsMut<QueryC>,
        _env: Env,
        _msg: Reply,
    ) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn migrate(
        &self,
        _deps: DepsMut<QueryC>,
        _env: Env,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
}

/// Instantiate the stub registry at a fresh address and return that address.
/// Tests that previously passed a cosmetic `mk(..)` registry should use this
/// so the `IncrementTasks` callback during `complete_task` / `fail_task`
/// succeeds instead of erroring on a non-existent contract.
fn instantiate_stub_registry(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubRegistry));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &Empty {},
        &[],
        "stub-registry",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn store_and_instantiate(app: &mut App, admin: &Addr, registry: &Addr, operators: Option<Vec<String>>) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            agent_registry: registry.to_string(),
            operators,
            registry: None,
        },
        &[],
        "task-ledger",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn submit_task(app: &mut App, contract: &Addr, sender: &Addr, agent_id: u64) -> u64 {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitTask {
            agent_id,
            input_hash: format!("hash-{}", agent_id),
            execution_tier: ExecutionTier::Local,
            proposal_id: None,
        },
        &[],
    )
    .unwrap();
    // Return the task ID (sequential from 1)
    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(contract, &QueryMsg::GetStats {})
        .unwrap();
    stats.total_tasks
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin);
    assert!(cfg.operators.is_empty());
}

#[test]
fn test_submit_task() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    let task_id = submit_task(&mut app, &contract, &user, 1);
    assert_eq!(task_id, 1);

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.agent_id, 1);
    assert_eq!(task.submitter, user);
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.output_hash.is_none());
}

#[test]
fn test_complete_task_by_submitter() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: 1,
            output_hash: "output_abc".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.output_hash, Some("output_abc".to_string()));
}

#[test]
fn test_complete_task_by_operator() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let daemon = mk(&app, "daemon");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(
        &mut app,
        &admin,
        &reg,
        Some(vec![daemon.to_string()]),
    );

    submit_task(&mut app, &contract, &user, 1);

    // daemon (operator) can complete the task
    app.execute_contract(
        daemon.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: 1,
            output_hash: "daemon_output".to_string(),
            cost_ujuno: Some(Uint128::from(500_000u128)),
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.cost_ujuno, Some(Uint128::from(500_000u128)));
}

#[test]
fn test_complete_task_unauthorized_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let stranger = mk(&app, "stranger");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 1);

    let err = app
        .execute_contract(
            stranger.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: 1,
                output_hash: "hack".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotSubmitter {}));
}

#[test]
fn test_fail_task_by_operator() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let daemon = mk(&app, "daemon");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(
        &mut app,
        &admin,
        &reg,
        Some(vec![daemon.to_string()]),
    );

    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        daemon.clone(),
        contract.clone(),
        &ExecuteMsg::FailTask { task_id: 1 },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Failed);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_failed, 1);
}

#[test]
fn test_cancel_task() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::CancelTask { task_id: 1 },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Cancelled);
}

#[test]
fn test_double_complete_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask { task_id: 1, output_hash: "h1".to_string(), cost_ujuno: None },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            user.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask { task_id: 1, output_hash: "h2".to_string(), cost_ujuno: None },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::TaskNotRunning { .. }));
}

#[test]
fn test_add_remove_operator() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let daemon = mk(&app, "daemon");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    // Add operator
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AddOperator { operator: daemon.to_string() },
        &[],
    )
    .unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert!(cfg.operators.contains(&daemon));

    // Remove operator
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::RemoveOperator { operator: daemon.to_string() },
        &[],
    )
    .unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert!(!cfg.operators.contains(&daemon));
}

#[test]
fn test_add_operator_only_admin() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    let err = app
        .execute_contract(
            user.clone(),
            contract.clone(),
            &ExecuteMsg::AddOperator { operator: user.to_string() },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_tasks_by_agent_query() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 42);
    submit_task(&mut app, &contract, &user, 42);
    submit_task(&mut app, &contract, &user, 99); // different agent

    let tasks: Vec<TaskRecord> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTasksByAgent { agent_id: 42, limit: None })
        .unwrap();
    assert_eq!(tasks.len(), 2);
}

#[test]
fn test_stats_counter() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    submit_task(&mut app, &contract, &user, 1);
    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask { task_id: 1, output_hash: "h".to_string(), cost_ujuno: None },
        &[],
    )
    .unwrap();
    app.execute_contract(
        user.clone(),
        contract.clone(),
        &ExecuteMsg::FailTask { task_id: 2 },
        &[],
    )
    .unwrap();

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_tasks, 2);
    assert_eq!(stats.total_completed, 1);
    assert_eq!(stats.total_failed, 1);
}
