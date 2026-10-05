use cosmwasm_std::{
    to_json_binary, Addr, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw_multi_test::{App, ContractWrapper, Executor};
use cw_storage_plus::Map;

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, LedgerStats};
use junoclaw_common::{
    Constraint, ExecutionTier, ObligationStatus, PaymentObligation, TaskRecord, TaskStatus,
};

const UJUNO: &str = "ujuno";

const STUB_TASKS: Map<u64, TaskRecord> = Map::new("stub_tasks");
const STUB_BY_PROPOSAL: Map<u64, u64> = Map::new("stub_by_proposal");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubTaskLedgerExecuteMsg {
    SeedTask {
        task_id: u64,
        submitter: String,
        proposal_id: Option<u64>,
        pre_hooks: Vec<Constraint>,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubTaskLedgerQueryMsg {
    GetTask { task_id: u64 },
    GetTaskByProposal { proposal_id: u64 },
}

fn stub_task_ledger_instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Empty,
) -> StdResult<Response> {
    Ok(Response::new())
}

fn stub_task_ledger_execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: StubTaskLedgerExecuteMsg,
) -> StdResult<Response> {
    match msg {
        StubTaskLedgerExecuteMsg::SeedTask {
            task_id,
            submitter,
            proposal_id,
            pre_hooks,
        } => {
            STUB_TASKS.save(
                deps.storage,
                task_id,
                &TaskRecord {
                    id: task_id,
                    agent_id: 1,
                    submitter: Addr::unchecked(submitter),
                    input_hash: format!("hash-{}", task_id),
                    output_hash: None,
                    execution_tier: ExecutionTier::Local,
                    status: TaskStatus::Running,
                    submitted_at: 0,
                    completed_at: None,
                    cost_ujuno: None,
                    proposal_id,
                    pre_hooks,
                    post_hooks: vec![],
                },
            )?;
            if let Some(pid) = proposal_id {
                STUB_BY_PROPOSAL.save(deps.storage, pid, &task_id)?;
            }
            Ok(Response::new())
        }
    }
}

fn stub_task_ledger_query(
    deps: Deps,
    _env: Env,
    msg: StubTaskLedgerQueryMsg,
) -> StdResult<Binary> {
    match msg {
        StubTaskLedgerQueryMsg::GetTask { task_id } => {
            to_json_binary(&STUB_TASKS.load(deps.storage, task_id)?)
        }
        StubTaskLedgerQueryMsg::GetTaskByProposal { proposal_id } => {
            let task = match STUB_BY_PROPOSAL.may_load(deps.storage, proposal_id)? {
                Some(task_id) => STUB_TASKS.may_load(deps.storage, task_id)?,
                None => None,
            };
            to_json_binary(&task)
        }
    }
}

fn stub_task_ledger(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(
        stub_task_ledger_execute,
        stub_task_ledger_instantiate,
        stub_task_ledger_query,
    );
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &Empty {},
        &[],
        "stub-task-ledger",
        None,
    )
    .unwrap()
}

fn seed_task(
    app: &mut App,
    escrow: &Addr,
    task_id: u64,
    submitter: &Addr,
    proposal_id: Option<u64>,
    pre_hooks: Vec<Constraint>,
) {
    let config: Config = app
        .wrap()
        .query_wasm_smart(escrow, &QueryMsg::GetConfig {})
        .unwrap();
    app.execute_contract(
        submitter.clone(),
        config.task_ledger,
        &StubTaskLedgerExecuteMsg::SeedTask {
            task_id,
            submitter: submitter.to_string(),
            proposal_id,
            pre_hooks,
        },
        &[],
    )
    .unwrap();
}

fn try_authorize(
    app: &mut App,
    sender: &Addr,
    contract: &Addr,
    task_id: u64,
    payee: &Addr,
    amount: u128,
) -> Result<(), ContractError> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Authorize {
            task_id,
            payee: payee.to_string(),
            amount: Uint128::from(amount),
        },
        &[],
    )
    .map(|_| ())
    .map_err(|e| e.downcast::<ContractError>().unwrap())
}

fn obligation_for(app: &App, contract: &Addr, task_id: u64) -> Option<PaymentObligation> {
    app.wrap()
        .query_wasm_smart(contract, &QueryMsg::GetObligationByTask { task_id })
        .unwrap()
}

fn store_and_instantiate(app: &mut App, admin: &Addr, task_ledger: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            task_ledger: task_ledger.to_string(),
            timeout_blocks: 100,
            denom: Some(UJUNO.to_string()),
            registry: None,
        },
        &[],
        "payment-ledger",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn make_addr(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

fn authorize(app: &mut App, sender: &Addr, contract: &Addr, task_id: u64, payee: &Addr, amount: u128) {
    seed_task(app, contract, task_id, sender, None, vec![]);
    try_authorize(app, sender, contract, task_id, payee, amount).unwrap();
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_obligations, 0);
    assert!(stats.total_pending.is_zero());
    assert!(stats.total_confirmed.is_zero());
}

#[test]
fn test_authorize_and_query() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.task_id, 1);
    assert_eq!(obligation.amount, Uint128::from(1_000_000u128));
    assert_eq!(obligation.status, ObligationStatus::Pending);
    assert_eq!(obligation.payer, payer);
    assert_eq!(obligation.payee, payee);
    assert!(obligation.attestation_hash.is_none());

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_obligations, 1);
    assert_eq!(stats.total_pending, Uint128::from(1_000_000u128));
}

#[test]
fn test_authorize_zero_amount_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Authorize {
                task_id: 1,
                payee: payee.to_string(),
                amount: Uint128::zero(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ZeroAmount {}));
}

#[test]
fn test_double_authorize_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Authorize {
                task_id: 1,
                payee: payee.to_string(),
                amount: Uint128::from(500_000u128),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AlreadyAuthorized { .. }));
}

#[test]
fn test_confirm_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Confirm { task_id: 1, tx_hash: Some("ABCDEF123".to_string()) },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Confirmed);
    assert!(obligation.settled_at.is_some());

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_confirmed, Uint128::from(1_000_000u128));
    assert!(stats.total_pending.is_zero());
}

#[test]
fn test_confirm_by_task_ledger() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let task_ledger = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &task_ledger);

    authorize(&mut app, &payer, &contract, 1, &payee, 2_000_000);

    app.execute_contract(
        task_ledger.clone(),
        contract.clone(),
        &ExecuteMsg::Confirm { task_id: 1, tx_hash: None },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Confirmed);
}

#[test]
fn test_dispute_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Dispute { task_id: 1, reason: "Task not completed".to_string() },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Disputed);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_disputed, Uint128::from(1_000_000u128));
    assert!(stats.total_pending.is_zero());
}

#[test]
fn test_dispute_unauthorized_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let stranger = make_addr(&app, "stranger");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            stranger.clone(),
            contract.clone(),
            &ExecuteMsg::Dispute { task_id: 1, reason: "fraud".to_string() },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_cancel_by_payer() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Cancelled);

    let stats: LedgerStats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_cancelled, Uint128::from(1_000_000u128));
}

#[test]
fn test_cancel_by_admin() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Cancelled);
}

#[test]
fn test_confirm_not_pending_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    // Cancel first
    app.execute_contract(
        payer.clone(),
        contract.clone(),
        &ExecuteMsg::Cancel { task_id: 1 },
        &[],
    )
    .unwrap();

    // Try to confirm a cancelled obligation
    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::Confirm { task_id: 1, tx_hash: None },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotPending { .. }));
}

#[test]
fn test_attach_attestation() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AttachAttestation {
            task_id: 1,
            attestation_hash: "wavs_hash_abc123".to_string(),
        },
        &[],
    )
    .unwrap();

    let obligation: PaymentObligation = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetObligationByTask { task_id: 1 })
        .unwrap();
    assert_eq!(obligation.status, ObligationStatus::Verified);
    assert_eq!(obligation.attestation_hash, Some("wavs_hash_abc123".to_string()));
}

#[test]
fn test_attach_attestation_unauthorized_fails() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);

    let err = app
        .execute_contract(
            payer.clone(),
            contract.clone(),
            &ExecuteMsg::AttachAttestation {
                task_id: 1,
                attestation_hash: "fake".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_list_obligations() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    authorize(&mut app, &payer, &contract, 1, &payee, 1_000_000);
    authorize(&mut app, &payer, &contract, 2, &payee, 2_000_000);
    authorize(&mut app, &payer, &contract, 3, &payee, 3_000_000);

    let obligations: Vec<PaymentObligation> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListObligations { start_after: None, limit: Some(10) })
        .unwrap();
    assert_eq!(obligations.len(), 3);
    assert_eq!(obligations[0].task_id, 1);
    assert_eq!(obligations[2].task_id, 3);
}

#[test]
fn test_authorize_rejects_stranger_and_leaves_slot_free() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let owner = make_addr(&app, "owner");
    let stranger = make_addr(&app, "stranger");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);
    seed_task(&mut app, &contract, 1, &owner, None, vec![]);

    let err = try_authorize(&mut app, &stranger, &contract, 1, &payee, 1_000_000).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
    assert!(obligation_for(&app, &contract, 1).is_none());

    try_authorize(&mut app, &owner, &contract, 1, &payee, 1_000_000).unwrap();
    let obligation = obligation_for(&app, &contract, 1).unwrap();
    assert_eq!(obligation.payer, owner);
}

#[test]
fn test_authorize_unknown_task_rejected() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let payer = make_addr(&app, "payer");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);

    let err = try_authorize(&mut app, &payer, &contract, 77, &payee, 1_000_000).unwrap_err();
    assert!(matches!(err, ContractError::TaskNotFound { task_id: 77 }));
    assert!(obligation_for(&app, &contract, 77).is_none());
}

#[test]
fn test_authorize_pinned_payer_must_match_pins() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let owner = make_addr(&app, "owner");
    let customer = make_addr(&app, "customer");
    let stranger = make_addr(&app, "stranger");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);
    seed_task(
        &mut app,
        &contract,
        5,
        &owner,
        None,
        vec![Constraint::EscrowObligationConfirmed {
            escrow: contract.clone(),
            task_id: 5,
            payer: Some(customer.clone()),
            payee: Some(owner.clone()),
            min_amount: Some(Uint128::new(250_000)),
        }],
    );

    let err = try_authorize(&mut app, &stranger, &contract, 5, &owner, 250_000).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
    let err = try_authorize(&mut app, &owner, &contract, 5, &owner, 250_000).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
    let err = try_authorize(&mut app, &customer, &contract, 5, &customer, 250_000).unwrap_err();
    assert!(matches!(err, ContractError::PinMismatch { task_id: 5 }));
    let err = try_authorize(&mut app, &customer, &contract, 5, &owner, 249_999).unwrap_err();
    assert!(matches!(err, ContractError::PinMismatch { task_id: 5 }));
    assert!(obligation_for(&app, &contract, 5).is_none());

    try_authorize(&mut app, &customer, &contract, 5, &owner, 250_000).unwrap();
    let obligation = obligation_for(&app, &contract, 5).unwrap();
    assert_eq!(obligation.payer, customer);
    assert_eq!(obligation.payee, owner);
    assert_eq!(obligation.amount, Uint128::new(250_000));
}

#[test]
fn test_pin_scoped_to_other_escrow_or_task_grants_nothing() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let owner = make_addr(&app, "owner");
    let stranger = make_addr(&app, "stranger");
    let payee = make_addr(&app, "payee");
    let other_escrow = make_addr(&app, "other-escrow");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);
    seed_task(
        &mut app,
        &contract,
        6,
        &owner,
        None,
        vec![
            Constraint::EscrowObligationConfirmed {
                escrow: other_escrow,
                task_id: 6,
                payer: Some(stranger.clone()),
                payee: None,
                min_amount: None,
            },
            Constraint::EscrowObligationConfirmed {
                escrow: contract.clone(),
                task_id: 99,
                payer: Some(stranger.clone()),
                payee: None,
                min_amount: None,
            },
        ],
    );

    let err = try_authorize(&mut app, &stranger, &contract, 6, &payee, 1).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
    try_authorize(&mut app, &owner, &contract, 6, &payee, 1).unwrap();
}

#[test]
fn test_authorize_resolves_proposal_keyed_tasks() {
    let mut app = App::default();
    let admin = make_addr(&app, "admin");
    let company = make_addr(&app, "company");
    let stranger = make_addr(&app, "stranger");
    let payee = make_addr(&app, "payee");
    let tl = stub_task_ledger(&mut app, &admin);
    let contract = store_and_instantiate(&mut app, &admin, &tl);
    seed_task(&mut app, &contract, 3, &company, Some(9), vec![]);
    seed_task(&mut app, &contract, 4, &company, Some(10), vec![]);

    let err = try_authorize(&mut app, &stranger, &contract, 10, &payee, 1).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
    let err = try_authorize(&mut app, &company, &contract, 4, &payee, 1).unwrap_err();
    assert!(matches!(err, ContractError::TaskNotFound { task_id: 4 }));

    try_authorize(&mut app, &company, &contract, 9, &payee, 1).unwrap();
    assert!(obligation_for(&app, &contract, 9).is_some());
    assert!(obligation_for(&app, &contract, 3).is_none());
}
