use cosmwasm_std::{
    to_json_binary, Addr, Binary, Coin, CustomMsg, CustomQuery, Deps, DepsMut, Empty, Env,
    MessageInfo, Reply, Response, Uint128,
};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_storage_plus::{Item, Map};

fn mk(app: &App, label: &str) -> Addr { app.api().addr_make(label) }

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::LedgerStats;
use junoclaw_common::{
    AgentProfile, Constraint, ExecutionTier, ObligationStatus, PaymentObligation, TaskRecord,
    TaskStatus,
};

const STUB_AGENTS: Map<u64, AgentProfile> = Map::new("stub_agents");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubRegistryExecuteMsg {
    RegisterAgent {
        agent_id: u64,
        owner: String,
    },
    /// Test-only: set the stored agent's `trust_score` directly, for
    /// exercising the `Constraint::AgentTrustAtLeast` hook.
    SetTrustScore {
        agent_id: u64,
        trust_score: u64,
    },
    IncrementTasks {
        agent_id: u64,
        success: bool,
    },
    UpdateRegistry {
        agent_registry: Option<String>,
        task_ledger: Option<String>,
        escrow: Option<String>,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubRegistryQueryMsg {
    GetAgent { agent_id: u64 },
}

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
        deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        if let Ok(msg) = cosmwasm_std::from_json::<StubRegistryExecuteMsg>(&msg) {
            match msg {
                StubRegistryExecuteMsg::RegisterAgent { agent_id, owner } => {
                    STUB_AGENTS.save(
                        deps.storage,
                        agent_id,
                        &AgentProfile {
                            owner: Addr::unchecked(owner),
                            name: format!("stub-agent-{}", agent_id),
                            description: "stub".to_string(),
                            capabilities_hash: format!("cap-{}", agent_id),
                            model: "stub".to_string(),
                            registered_at: 0,
                            is_active: true,
                            total_tasks: 0,
                            successful_tasks: 0,
                            trust_score: 0,
                        },
                    )?;
                }
                StubRegistryExecuteMsg::SetTrustScore { agent_id, trust_score } => {
                    let mut profile = STUB_AGENTS.load(deps.storage, agent_id)?;
                    profile.trust_score = trust_score;
                    STUB_AGENTS.save(deps.storage, agent_id, &profile)?;
                }
                StubRegistryExecuteMsg::IncrementTasks { .. }
                | StubRegistryExecuteMsg::UpdateRegistry { .. } => {}
            }
        }
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
        deps: Deps<QueryC>,
        _env: Env,
        msg: Vec<u8>,
    ) -> anyhow::Result<Binary> {
        if let Ok(StubRegistryQueryMsg::GetAgent { agent_id }) =
            cosmwasm_std::from_json::<StubRegistryQueryMsg>(&msg)
        {
            let profile = STUB_AGENTS.load(deps.storage, agent_id)?;
            return Ok(to_json_binary(&profile)?);
        }
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

fn register_stub_agent(app: &mut App, registry: &Addr, owner: &Addr, agent_id: u64) {
    app.execute_contract(
        owner.clone(),
        registry.clone(),
        &StubRegistryExecuteMsg::RegisterAgent {
            agent_id,
            owner: owner.to_string(),
        },
        &[],
    )
    .unwrap();
}

fn set_stub_trust_score(app: &mut App, registry: &Addr, caller: &Addr, agent_id: u64, trust_score: u64) {
    app.execute_contract(
        caller.clone(),
        registry.clone(),
        &StubRegistryExecuteMsg::SetTrustScore { agent_id, trust_score },
        &[],
    )
    .unwrap();
}

// ── Stub junoswap-pair for PairReservesPositive constraint tests ──
// Responds only to the `Pool {}` query and admits a test-only
// `SetReserves { a, b }` execute to simulate pool state changes without
// pulling in the junoswap-pair crate as a dev-dependency.

const STUB_RESERVES: Item<(Uint128, Uint128)> = Item::new("stub_reserves");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubPairExecuteMsg {
    SetReserves { a: Uint128, b: Uint128 },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubPairQueryMsg {
    Pool {},
}

#[derive(serde::Serialize)]
struct StubPoolResponse {
    reserve_a: Uint128,
    reserve_b: Uint128,
    total_lp_shares: Uint128,
}

struct StubPair;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubPair
where
    ExecC: CustomMsg + 'static,
    QueryC: CustomQuery + serde::de::DeserializeOwned + 'static,
{
    fn execute(
        &self,
        deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        if let Ok(StubPairExecuteMsg::SetReserves { a, b }) =
            cosmwasm_std::from_json::<StubPairExecuteMsg>(&msg)
        {
            STUB_RESERVES.save(deps.storage, &(a, b))?;
        }
        Ok(Response::new())
    }
    fn instantiate(
        &self,
        deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        _msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        STUB_RESERVES.save(deps.storage, &(Uint128::zero(), Uint128::zero()))?;
        Ok(Response::new())
    }
    fn query(
        &self,
        deps: Deps<QueryC>,
        _env: Env,
        msg: Vec<u8>,
    ) -> anyhow::Result<Binary> {
        if let Ok(StubPairQueryMsg::Pool {}) = cosmwasm_std::from_json::<StubPairQueryMsg>(&msg) {
            let (a, b) = STUB_RESERVES.load(deps.storage)?;
            return Ok(to_json_binary(&StubPoolResponse {
                reserve_a: a,
                reserve_b: b,
                total_lp_shares: Uint128::zero(),
            })?);
        }
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

fn instantiate_stub_pair(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubPair));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &Empty {},
        &[],
        "stub-pair",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn set_stub_pair_reserves(app: &mut App, pair: &Addr, caller: &Addr, a: u128, b: u128) {
    app.execute_contract(
        caller.clone(),
        pair.clone(),
        &StubPairExecuteMsg::SetReserves {
            a: Uint128::new(a),
            b: Uint128::new(b),
        },
        &[],
    )
    .unwrap();
}

// ── Stub escrow for EscrowObligationConfirmed constraint tests ──
// Responds to `GetObligationByTask { task_id }` returning
// `Option<PaymentObligation>`, matching the real escrow's public
// query contract. A test-only `SetObligation { task_id, status }`
// execute lets tests pre-seed the escrow's view of a given task
// without pulling the escrow crate in as a dev-dependency.

const STUB_OBLIGATIONS: Map<u64, PaymentObligation> = Map::new("stub_obligations");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubEscrowExecuteMsg {
    SetObligation { task_id: u64, status: ObligationStatus },
    ClearObligation { task_id: u64 },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubEscrowQueryMsg {
    GetObligationByTask { task_id: u64 },
}

struct StubEscrow;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubEscrow
where
    ExecC: CustomMsg + 'static,
    QueryC: CustomQuery + serde::de::DeserializeOwned + 'static,
{
    fn execute(
        &self,
        deps: DepsMut<QueryC>,
        _env: Env,
        _info: MessageInfo,
        msg: Vec<u8>,
    ) -> anyhow::Result<Response<ExecC>> {
        if let Ok(msg) = cosmwasm_std::from_json::<StubEscrowExecuteMsg>(&msg) {
            match msg {
                StubEscrowExecuteMsg::SetObligation { task_id, status } => {
                    STUB_OBLIGATIONS.save(
                        deps.storage,
                        task_id,
                        &PaymentObligation {
                            id: task_id,
                            payer: Addr::unchecked("stub-payer"),
                            payee: Addr::unchecked("stub-payee"),
                            task_id,
                            amount: Uint128::new(1),
                            denom: "ujuno".to_string(),
                            status,
                            created_at: 0,
                            settled_at: None,
                            attestation_hash: None,
                        },
                    )?;
                }
                StubEscrowExecuteMsg::ClearObligation { task_id } => {
                    STUB_OBLIGATIONS.remove(deps.storage, task_id);
                }
            }
        }
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
        deps: Deps<QueryC>,
        _env: Env,
        msg: Vec<u8>,
    ) -> anyhow::Result<Binary> {
        if let Ok(StubEscrowQueryMsg::GetObligationByTask { task_id }) =
            cosmwasm_std::from_json::<StubEscrowQueryMsg>(&msg)
        {
            let found = STUB_OBLIGATIONS.may_load(deps.storage, task_id)?;
            return Ok(to_json_binary(&found)?);
        }
        Ok(to_json_binary(&None::<PaymentObligation>).unwrap())
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

fn instantiate_stub_escrow(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubEscrow));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &Empty {},
        &[],
        "stub-escrow",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn set_stub_obligation(
    app: &mut App,
    escrow: &Addr,
    caller: &Addr,
    task_id: u64,
    status: ObligationStatus,
) {
    app.execute_contract(
        caller.clone(),
        escrow.clone(),
        &StubEscrowExecuteMsg::SetObligation { task_id, status },
        &[],
    )
    .unwrap();
}

fn store_and_instantiate_with_agent_company(
    app: &mut App,
    admin: &Addr,
    registry: &Addr,
    operators: Option<Vec<String>>,
    agent_company: Option<String>,
) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            agent_registry: registry.to_string(),
            operators,
            agent_company,
            registry: None,
        },
        &[],
        "task-ledger",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn store_and_instantiate(app: &mut App, admin: &Addr, registry: &Addr, operators: Option<Vec<String>>) -> Addr {
    store_and_instantiate_with_agent_company(app, admin, registry, operators, None)
}

fn submit_task(app: &mut App, contract: &Addr, sender: &Addr, agent_id: u64) -> u64 {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitTask {
            agent_id,
            input_hash: format!("hash-{}", agent_id),
            execution_tier: ExecutionTier::Local,
            pre_hooks: vec![],
            post_hooks: vec![],
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
    register_stub_agent(&mut app, &reg, &user, 1);

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
fn test_submit_task_rejects_unowned_agent() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let owner = mk(&app, "owner");
    let stranger = mk(&app, "stranger");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &owner, 1);

    let err = app
        .execute_contract(
            stranger,
            contract.clone(),
            &ExecuteMsg::SubmitTask {
                agent_id: 1,
                input_hash: "hash-1".to_string(),
                execution_tier: ExecutionTier::Local,
                proposal_id: None,
                pre_hooks: vec![],
                post_hooks: vec![],
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AgentNotOwned { agent_id: 1 }));
}

#[test]
fn test_submit_task_rejects_reserved_agent_id_for_public_sender() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);

    let err = app
        .execute_contract(
            user,
            contract.clone(),
            &ExecuteMsg::SubmitTask {
                agent_id: 0,
                input_hash: "hash-0".to_string(),
                execution_tier: ExecutionTier::Local,
                proposal_id: None,
                pre_hooks: vec![],
                post_hooks: vec![],
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ReservedAgentId {}));
}

#[test]
fn test_submit_task_proposal_id_requires_agent_company_and_is_unique() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let agent_company = mk(&app, "agent-company");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate_with_agent_company(
        &mut app,
        &admin,
        &reg,
        None,
        Some(agent_company.to_string()),
    );

    let err = app
        .execute_contract(
            user,
            contract.clone(),
            &ExecuteMsg::SubmitTask {
                agent_id: 0,
                input_hash: "hash-prop".to_string(),
                execution_tier: ExecutionTier::Local,
                proposal_id: Some(7),
                pre_hooks: vec![],
                post_hooks: vec![],
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));

    app.execute_contract(
        agent_company.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitTask {
            agent_id: 0,
            input_hash: "hash-prop".to_string(),
            execution_tier: ExecutionTier::Local,
            proposal_id: Some(7),
            pre_hooks: vec![],
            post_hooks: vec![],
        },
        &[],
    )
    .unwrap();

    let task: Option<TaskRecord> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTaskByProposal { proposal_id: 7 })
        .unwrap();
    assert!(task.is_some());

    let err = app
        .execute_contract(
            agent_company,
            contract.clone(),
            &ExecuteMsg::SubmitTask {
                agent_id: 0,
                input_hash: "hash-prop-2".to_string(),
                execution_tier: ExecutionTier::Local,
                proposal_id: Some(7),
                pre_hooks: vec![],
                post_hooks: vec![],
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ProposalTaskAlreadyExists { proposal_id: 7 }));
}

#[test]
fn test_complete_task_by_submitter_is_rejected() {
    // v6 F1 — Completion is an attestation, not a self-declaration. A
    // submitter who was also the payer on a linked escrow obligation used
    // to be able to self-complete and fire the atomic `escrow::Confirm`
    // callback, which marked the obligation paid without any funds ever
    // moving. The submitter path is now closed.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    submit_task(&mut app, &contract, &user, 1);

    let err = app
        .execute_contract(
            user.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: 1,
                output_hash: "output_abc".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));

    // Task must still be Running — the rejected Complete must not have
    // leaked any state change.
    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Running);
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
    register_stub_agent(&mut app, &reg, &user, 1);

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
    register_stub_agent(&mut app, &reg, &user, 1);

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
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_fail_task_by_submitter_is_rejected() {
    // v6 F1 — the symmetric invariant for `FailTask`: a payer who was also
    // the task submitter used to be able to unilaterally mark the task
    // Failed, which fires the atomic `escrow::Cancel` callback and voids
    // their debt. The submitter path is now closed for `FailTask` as
    // well; only operators/admin/agent_company can finalise a task.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    submit_task(&mut app, &contract, &user, 1);

    let err = app
        .execute_contract(
            user.clone(),
            contract.clone(),
            &ExecuteMsg::FailTask { task_id: 1 },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: 1 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Running);
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
    register_stub_agent(&mut app, &reg, &user, 1);

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
    register_stub_agent(&mut app, &reg, &user, 1);

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
    // admin is the operator; a daemon-style wallet would work equally.
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    submit_task(&mut app, &contract, &user, 1);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask { task_id: 1, output_hash: "h1".to_string(), cost_ujuno: None },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            admin.clone(),
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
    register_stub_agent(&mut app, &reg, &user, 42);
    register_stub_agent(&mut app, &reg, &user, 99);

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
    register_stub_agent(&mut app, &reg, &user, 1);

    submit_task(&mut app, &contract, &user, 1);
    submit_task(&mut app, &contract, &user, 1);

    // v6 F1 — only operators/admin/agent_company may finalise a task.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask { task_id: 1, output_hash: "h".to_string(), cost_ujuno: None },
        &[],
    )
    .unwrap();
    app.execute_contract(
        admin.clone(),
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

// ─────────────────────────────────────────────────────────────────
// v7: Task hook constraints
// ─────────────────────────────────────────────────────────────────

fn submit_task_with_hooks(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    agent_id: u64,
    pre_hooks: Vec<Constraint>,
    post_hooks: Vec<Constraint>,
) -> u64 {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitTask {
            agent_id,
            input_hash: format!("hash-{}", agent_id),
            execution_tier: ExecutionTier::Local,
            pre_hooks,
            post_hooks,
            proposal_id: None,
        },
        &[],
    )
    .unwrap();
    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(contract, &QueryMsg::GetStats {})
        .unwrap();
    stats.total_tasks
}

#[test]
fn test_v7_submit_task_stores_hooks_on_record() {
    // Hooks attached at submit time must round-trip through GetTask
    // unchanged. This proves storage serialisation is stable so a hook
    // set at block N is the same hook evaluated at completion block M.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    let pre = vec![Constraint::AgentTrustAtLeast { agent_id: 1, min_score: 10 }];
    let post = vec![Constraint::BalanceAtLeast {
        who: admin.clone(),
        denom: "ujuno".to_string(),
        amount: Uint128::zero(),
    }];
    let task_id = submit_task_with_hooks(&mut app, &contract, &user, 1, pre.clone(), post.clone());

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.pre_hooks, pre);
    assert_eq!(task.post_hooks, post);
}

#[test]
fn test_v7_complete_evaluates_pre_hook_happy_path() {
    // An agent with trust_score ≥ min_score satisfies the pre_hook, so
    // completion proceeds and the task reaches Completed.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);
    set_stub_trust_score(&mut app, &reg, &admin, 1, 100);

    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::AgentTrustAtLeast { agent_id: 1, min_score: 50 }],
        vec![],
    );

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
}

#[test]
fn test_v7_pre_hook_violation_reverts_tx() {
    // An agent with trust_score below min_score triggers a
    // ConstraintViolated error. The task must remain Running — no state
    // write for status, no escrow callback, no registry callback.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);
    set_stub_trust_score(&mut app, &reg, &admin, 1, 10);

    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::AgentTrustAtLeast { agent_id: 1, min_score: 50 }],
        vec![],
    );

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let ce = err.downcast::<ContractError>().unwrap();
    match &ce {
        ContractError::ConstraintViolated { reason } => {
            assert!(
                reason.starts_with("pre_hook:"),
                "expected pre_hook prefix, got: {}",
                reason
            );
        }
        other => panic!("expected ConstraintViolated, got: {:?}", other),
    }

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.output_hash.is_none());
    assert!(task.completed_at.is_none());
}

#[test]
fn test_v7_post_hook_violation_reverts_tx() {
    // Atomicity proof: a post_hook evaluated *after* status transitions
    // to Completed but failing must revert the status change. Task must
    // remain Running despite the in-memory mutation already having
    // occurred in execute_complete. This works because CosmWasm
    // discards all storage writes when the entry point returns Err.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    // Post-hook requires admin to hold ≥ 1 ujuno. Admin balance is zero
    // in App::default(), so the hook is guaranteed to fail at complete
    // time.
    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![],
        vec![Constraint::BalanceAtLeast {
            who: admin.clone(),
            denom: "ujuno".to_string(),
            amount: Uint128::new(1),
        }],
    );

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let ce = err.downcast::<ContractError>().unwrap();
    match &ce {
        ContractError::ConstraintViolated { reason } => {
            assert!(
                reason.starts_with("post_hook:"),
                "expected post_hook prefix, got: {}",
                reason
            );
        }
        other => panic!("expected ConstraintViolated, got: {:?}", other),
    }

    // Atomic revert: status unchanged, no output_hash recorded, stats
    // counter not incremented. If any of these hold incorrect values the
    // post-hook short-circuit failed to roll the tx back.
    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.output_hash.is_none());

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_completed, 0);
}

#[test]
fn test_v7_balance_at_least_constraint() {
    // Funded wallet satisfies the constraint; zero-balance wallet
    // triggers ConstraintViolated. Uses the cw-multi-test bank module
    // so the querier resolves a real, verifiable balance.
    let alice = cosmwasm_std::testing::MockApi::default().addr_make("alice");
    let alice_fund = alice.clone();
    let mut app = App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &alice_fund, vec![Coin::new(1_000u128, "ujuno")])
            .unwrap();
    });
    let admin = mk(&app, "admin");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &alice, 1);

    // Happy path: alice has 1000 ujuno, constraint asks for ≥ 500.
    let t1 = submit_task_with_hooks(
        &mut app,
        &contract,
        &alice,
        1,
        vec![Constraint::BalanceAtLeast {
            who: alice.clone(),
            denom: "ujuno".to_string(),
            amount: Uint128::new(500),
        }],
        vec![],
    );
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: t1,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    // Violation path: asks for ≥ 2000.
    let t2 = submit_task_with_hooks(
        &mut app,
        &contract,
        &alice,
        1,
        vec![Constraint::BalanceAtLeast {
            who: alice.clone(),
            denom: "ujuno".to_string(),
            amount: Uint128::new(2_000),
        }],
        vec![],
    );
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: t2,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("BalanceAtLeast"));
}

#[test]
fn test_v7_pair_reserves_positive_constraint() {
    // An empty pool violates the liveness invariant; a funded pool
    // satisfies it. Proves the constraint correctly cross-contract
    // queries the pair's Pool response shape.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let pair = instantiate_stub_pair(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    // Pool starts with (0, 0) → constraint fails.
    let t1 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::PairReservesPositive { pair: pair.clone() }],
        vec![],
    );
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: t1,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("PairReservesPositive"));

    // Fund the pair and the same constraint now passes.
    set_stub_pair_reserves(&mut app, &pair, &admin, 1_000, 5_000);
    let t2 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::PairReservesPositive { pair: pair.clone() }],
        vec![],
    );
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: t2,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_v7_task_status_is_constraint_expresses_dependencies() {
    // Task B depends on Task A being Completed. Attempting to complete
    // B while A is still Running trips the constraint. Completing A
    // first then B succeeds.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    let task_a = submit_task(&mut app, &contract, &user, 1);
    let task_b = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::TaskStatusIs {
            task_ledger: contract.clone(),
            task_id: task_a,
            status: TaskStatus::Completed,
        }],
        vec![],
    );

    // Completing B while A is Running must fail.
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: task_b,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("TaskStatusIs"));

    // Complete A, then B's dependency is satisfied.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: task_a,
            output_hash: "a_done".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: task_b,
            output_hash: "b_done".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_v7_multiple_hooks_first_failure_wins_with_index() {
    // Two pre_hooks. The first passes; the second fails. The
    // `evaluate_all` helper preserves declaration order and surfaces
    // the failing index so a proposal author can diagnose exactly
    // which constraint tripped.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);
    set_stub_trust_score(&mut app, &reg, &admin, 1, 100);

    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![
            Constraint::AgentTrustAtLeast { agent_id: 1, min_score: 50 }, // passes (100 >= 50)
            Constraint::AgentTrustAtLeast { agent_id: 1, min_score: 200 }, // fails (100 < 200)
        ],
        vec![],
    );

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let msg = err.root_cause().to_string();
    assert!(
        msg.contains("hook[1]"),
        "expected hook[1] in error, got: {}",
        msg
    );
    // And the first hook (index 0) should not be mentioned as the
    // failure source.
    assert!(!msg.contains("hook[0]"), "hook[0] should not surface when hook[1] fails: {}", msg);
}

// ─────────────────────────────────────────────────────────────────
// v7 / Tier 1.5: TimeAfter, BlockHeightAtLeast, EscrowObligationConfirmed
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_v7_time_after_constraint() {
    // TimeAfter denies completion until the block time reaches a wall
    // clock threshold, then allows it. Proves the evaluator reads
    // `env.block.time` rather than some cached value from submit time.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    // Pin a threshold 1 hour after the current block time.
    let current = app.block_info().time.seconds();
    let threshold = current + 3600;
    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::TimeAfter { unix_seconds: threshold }],
        vec![],
    );

    // Too early: constraint trips.
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("TimeAfter"));

    // Advance the chain past the threshold. Same task, same hook,
    // now satisfied.
    app.update_block(|b| {
        b.time = b.time.plus_seconds(3601);
    });
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
}

#[test]
fn test_v7_block_height_at_least_constraint() {
    // BlockHeightAtLeast denies completion while the chain is below
    // the threshold and permits it once the chain has advanced past.
    // Separate from TimeAfter because some invariants care about
    // finality blocks rather than wall-clock (e.g. IBC acknowledgement
    // windows denominated in blocks).
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    let current_height = app.block_info().height;
    let threshold = current_height + 100;
    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::BlockHeightAtLeast { height: threshold }],
        vec![],
    );

    // Too early: constraint trips.
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("BlockHeightAtLeast"));

    // Advance past the threshold and retry the same task.
    app.update_block(|b| {
        b.height += 101;
    });
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
}

#[test]
fn test_v7_escrow_obligation_confirmed_constraint() {
    // EscrowObligationConfirmed ties task completion to payment
    // settlement. Three scenarios:
    //   (a) no obligation exists in escrow              → fail
    //   (b) obligation exists but status is Pending    → fail
    //   (c) obligation exists and status is Confirmed  → pass
    // Atomic-revert of (a) and (b) is what prevents a task from being
    // marked Completed without the payment journal agreeing.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let escrow = instantiate_stub_escrow(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    // (a) No obligation yet.
    let t1 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::EscrowObligationConfirmed {
            escrow: escrow.clone(),
            task_id: 42,
        }],
        vec![],
    );
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: t1,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let msg = err.root_cause().to_string();
    assert!(
        msg.contains("no obligation for task 42"),
        "expected missing-obligation diagnostic, got: {}",
        msg
    );

    // (b) Obligation exists but only Pending.
    set_stub_obligation(&mut app, &escrow, &admin, 42, ObligationStatus::Pending);
    let t2 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::EscrowObligationConfirmed {
            escrow: escrow.clone(),
            task_id: 42,
        }],
        vec![],
    );
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: t2,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let msg = err.root_cause().to_string();
    assert!(
        msg.contains("expected Confirmed"),
        "expected status-mismatch diagnostic, got: {}",
        msg
    );

    // (c) Flip obligation to Confirmed and the same hook now passes.
    set_stub_obligation(&mut app, &escrow, &admin, 42, ObligationStatus::Confirmed);
    let t3 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::EscrowObligationConfirmed {
            escrow: escrow.clone(),
            task_id: 42,
        }],
        vec![],
    );
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CompleteTask {
            task_id: t3,
            output_hash: "ok".to_string(),
            cost_ujuno: None,
        },
        &[],
    )
    .unwrap();

    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetTask { task_id: t3 })
        .unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
}

// ─────────────────────────────────────────────────────────────────
// Error-string shape regression — observability dependency
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_v7_constraint_violated_error_string_shape_is_stable() {
    // Regression-lock the layered prefix of `ContractError::ConstraintViolated::reason`:
    //
    //     reason == "{pre_hook|post_hook}: hook[{i}]: {VariantName}: {details}"
    //
    // This exact shape is what downstream dashboards grep tx failure
    // logs for. The layering is produced by three code sites:
    //
    //   1. `junoclaw_common::Constraint::evaluate` — adds
    //      "{VariantName}: {details}" per variant arm.
    //   2. `junoclaw_common::evaluate_all` — wraps with "hook[{i}]: ...".
    //   3. `task-ledger::execute_complete` — wraps with "pre_hook: ..." or
    //      "post_hook: ..." depending on which list is being evaluated.
    //
    // A silent refactor at any of the three sites would change the
    // prefix shape and silently break tx-log-grep observability
    // downstream. See docs/TIER15_ARCHITECTURE_UPGRADE.md §10.5 for the
    // standing observability requirement this test anchors.
    //
    // TimeAfter and BlockHeightAtLeast are used as the two exemplars
    // because they need no cross-contract stubs — just `env.block`
    // manipulation — so the test stays focused on the wrapper layering
    // and the variant-name emission, not stub plumbing. The wrapper
    // layering is variant-agnostic: if it is correct for these two it
    // is correct for all seven.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let user = mk(&app, "user1");
    let reg = instantiate_stub_registry(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &reg, None);
    register_stub_agent(&mut app, &reg, &user, 1);

    // ── pre_hook: TimeAfter ───────────────────────────────────────
    let future = app.block_info().time.seconds() + 3600;
    let task_id = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![Constraint::TimeAfter { unix_seconds: future }],
        vec![],
    );
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let ce = err.downcast::<ContractError>().unwrap();
    match &ce {
        ContractError::ConstraintViolated { reason } => {
            assert!(
                reason.starts_with("pre_hook: hook[0]: TimeAfter: "),
                "pre_hook/TimeAfter prefix drifted — dashboard grep will break. got: {}",
                reason,
            );
            assert!(
                reason.contains("block time"),
                "TimeAfter detail string missing 'block time': {}",
                reason,
            );
        }
        other => panic!("expected ConstraintViolated, got: {:?}", other),
    }

    // ── post_hook: BlockHeightAtLeast ────────────────────────────
    //
    // Using the other wrapper position (post_hook:) and a different
    // variant (BlockHeightAtLeast) proves (a) the post_hook wrapper in
    // execute_complete emits the expected literal, (b) the variant name
    // emission in evaluate is stable across arms. Together with the
    // TimeAfter case above this covers all three layering sources.
    let height = app.block_info().height + 100_000;
    let task_id2 = submit_task_with_hooks(
        &mut app,
        &contract,
        &user,
        1,
        vec![],
        vec![Constraint::BlockHeightAtLeast { height }],
    );
    let err2 = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::CompleteTask {
                task_id: task_id2,
                output_hash: "ok".to_string(),
                cost_ujuno: None,
            },
            &[],
        )
        .unwrap_err();
    let ce2 = err2.downcast::<ContractError>().unwrap();
    match &ce2 {
        ContractError::ConstraintViolated { reason } => {
            assert!(
                reason.starts_with("post_hook: hook[0]: BlockHeightAtLeast: "),
                "post_hook/BlockHeightAtLeast prefix drifted: {}",
                reason,
            );
            assert!(
                reason.contains("block height"),
                "BlockHeightAtLeast detail string missing 'block height': {}",
                reason,
            );
        }
        other => panic!("expected ConstraintViolated, got: {:?}", other),
    }

    // ── Outer Display layer ──────────────────────────────────────
    //
    // Independent of the `reason` field shape, the Display impl of
    // ContractError::ConstraintViolated prefixes "Constraint violated: "
    // (see contracts/task-ledger/src/error.rs). Tx-log-grep dashboards
    // that read cosmjs error.message (rather than downcasting the typed
    // error) see the full Display output, so lock that shape too.
    assert_eq!(
        ce2.to_string(),
        format!(
            "Constraint violated: post_hook: hook[0]: BlockHeightAtLeast: block height {} < required {}",
            app.block_info().height,
            height,
        ),
        "Display shape of ContractError::ConstraintViolated drifted",
    );
}
