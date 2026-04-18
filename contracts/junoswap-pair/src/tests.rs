use cosmwasm_std::{Addr, Coin, Uint128};
use cosmwasm_std::testing::MockApi;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::*;
use junoclaw_common::AssetInfo;

fn mk(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

fn store_and_instantiate(app: &mut App, factory: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        factory.clone(),
        &InstantiateMsg {
            token_a: AssetInfo::Native("ujuno".to_string()),
            token_b: AssetInfo::Native("uusdc".to_string()),
            fee_bps: 30,
            factory: factory.to_string(),
            junoclaw_contract: None,
        },
        &[],
        "junoswap-pair",
        None,
    )
    .unwrap()
}

fn provide(app: &mut App, pair: &Addr, sender: &Addr, juno: u128, usdc: u128) {
    let funds = vec![
        Coin::new(juno, "ujuno"),
        Coin::new(usdc, "uusdc"),
    ];
    app.execute_contract(
        sender.clone(),
        pair.clone(),
        &ExecuteMsg::ProvideLiquidity {},
        &funds,
    )
    .unwrap();
}

fn q<T: serde::de::DeserializeOwned>(app: &App, pair: &Addr, msg: &QueryMsg) -> T {
    app.wrap().query_wasm_smart(pair, msg).unwrap()
}

#[test]
fn test_instantiate_pair() {
    let mut app = App::default();
    let factory = mk(&app, "factory");
    let pair = store_and_instantiate(&mut app, &factory);

    let res: PairInfoResponse = q(&app, &pair, &QueryMsg::PairInfo {});
    assert_eq!(res.factory, factory);
    assert_eq!(res.fee_bps, 30);
}

#[test]
fn test_provide_liquidity_rejects_unexpected_denom() {
    // v6 F4 — `ProvideLiquidity` previously silently dropped any denom
    // in `info.funds` that wasn't `token_a` or `token_b`, absorbing
    // those funds into the pair contract's bank balance with no way
    // for the depositor to reclaim them. The pair now fails closed so
    // the revert refunds every attached coin atomically.
    let alice = MockApi::default().addr_make("alice");
    let alice_clone = alice.clone();
    let mut app = App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &alice_clone, vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
                Coin::new(1_000u128, "uatom"), // intruder denom
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let pair = store_and_instantiate(&mut app, &factory);

    let err = app
        .execute_contract(
            alice.clone(),
            pair.clone(),
            &ExecuteMsg::ProvideLiquidity {},
            &[
                Coin::new(1_000_000u128, "ujuno"),
                Coin::new(5_000_000u128, "uusdc"),
                Coin::new(1_000u128, "uatom"),
            ],
        )
        .unwrap_err();
    assert!(
        err.root_cause().to_string().contains("Unexpected denom"),
        "expected UnexpectedDenom, got: {}",
        err.root_cause()
    );

    // Alice's `uatom` must still be on her balance — the revert refunded
    // every attached coin, including the two legitimate ones.
    let atom_bal = app.wrap().query_balance(alice.to_string(), "uatom").unwrap();
    assert_eq!(atom_bal.amount, Uint128::new(1_000));
    let juno_bal = app.wrap().query_balance(alice.to_string(), "ujuno").unwrap();
    assert_eq!(juno_bal.amount, Uint128::new(10_000_000));
}

#[test]
fn test_provide_initial_liquidity() {
    let alice = MockApi::default().addr_make("alice");
    let alice_clone = alice.clone();
    let mut app = App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &alice_clone, vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let pair = store_and_instantiate(&mut app, &factory);

    provide(&mut app, &pair, &alice, 1_000_000, 5_000_000);

    let pool: PoolStateResponse = q(&app, &pair, &QueryMsg::Pool {});
    assert_eq!(pool.reserve_a, Uint128::new(1_000_000));
    assert_eq!(pool.reserve_b, Uint128::new(5_000_000));
    assert!(!pool.total_lp_shares.is_zero());

    let lp: Uint128 = q(
        &app,
        &pair,
        &QueryMsg::LpBalance {
            address: alice.to_string(),
        },
    );
    assert!(!lp.is_zero());
}

#[test]
fn test_swap_and_constant_product() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("alice_init"), vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
            ])
            .unwrap();
        router
            .bank
            .init_balance(storage, &Addr::unchecked("bob_init"), vec![
                Coin::new(1_000_000u128, "ujuno"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let alice = Addr::unchecked("alice_init");
    let bob = Addr::unchecked("bob_init");
    let pair = store_and_instantiate(&mut app, &factory);

    provide(&mut app, &pair, &alice, 1_000_000, 5_000_000);

    let pool_before: PoolStateResponse = q(&app, &pair, &QueryMsg::Pool {});
    let k_before = pool_before.reserve_a.u128() * pool_before.reserve_b.u128();

    // Swap ujuno → uusdc
    let res = app
        .execute_contract(
            bob.clone(),
            pair.clone(),
            &ExecuteMsg::Swap {
                offer_asset: AssetInfo::Native("ujuno".to_string()),
                min_return: None,
            },
            &[Coin::new(100_000u128, "ujuno")],
        )
        .unwrap();

    // Check return amount in attributes
    let return_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "return_amount")
        .unwrap();
    let return_amount: u128 = return_attr.value.parse().unwrap();
    assert!(return_amount > 0, "swap must return tokens");

    // k must not decrease (fees increase it)
    let pool_after: PoolStateResponse = q(&app, &pair, &QueryMsg::Pool {});
    let k_after = pool_after.reserve_a.u128() * pool_after.reserve_b.u128();
    assert!(k_after >= k_before, "k must not decrease");
    assert_eq!(pool_after.total_swaps, 1);
}

#[test]
fn test_swap_slippage_protection() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("alice_init"), vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
            ])
            .unwrap();
        router
            .bank
            .init_balance(storage, &Addr::unchecked("bob_init"), vec![
                Coin::new(1_000_000u128, "ujuno"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let alice = Addr::unchecked("alice_init");
    let bob = Addr::unchecked("bob_init");
    let pair = store_and_instantiate(&mut app, &factory);

    provide(&mut app, &pair, &alice, 1_000_000, 5_000_000);

    let err = app
        .execute_contract(
            bob.clone(),
            pair.clone(),
            &ExecuteMsg::Swap {
                offer_asset: AssetInfo::Native("ujuno".to_string()),
                min_return: Some(Uint128::new(999_999_999)),
            },
            &[Coin::new(100_000u128, "ujuno")],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Slippage"));
}

#[test]
fn test_simulate_swap() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("alice_init"), vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let alice = Addr::unchecked("alice_init");
    let pair = store_and_instantiate(&mut app, &factory);

    provide(&mut app, &pair, &alice, 1_000_000, 5_000_000);

    let res: SimulateResponse = q(
        &app,
        &pair,
        &QueryMsg::SimulateSwap {
            offer_asset: AssetInfo::Native("ujuno".to_string()),
            offer_amount: Uint128::new(100_000),
        },
    );
    assert!(!res.return_amount.is_zero());
    assert!(!res.fee_amount.is_zero());
}

#[test]
fn test_swap_empty_pool() {
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("bob_init"), vec![
                Coin::new(100_000u128, "ujuno"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let bob = Addr::unchecked("bob_init");
    let pair = store_and_instantiate(&mut app, &factory);

    let err = app
        .execute_contract(
            bob.clone(),
            pair.clone(),
            &ExecuteMsg::Swap {
                offer_asset: AssetInfo::Native("ujuno".to_string()),
                min_return: None,
            },
            &[Coin::new(100_000u128, "ujuno")],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("empty"));
}

#[test]
fn test_withdraw_liquidity() {
    let alice = MockApi::default().addr_make("alice");
    let alice_clone = alice.clone();
    let mut app = App::new(move |router, _, storage| {
        router
            .bank
            .init_balance(storage, &alice_clone, vec![
                Coin::new(10_000_000u128, "ujuno"),
                Coin::new(50_000_000u128, "uusdc"),
            ])
            .unwrap();
    });
    let factory = mk(&app, "factory");
    let pair = store_and_instantiate(&mut app, &factory);

    provide(&mut app, &pair, &alice, 1_000_000, 5_000_000);

    let lp: Uint128 = q(
        &app,
        &pair,
        &QueryMsg::LpBalance {
            address: alice.to_string(),
        },
    );
    let half = lp / Uint128::new(2);

    app.execute_contract(
        alice.clone(),
        pair.clone(),
        &ExecuteMsg::WithdrawLiquidity { lp_amount: half },
        &[],
    )
    .unwrap();

    let pool: PoolStateResponse = q(&app, &pair, &QueryMsg::Pool {});
    assert!(!pool.reserve_a.is_zero());
    assert!(!pool.reserve_b.is_zero());

    let lp_after: Uint128 = q(
        &app,
        &pair,
        &QueryMsg::LpBalance {
            address: alice.to_string(),
        },
    );
    assert_eq!(lp_after, lp - half);
}
