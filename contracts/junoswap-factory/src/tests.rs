use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
use cosmwasm_std::{Addr, Binary, Event, Reply, SubMsgResponse, SubMsgResult};

use crate::contract::{execute, instantiate, query, reply};
use crate::error::ContractError;
use crate::msg::*;
use junoclaw_common::AssetInfo;

/// Build a successful instantiate reply carrying the standard wasmd
/// `instantiate` event with the spawned `_contract_address`.
#[allow(deprecated)]
fn make_instantiate_reply(id: u64, contract_addr: &str) -> Reply {
    Reply {
        id,
        payload: Binary::default(),
        gas_used: 0,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![Event::new("instantiate")
                .add_attribute("_contract_address", contract_addr)
                .add_attribute("code_id", "1")],
            data: None,
            msg_responses: vec![],
        }),
    }
}

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

#[test]
fn test_create_pair_uses_default_fee() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("ujuno".to_string()),
        token_b: AssetInfo::Native("uusdc".to_string()),
        fee_bps: None, // should use default (30)
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let event = res.events.iter().find(|e| e.ty == "wasm-create_pair").unwrap();
    assert_eq!(
        event.attributes.iter().find(|a| a.key == "fee_bps").unwrap().value,
        "30"
    );
}

#[test]
fn test_create_pair_excessive_fee() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("ujuno".to_string()),
        token_b: AssetInfo::Native("uusdc".to_string()),
        fee_bps: Some(15000),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(err.to_string().contains("Invalid fee"));
}

#[test]
fn test_create_pair_emits_submsg() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("ujuno".to_string()),
        token_b: AssetInfo::Native("uusdc".to_string()),
        fee_bps: Some(25),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Should contain a WasmMsg::Instantiate submessage for the pair contract
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        cosmwasm_std::CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Instantiate { code_id, label, .. }) => {
            assert_eq!(*code_id, 1); // pair_code_id from setup
            assert!(label.contains("junoswap-pair"));
        }
        _ => panic!("expected WasmMsg::Instantiate"),
    }
}

#[test]
fn test_create_pair_sorts_assets() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    // Create with reversed order — event should show sorted keys
    let info = message_info(&Addr::unchecked("user1"), &[]);
    let msg = ExecuteMsg::CreatePair {
        token_a: AssetInfo::Native("uusdc".to_string()),
        token_b: AssetInfo::Native("ujuno".to_string()),
        fee_bps: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let event = res.events.iter().find(|e| e.ty == "wasm-create_pair").unwrap();
    let token_a_val = &event.attributes.iter().find(|a| a.key == "token_a").unwrap().value;
    let token_b_val = &event.attributes.iter().find(|a| a.key == "token_b").unwrap().value;
    // Sorted: ujuno < uusdc
    assert_eq!(token_a_val, "ujuno");
    assert_eq!(token_b_val, "uusdc");
}

#[test]
fn test_pair_count_query() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    // Initially 0
    let count: u64 =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::PairCount {}).unwrap())
            .unwrap();
    assert_eq!(count, 0);

    // Create a pair → count becomes 1
    let info = message_info(&Addr::unchecked("user1"), &[]);
    execute(
        deps.as_mut(),
        mock_env(),
        info.clone(),
        ExecuteMsg::CreatePair {
            token_a: AssetInfo::Native("ujuno".to_string()),
            token_b: AssetInfo::Native("uusdc".to_string()),
            fee_bps: None,
        },
    )
    .unwrap();

    let count: u64 =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::PairCount {}).unwrap())
            .unwrap();
    assert_eq!(count, 1);

    // Create another → count becomes 2
    execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::CreatePair {
            token_a: AssetInfo::Native("ujuno".to_string()),
            token_b: AssetInfo::Native("uatom".to_string()),
            fee_bps: Some(50),
        },
    )
    .unwrap();

    let count: u64 =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::PairCount {}).unwrap())
            .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_update_config_invalid_fee() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let info = message_info(&Addr::unchecked("owner"), &[]);
    let msg = ExecuteMsg::UpdateConfig {
        pair_code_id: None,
        default_fee_bps: Some(20000),
        junoclaw_contract: None,
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(err.to_string().contains("Invalid fee"));
}

#[test]
fn test_fee_boundary_10000_valid() {
    let mut deps = mock_dependencies();
    let info = message_info(&Addr::unchecked("owner"), &[]);
    let msg = InstantiateMsg {
        pair_code_id: 1,
        default_fee_bps: 10000,
        junoclaw_contract: None,
    };
    // 10000 bps = 100% — valid boundary
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res: ConfigResponse =
        cosmwasm_std::from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap();
    assert_eq!(res.default_fee_bps, 10000);
}

#[test]
fn test_create_pair_registers_pair_via_reply() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let token_a = AssetInfo::Native("ujuno".to_string());
    let token_b = AssetInfo::Native("uusdc".to_string());

    // Create the pair → id 1, PENDING_PAIRS[1] stashed.
    let info = message_info(&Addr::unchecked("user1"), &[]);
    execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::CreatePair {
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            fee_bps: Some(30),
        },
    )
    .unwrap();

    // Before the reply, the pair is not yet queryable.
    assert!(query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Pair { token_a: token_a.clone(), token_b: token_b.clone() },
    )
    .is_err());

    // Simulate the child instantiate reply.
    let pair_addr = deps.api.addr_make("pair1");
    let reply_msg = make_instantiate_reply(1, pair_addr.as_str());
    reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    // Now the pair resolves via both Pair and AllPairs.
    let res: PairResponse = cosmwasm_std::from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Pair { token_a: token_a.clone(), token_b: token_b.clone() },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(res.pair_addr, pair_addr);

    let all: PairsResponse = cosmwasm_std::from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::AllPairs { start_after: None, limit: None },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(all.pairs.len(), 1);
    assert_eq!(all.pairs[0].pair_addr, pair_addr);
}

#[test]
fn test_duplicate_pair_rejected_after_registration() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let token_a = AssetInfo::Native("ujuno".to_string());
    let token_b = AssetInfo::Native("uusdc".to_string());
    let info = message_info(&Addr::unchecked("user1"), &[]);

    // First create + reply registers the pair.
    execute(
        deps.as_mut(),
        mock_env(),
        info.clone(),
        ExecuteMsg::CreatePair {
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            fee_bps: None,
        },
    )
    .unwrap();
    let pair_addr = deps.api.addr_make("pair1");
    reply(deps.as_mut(), mock_env(), make_instantiate_reply(1, pair_addr.as_str())).unwrap();

    // Second create with the same assets (even reversed) now fails.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::CreatePair {
            token_a: token_b,
            token_b: token_a,
            fee_bps: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::PairExists {}));
}

#[test]
fn test_reply_unknown_id_rejected() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);

    let pair_addr = deps.api.addr_make("ghost");
    let err = reply(deps.as_mut(), mock_env(), make_instantiate_reply(999, pair_addr.as_str()))
        .unwrap_err();
    assert!(matches!(err, ContractError::UnknownReplyId { id: 999 }));
}

#[test]
fn test_migrate_ok() {
    let mut deps = mock_dependencies();
    setup_factory(&mut deps);
    let res = crate::contract::migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    assert_eq!(
        res.attributes.iter().find(|a| a.key == "action").unwrap().value,
        "migrate"
    );
}
