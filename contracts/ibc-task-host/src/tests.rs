use cosmwasm_std::Addr;
use cosmwasm_std::coins;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::*;

struct TestAddrs {
    admin: Addr,
    task_ledger: Addr,
    escrow: Addr,
    zk_verifier: Addr,
    pair: Addr,
    sender: Addr,
    hacker: Addr,
}

fn make_addrs(app: &App) -> TestAddrs {
    TestAddrs {
        admin: app.api().addr_make("admin"),
        task_ledger: app.api().addr_make("task_ledger"),
        escrow: app.api().addr_make("escrow"),
        zk_verifier: app.api().addr_make("zk_verifier"),
        pair: app.api().addr_make("pair_juno_osmo"),
        sender: app.api().addr_make("ibc_module"),
        hacker: app.api().addr_make("hacker"),
    }
}

fn store_and_instantiate(app: &mut App, addrs: &TestAddrs) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        addrs.admin.clone(),
        &InstantiateMsg {
            admin: addrs.admin.to_string(),
            task_ledger: Some(addrs.task_ledger.to_string()),
            escrow: Some(addrs.escrow.to_string()),
            zk_verifier: Some(addrs.zk_verifier.to_string()),
            allowed_pairs: vec![addrs.pair.to_string()],
        },
        &[],
        "ibc-task-host",
        Some(addrs.admin.to_string()),
    )
    .unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let addrs = make_addrs(&app);
    let host = store_and_instantiate(&mut app, &addrs);

    let config: HostConfigResponse = app
        .wrap()
        .query_wasm_smart(&host, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.admin, addrs.admin);
    assert_eq!(config.task_ledger, Some(addrs.task_ledger.clone()));
    assert_eq!(config.allowed_pairs, vec![addrs.pair.clone()]);
}

#[test]
fn test_stats_initial_zero() {
    let mut app = App::default();
    let addrs = make_addrs(&app);
    let host = store_and_instantiate(&mut app, &addrs);

    let stats: HostStatsResponse = app
        .wrap()
        .query_wasm_smart(&host, &QueryMsg::Stats {})
        .unwrap();
    assert_eq!(stats.total_accept_task, 0);
    assert_eq!(stats.total_submit_proof, 0);
    assert_eq!(stats.total_reclaim, 0);
    assert_eq!(stats.total_swap, 0);
}

#[test]
fn test_swap_non_whitelisted_pair_rejected() {
    let mut app = App::default();
    let addrs = make_addrs(&app);
    let host = store_and_instantiate(&mut app, &addrs);
    let rogue = app.api().addr_make("rogue_pair");

    let msg = ExecuteMsg::JunoClawV1(JunoClawV1Op::Swap {
        pair_contract: rogue.to_string(),
        offer_denom: "ibc/27394F".into(),
        min_return: "900000".into(),
        agent_origin_chain: "osmosis-1".into(),
        agent_origin_addr: "osmo1agent".into(),
        max_price_impact_bps: None,
    });

    // Don't attach funds — whitelist check runs before fund handling.
    // cw-multi-test would reject the bank debit anyway (sender has no balance).
    let err = app
        .execute_contract(addrs.sender.clone(), host, &msg, &[])
        .unwrap_err();
    let err_str = err.root_cause().to_string();
    assert!(err_str.contains("Invalid pair contract"), "unexpected error: {}", err_str);
}

#[test]
fn test_swap_no_funds_rejected() {
    let mut app = App::default();
    let addrs = make_addrs(&app);
    let host = store_and_instantiate(&mut app, &addrs);

    let msg = ExecuteMsg::JunoClawV1(JunoClawV1Op::Swap {
        pair_contract: addrs.pair.to_string(),
        offer_denom: "ibc/27394F".into(),
        min_return: "900000".into(),
        agent_origin_chain: "osmosis-1".into(),
        agent_origin_addr: "osmo1agent".into(),
        max_price_impact_bps: None,
    });

    let err = app
        .execute_contract(addrs.sender.clone(), host, &msg, &[])
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("No funds"));
}

#[test]
fn test_update_config_admin_only() {
    let mut app = App::default();
    let addrs = make_addrs(&app);
    let host = store_and_instantiate(&mut app, &addrs);
    let new_tl = app.api().addr_make("new_task_ledger");
    let new_pair = app.api().addr_make("pair_juno_atom");

    // Non-admin rejected
    let msg = ExecuteMsg::UpdateConfig {
        task_ledger: Some(new_tl.to_string()),
        escrow: None,
        zk_verifier: None,
        allowed_pairs: None,
    };
    let err = app
        .execute_contract(addrs.hacker.clone(), host.clone(), &msg, &[])
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));

    // Admin succeeds
    let msg = ExecuteMsg::UpdateConfig {
        task_ledger: Some(new_tl.to_string()),
        escrow: None,
        zk_verifier: None,
        allowed_pairs: Some(vec![addrs.pair.to_string(), new_pair.to_string()]),
    };
    app.execute_contract(addrs.admin.clone(), host.clone(), &msg, &[]).unwrap();

    let config: HostConfigResponse = app
        .wrap()
        .query_wasm_smart(&host, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.task_ledger, Some(new_tl));
    assert_eq!(config.allowed_pairs.len(), 2);
}
