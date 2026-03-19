use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
use cosmwasm_std::Addr;

use crate::contract::{execute, instantiate, query};
use crate::msg::*;
use junoclaw_common::AssetInfo;

fn setup_factory(deps: &mut cosmwasm_std::OwnedDeps<cosmwasm_std::MemoryStorage, cosmwasm_std::testing::MockApi, cosmwasm_std::testing::MockQuerier>) {
    let info = message_info(&Addr::unchecked("owner"), &[]);
    let msg = InstantiateMsg {
        pair_code_id: 1,
        default_fee_bps: 30,
        junoclaw_contract: None,
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let res: ConfigResponse =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap();
    assert_eq!(res.owner, Addr::unchecked("owner"));
    assert_eq!(res.pair_code_id, 1);
    assert_eq!(res.default_fee_bps, 30);
    assert_eq!(res.pair_count, 0);
}

#[test]
fn test_invalid_fee() {
    let mut deps = mock_dependencies();
    let info = message_info(&Addr::unchecked("owner"), &[]);
    let msg = InstantiateMsg {
        pair_code_id: 1,
        default_fee_bps: 20000,
        junoclaw_contract: None,
    };
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(err.to_string().contains("Invalid fee"));
}

#[test]
fn test_create_pair_identical_assets() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("ujuno".to_string()),
        token_b: AssetInfo::Native("ujuno".to_string()),
        fee_bps: None,
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(err.to_string().contains("identical"));
}

#[test]
fn test_create_pair_emits_event() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("ujuno".to_string()),
        token_b: AssetInfo::Native("uusdc".to_string()),
        fee_bps: Some(50),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let event = res.events.iter().find(|e| e.ty == "wasm-create_pair").unwrap();
    assert_eq!(
        event.attributes.iter().find(|a| a.key == "pair_id").unwrap().value,
        "1"
    );
    assert_eq!(
        event.attributes.iter().find(|a| a.key == "fee_bps").unwrap().value,
        "50"
    );
}

#[test]
fn test_update_config_unauthorized() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("rando"), &[]);
    let msg = ExecuteMsg::UpdateConfig {
        pair_code_id: Some(2),
        default_fee_bps: None,
        junoclaw_contract: None,
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(err.to_string().contains("Unauthorized"));
}

#[test]
fn test_update_config_success() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("owner"), &[]);
    let msg = ExecuteMsg::UpdateConfig {
        pair_code_id: Some(5),
        default_fee_bps: Some(100),
        junoclaw_contract: None,
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res: ConfigResponse =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap();
    assert_eq!(res.pair_code_id, 5);
    assert_eq!(res.default_fee_bps, 100);
}
