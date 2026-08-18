use cosmwasm_std::{
    coins, to_json_binary, Addr, Binary, CustomMsg, CustomQuery, Deps, DepsMut, Empty, Env,
    MessageInfo, Reply, Response, Uint128,
};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_storage_plus::Map;

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Hire, HireStatus, Listing, MarketplaceStats};
use junoclaw_common::{TaskRecord, TaskStatus};

const UJUNO: &str = "ujuno";

// ── Stub task-ledger ──
// Responds to `GetTask { task_id }` with a `TaskRecord` whose status can
// be pre-seeded by a test-only `SetTaskStatus` execute. Avoids pulling
// the full task-ledger crate in as a dev-dependency.

const STUB_TASKS: Map<u64, TaskRecord> = Map::new("stub_tasks");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubTaskLedgerExecuteMsg {
    SetTaskStatus { task_id: u64, status: TaskStatus },
    SeedTask { task_id: u64, submitter: String },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubTaskLedgerQueryMsg {
    GetTask { task_id: u64 },
}

struct StubTaskLedger;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubTaskLedger
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
        if let Ok(msg) = cosmwasm_std::from_json::<StubTaskLedgerExecuteMsg>(&msg) {
            match msg {
                StubTaskLedgerExecuteMsg::SetTaskStatus { task_id, status } => {
                    let mut task = STUB_TASKS.load(deps.storage, task_id)?;
                    task.status = status;
                    STUB_TASKS.save(deps.storage, task_id, &task)?;
                }
                StubTaskLedgerExecuteMsg::SeedTask { task_id, submitter } => {
                    STUB_TASKS.save(
                        deps.storage,
                        task_id,
                        &TaskRecord {
                            id: task_id,
                            agent_id: 1,
                            submitter: Addr::unchecked(submitter),
                            input_hash: format!("hash-{}", task_id),
                            output_hash: None,
                            execution_tier: junoclaw_common::ExecutionTier::Local,
                            status: TaskStatus::Running,
                            submitted_at: 0,
                            completed_at: None,
                            cost_ujuno: None,
                            proposal_id: None,
                            pre_hooks: vec![],
                            post_hooks: vec![],
                        },
                    )?;
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
    fn query(&self, deps: Deps<QueryC>, _env: Env, msg: Vec<u8>) -> anyhow::Result<Binary> {
        if let Ok(StubTaskLedgerQueryMsg::GetTask { task_id }) =
            cosmwasm_std::from_json::<StubTaskLedgerQueryMsg>(&msg)
        {
            let task = STUB_TASKS.load(deps.storage, task_id)?;
            return Ok(to_json_binary(&task)?);
        }
        Ok(to_json_binary(&()).unwrap())
    }
    fn sudo(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn reply(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Reply) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn migrate(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
}

fn instantiate_stub_task_ledger(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubTaskLedger));
    app.instantiate_contract(code_id, admin.clone(), &Empty {}, &[], "stub-task-ledger", None)
        .unwrap()
}

// ── Stub truth-market ──
// Responds to `GetEpoch { batch_height }` with an epoch view. A test-only
// `SetEpoch` execute seeds the finalized verdict for a batch height.

#[derive(serde::Serialize)]
struct StubEpochView {
    consensus_verdict: String,
    finalized: bool,
}

const STUB_EPOCHS: Map<u64, (String, bool)> = Map::new("stub_epochs");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubTruthMarketExecuteMsg {
    SetEpoch {
        batch_height: u64,
        consensus_verdict: String,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubTruthMarketQueryMsg {
    GetEpoch { batch_height: u64 },
}

struct StubTruthMarket;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubTruthMarket
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
        if let Ok(StubTruthMarketExecuteMsg::SetEpoch { batch_height, consensus_verdict }) =
            cosmwasm_std::from_json::<StubTruthMarketExecuteMsg>(&msg)
        {
            STUB_EPOCHS.save(deps.storage, batch_height, &(consensus_verdict, true))?;
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
    fn query(&self, deps: Deps<QueryC>, _env: Env, msg: Vec<u8>) -> anyhow::Result<Binary> {
        if let Ok(StubTruthMarketQueryMsg::GetEpoch { batch_height }) =
            cosmwasm_std::from_json::<StubTruthMarketQueryMsg>(&msg)
        {
            let (verdict, finalized) = STUB_EPOCHS.load(deps.storage, batch_height)?;
            return Ok(to_json_binary(&StubEpochView {
                consensus_verdict: verdict,
                finalized,
            })?);
        }
        Ok(to_json_binary(&()).unwrap())
    }
    fn sudo(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn reply(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Reply) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn migrate(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
}

fn instantiate_stub_truth_market(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubTruthMarket));
    app.instantiate_contract(code_id, admin.clone(), &Empty {}, &[], "stub-truth-market", None)
        .unwrap()
}

// ── Stub skill-registry ──
// Responds to `GetSkill { dapp_name }` — returns the entry if it was
// pre-seeded via `SeedSkill`, otherwise a query error (mirrors the real
// skill-registry's not-found behavior).

const STUB_SKILLS: Map<String, String> = Map::new("stub_skills");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum StubSkillRegistryExecuteMsg {
    SeedSkill { dapp_name: String },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubSkillRegistryQueryMsg {
    GetSkill { dapp_name: String },
}

#[derive(serde::Serialize)]
struct StubSkillEntryView {
    dapp_name: String,
}

struct StubSkillRegistry;

impl<ExecC, QueryC> Contract<ExecC, QueryC> for StubSkillRegistry
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
        if let Ok(StubSkillRegistryExecuteMsg::SeedSkill { dapp_name }) =
            cosmwasm_std::from_json::<StubSkillRegistryExecuteMsg>(&msg)
        {
            STUB_SKILLS.save(deps.storage, dapp_name.clone(), &dapp_name)?;
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
    fn query(&self, deps: Deps<QueryC>, _env: Env, msg: Vec<u8>) -> anyhow::Result<Binary> {
        if let Ok(StubSkillRegistryQueryMsg::GetSkill { dapp_name }) =
            cosmwasm_std::from_json::<StubSkillRegistryQueryMsg>(&msg)
        {
            let found = STUB_SKILLS.load(deps.storage, dapp_name)?;
            return Ok(to_json_binary(&StubSkillEntryView { dapp_name: found })?);
        }
        Ok(to_json_binary(&()).unwrap())
    }
    fn sudo(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn reply(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Reply) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
    fn migrate(&self, _deps: DepsMut<QueryC>, _env: Env, _msg: Vec<u8>) -> anyhow::Result<Response<ExecC>> {
        Ok(Response::new())
    }
}

fn instantiate_stub_skill_registry(app: &mut App, admin: &Addr) -> Addr {
    let code_id = app.store_code(Box::new(StubSkillRegistry));
    app.instantiate_contract(code_id, admin.clone(), &Empty {}, &[], "stub-skill-registry", None)
        .unwrap()
}

fn seed_skill(app: &mut App, skill_registry: &Addr, admin: &Addr, dapp_name: &str) {
    app.execute_contract(
        admin.clone(),
        skill_registry.clone(),
        &StubSkillRegistryExecuteMsg::SeedSkill { dapp_name: dapp_name.to_string() },
        &[],
    )
    .unwrap();
}

fn set_epoch(app: &mut App, truth_market: &Addr, admin: &Addr, batch_height: u64, verdict: &str) {
    app.execute_contract(
        admin.clone(),
        truth_market.clone(),
        &StubTruthMarketExecuteMsg::SetEpoch {
            batch_height,
            consensus_verdict: verdict.to_string(),
        },
        &[],
    )
    .unwrap();
}

fn seed_task_record(app: &mut App, task_ledger: &Addr, caller: &Addr, task_id: u64, submitter: &Addr) {
    app.execute_contract(
        caller.clone(),
        task_ledger.clone(),
        &StubTaskLedgerExecuteMsg::SeedTask {
            task_id,
            submitter: submitter.to_string(),
        },
        &[],
    )
    .unwrap();
}

fn set_task_status(app: &mut App, task_ledger: &Addr, caller: &Addr, task_id: u64, status: TaskStatus) {
    app.execute_contract(
        caller.clone(),
        task_ledger.clone(),
        &StubTaskLedgerExecuteMsg::SetTaskStatus { task_id, status },
        &[],
    )
    .unwrap();
}

fn mk_addr(label: &str) -> Addr {
    cosmwasm_std::testing::MockApi::default().addr_make(label)
}

fn setup_app(client: &Addr) -> App {
    let client = client.clone();
    App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &client, coins(10_000_000, UJUNO))
            .unwrap();
    })
}

fn store_and_instantiate(
    app: &mut App,
    admin: &Addr,
    truth_market: &Addr,
    task_ledger: &Addr,
    cancel_window_secs: Option<u64>,
) -> Addr {
    store_and_instantiate_with_registry(app, admin, truth_market, task_ledger, None, cancel_window_secs)
}

fn store_and_instantiate_with_registry(
    app: &mut App,
    admin: &Addr,
    truth_market: &Addr,
    task_ledger: &Addr,
    skill_registry: Option<&Addr>,
    cancel_window_secs: Option<u64>,
) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: None,
            truth_market: truth_market.to_string(),
            task_ledger: task_ledger.to_string(),
            skill_registry: skill_registry.map(|a| a.to_string()),
            denom: Some(UJUNO.to_string()),
            cancel_window_secs,
        },
        &[],
        "marketplace",
        Some(admin.to_string()),
    )
    .unwrap()
}

struct Fixture {
    app: App,
    admin: Addr,
    agent: Addr,
    client: Addr,
    task_ledger: Addr,
    truth_market: Addr,
    marketplace: Addr,
}

fn fixture() -> Fixture {
    let admin = mk_addr("admin");
    let agent = mk_addr("agent1");
    let client = mk_addr("client1");
    let mut app = setup_app(&client);
    let task_ledger = instantiate_stub_task_ledger(&mut app, &admin);
    let truth_market = instantiate_stub_truth_market(&mut app, &admin);
    let marketplace = store_and_instantiate(&mut app, &admin, &truth_market, &task_ledger, None);
    Fixture {
        app,
        admin,
        agent,
        client,
        task_ledger,
        truth_market,
        marketplace,
    }
}

fn list_service(f: &mut Fixture, price: u128) -> u64 {
    f.app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ListService {
                skill_ref: "j-lens-probe-audit".to_string(),
                price: Uint128::new(price),
                description: "Cognitive integrity audit for RL locomotion policies".to_string(),
            },
            &[],
        )
        .unwrap();
    let stats: MarketplaceStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetStats {})
        .unwrap();
    stats.total_listings
}

fn hire_service(f: &mut Fixture, listing_id: u64, task_id: u64, amount: u128) {
    seed_task_record(&mut f.app, &f.task_ledger, &f.admin, task_id, &f.client);
    f.app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::HireService { listing_id, task_id },
            &coins(amount, UJUNO),
        )
        .unwrap();
}

#[test]
fn test_instantiate() {
    let f = fixture();
    let config: crate::state::Config = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(config.admin, f.admin);
    assert_eq!(config.denom, UJUNO);
    assert_eq!(config.cancel_window_secs, 3600);
}

#[test]
fn test_list_service() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    assert_eq!(listing_id, 1);

    let listing: Listing = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetListing { listing_id })
        .unwrap();
    assert_eq!(listing.agent, f.agent);
    assert_eq!(listing.price, Uint128::new(1_000_000));
    assert!(listing.active);
}

#[test]
fn test_list_service_rejects_zero_price() {
    let mut f = fixture();
    let err = f
        .app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ListService {
                skill_ref: "skill".to_string(),
                price: Uint128::zero(),
                description: "desc".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ZeroPrice {}));
}

#[test]
fn test_delist_service() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);

    f.app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::DelistService { listing_id },
            &[],
        )
        .unwrap();

    let listing: Listing = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetListing { listing_id })
        .unwrap();
    assert!(!listing.active);

    let stats: MarketplaceStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.active_listings, 0);
}

#[test]
fn test_delist_unauthorized_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::DelistService { listing_id },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_hire_service_escrows_funds() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    let hire: Hire = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHire { hire_id: 1 })
        .unwrap();
    assert_eq!(hire.client, f.client);
    assert_eq!(hire.agent, f.agent);
    assert_eq!(hire.amount, Uint128::new(1_000_000));
    assert!(matches!(hire.status, HireStatus::Escrowed));

    let contract_balance = f.app.wrap().query_balance(&f.marketplace, UJUNO).unwrap();
    assert_eq!(contract_balance.amount, Uint128::new(1_000_000));
}

#[test]
fn test_hire_service_wrong_amount_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    seed_task_record(&mut f.app, &f.task_ledger, &f.admin, 1, &f.client);

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::HireService { listing_id, task_id: 1 },
            &coins(500_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::WrongFunds { .. }));
}

#[test]
fn test_hire_service_inactive_listing_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    f.app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::DelistService { listing_id },
            &[],
        )
        .unwrap();
    seed_task_record(&mut f.app, &f.task_ledger, &f.admin, 1, &f.client);

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::HireService { listing_id, task_id: 1 },
            &coins(1_000_000, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::ListingNotActive { .. }));
}

#[test]
fn test_release_on_green_verdict_pays_agent() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    set_task_status(&mut f.app, &f.task_ledger, &f.admin, 1, TaskStatus::Completed);
    set_epoch(&mut f.app, &f.truth_market, &f.admin, 42, "green");

    f.app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict {
                hire_id: 1,
                batch_height: 42,
            },
            &[],
        )
        .unwrap();

    let hire: Hire = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHire { hire_id: 1 })
        .unwrap();
    assert!(matches!(hire.status, HireStatus::Released));

    let agent_balance = f.app.wrap().query_balance(&f.agent, UJUNO).unwrap();
    assert_eq!(agent_balance.amount, Uint128::new(1_000_000));

    let stats: MarketplaceStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_released, Uint128::new(1_000_000));
}

#[test]
fn test_release_on_red_verdict_refunds_client() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    set_task_status(&mut f.app, &f.task_ledger, &f.admin, 1, TaskStatus::Completed);
    set_epoch(&mut f.app, &f.truth_market, &f.admin, 42, "red");

    let balance_before = f.app.wrap().query_balance(&f.client, UJUNO).unwrap();

    f.app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict {
                hire_id: 1,
                batch_height: 42,
            },
            &[],
        )
        .unwrap();

    let hire: Hire = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHire { hire_id: 1 })
        .unwrap();
    assert!(matches!(hire.status, HireStatus::Slashed));

    let balance_after = f.app.wrap().query_balance(&f.client, UJUNO).unwrap();
    assert_eq!(balance_after.amount, balance_before.amount + Uint128::new(1_000_000));

    let stats: MarketplaceStats = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_slashed, Uint128::new(1_000_000));
}

#[test]
fn test_release_refunds_on_failed_task_without_epoch() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    set_task_status(&mut f.app, &f.task_ledger, &f.admin, 1, TaskStatus::Failed);

    // No epoch seeded for batch_height 99 — must not be required for a
    // Failed task since there is nothing to verify.
    f.app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict {
                hire_id: 1,
                batch_height: 99,
            },
            &[],
        )
        .unwrap();

    let hire: Hire = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHire { hire_id: 1 })
        .unwrap();
    assert!(matches!(hire.status, HireStatus::Refunded));
}

#[test]
fn test_release_before_completion_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);
    // task remains Running (default seed status)

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict {
                hire_id: 1,
                batch_height: 42,
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::TaskNotCompleted { .. }));
}

#[test]
fn test_release_before_epoch_finalized_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);
    set_task_status(&mut f.app, &f.task_ledger, &f.admin, 1, TaskStatus::Completed);
    // No epoch seeded — batch_height 42 has no finalized verdict yet.

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict {
                hire_id: 1,
                batch_height: 42,
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::EpochNotFinalized { .. }));
}

#[test]
fn test_double_release_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);
    set_task_status(&mut f.app, &f.task_ledger, &f.admin, 1, TaskStatus::Completed);
    set_epoch(&mut f.app, &f.truth_market, &f.admin, 42, "green");

    f.app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict { hire_id: 1, batch_height: 42 },
            &[],
        )
        .unwrap();

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ReleaseOnVerdict { hire_id: 1, batch_height: 42 },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotEscrowed { .. }));
}

#[test]
fn test_cancel_hire_before_window_fails() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    let err = f
        .app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::CancelHire { hire_id: 1 },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::CancelWindowNotElapsed { .. }));
}

#[test]
fn test_cancel_hire_after_window_refunds_client() {
    let admin = mk_addr("admin");
    let agent = mk_addr("agent1");
    let client = mk_addr("client1");
    let mut app = setup_app(&client);
    let task_ledger = instantiate_stub_task_ledger(&mut app, &admin);
    let truth_market = instantiate_stub_truth_market(&mut app, &admin);
    // Zero-second cancel window so the test doesn't need to fast-forward
    // block time.
    let marketplace = store_and_instantiate(&mut app, &admin, &truth_market, &task_ledger, Some(0));
    let mut f = Fixture {
        app,
        admin,
        agent,
        client,
        task_ledger,
        truth_market,
        marketplace,
    };

    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 1, 1_000_000);

    let balance_before = f.app.wrap().query_balance(&f.client, UJUNO).unwrap();

    f.app
        .execute_contract(
            f.client.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::CancelHire { hire_id: 1 },
            &[],
        )
        .unwrap();

    let hire: Hire = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHire { hire_id: 1 })
        .unwrap();
    assert!(matches!(hire.status, HireStatus::Cancelled));

    let balance_after = f.app.wrap().query_balance(&f.client, UJUNO).unwrap();
    assert_eq!(balance_after.amount, balance_before.amount + Uint128::new(1_000_000));
}

#[test]
fn test_get_hire_by_task() {
    let mut f = fixture();
    let listing_id = list_service(&mut f, 1_000_000);
    hire_service(&mut f, listing_id, 7, 1_000_000);

    let hire: Option<Hire> = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetHireByTask { task_id: 7 })
        .unwrap();
    assert_eq!(hire.unwrap().id, 1);
}

#[test]
fn test_list_listings_by_agent() {
    let mut f = fixture();
    list_service(&mut f, 1_000_000);
    list_service(&mut f, 2_000_000);

    let listings: Vec<Listing> = f
        .app
        .wrap()
        .query_wasm_smart(
            &f.marketplace,
            &QueryMsg::ListListingsByAgent {
                agent: f.agent.to_string(),
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(listings.len(), 2);
}

fn fixture_with_registry() -> (Fixture, Addr) {
    let admin = mk_addr("admin");
    let agent = mk_addr("agent1");
    let client = mk_addr("client1");
    let mut app = setup_app(&client);
    let task_ledger = instantiate_stub_task_ledger(&mut app, &admin);
    let truth_market = instantiate_stub_truth_market(&mut app, &admin);
    let skill_registry = instantiate_stub_skill_registry(&mut app, &admin);
    let marketplace = store_and_instantiate_with_registry(
        &mut app,
        &admin,
        &truth_market,
        &task_ledger,
        Some(&skill_registry),
        None,
    );
    (
        Fixture {
            app,
            admin,
            agent,
            client,
            task_ledger,
            truth_market,
            marketplace,
        },
        skill_registry,
    )
}

#[test]
fn test_list_service_rejects_unregistered_skill() {
    let (mut f, _skill_registry) = fixture_with_registry();
    let err = f
        .app
        .execute_contract(
            f.agent.clone(),
            f.marketplace.clone(),
            &ExecuteMsg::ListService {
                skill_ref: "j-lens-probe-audit".to_string(),
                price: Uint128::new(1_000_000),
                description: "Cognitive integrity audit for RL locomotion policies".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::SkillNotRegistered { .. }));
}

#[test]
fn test_list_service_accepts_registered_skill() {
    let (mut f, skill_registry) = fixture_with_registry();
    seed_skill(&mut f.app, &skill_registry, &f.admin, "j-lens-probe-audit");

    let listing_id = list_service(&mut f, 1_000_000);
    assert_eq!(listing_id, 1);

    let listing: Listing = f
        .app
        .wrap()
        .query_wasm_smart(&f.marketplace, &QueryMsg::GetListing { listing_id })
        .unwrap();
    assert_eq!(listing.skill_ref, "j-lens-probe-audit");
}
