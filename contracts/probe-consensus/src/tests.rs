use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

const ROOT_A: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ROOT_B: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn setup(app: &mut App, admin: &Addr, required: u32, ops: &[Addr]) -> Addr {
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));
    let msg = InstantiateMsg {
        admin: Some(admin.to_string()),
        required,
        operators: ops.iter().map(|a| a.to_string()).collect(),
    };
    app.instantiate_contract(code_id, admin.clone(), &msg, &[], "probe-consensus", None)
        .unwrap()
}

fn exec_commit(
    app: &mut App,
    sender: &Addr,
    contract: &Addr,
    root: &str,
    batch: u64,
) -> anyhow::Result<()> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Commit {
            robot_id: "r1".to_string(),
            batch_height: batch,
            merkle_root: root.to_string(),
            cycle_count: 1,
        },
        &[],
    )
    .map(|_| ())
}

fn err_text(err: &anyhow::Error) -> String {
    let mut s = format!("{err:?}");
    for cause in err.chain().skip(1) {
        s.push_str(&format!(" | {cause}"));
    }
    s
}

fn agreed(app: &App, contract: &Addr, batch: u64) -> bool {
    app.wrap()
        .query_wasm_smart::<serde_json::Value>(
            contract,
            &QueryMsg::GetAgreed {
                robot_id: "r1".to_string(),
                batch_height: batch,
            },
        )
        .is_ok()
}

#[test]
fn quorum_identical_roots_agrees() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone()]);

    exec_commit(&mut app, &op1, &contract, ROOT_A, 7).unwrap();
    assert!(!agreed(&app, &contract, 7), "1 of 2 commits — no agreement yet");

    exec_commit(&mut app, &op2, &contract, ROOT_A, 7).unwrap();
    assert!(agreed(&app, &contract, 7), "2 identical commits — agreed");
}

#[test]
fn mismatched_roots_never_agree() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone()]);

    exec_commit(&mut app, &op1, &contract, ROOT_A, 8).unwrap();
    exec_commit(&mut app, &op2, &contract, ROOT_B, 8).unwrap();

    assert!(!agreed(&app, &contract, 8));
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::Finalize {
                robot_id: "r1".to_string(),
                batch_height: 8,
            },
            &[],
        )
        .unwrap_err();
    let text = err_text(&err);
    assert!(text.contains("commits disagree"), "got: {text}");
}

#[test]
fn insufficient_operators_no_agreement() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    // required=3 but only 2 operators registered → instantiate must fail.
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));
    let res = app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: Some(admin.to_string()),
            required: 3,
            operators: vec![op1.to_string(), op2.to_string()],
        },
        &[],
        "probe-consensus",
        None,
    );
    assert!(res.is_err(), "operator set smaller than quorum must fail");
}

#[test]
fn duplicate_commit_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone()]);

    exec_commit(&mut app, &op1, &contract, ROOT_A, 9).unwrap();
    let err = exec_commit(&mut app, &op1, &contract, ROOT_A, 9).unwrap_err();
    let text = err_text(&err);
    assert!(text.contains("already committed"), "got: {text}");
}

#[test]
fn non_operator_cannot_commit() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let rando = app.api().addr_make("rando");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone()]);

    let err = exec_commit(&mut app, &rando, &contract, ROOT_A, 10).unwrap_err();
    let text = err_text(&err);
    assert!(text.contains("not a registered operator"), "got: {text}");
}

#[test]
fn late_commit_after_agreement_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let op3 = app.api().addr_make("op3");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone(), op3.clone()]);

    exec_commit(&mut app, &op1, &contract, ROOT_A, 11).unwrap();
    exec_commit(&mut app, &op2, &contract, ROOT_A, 11).unwrap();
    assert!(agreed(&app, &contract, 11));

    // op3 tries to commit a *different* root after agreement — must fail.
    let err = exec_commit(&mut app, &op3, &contract, ROOT_B, 11).unwrap_err();
    let text = err_text(&err);
    assert!(text.contains("already agreed"), "got: {text}");
}

#[test]
fn finalize_idempotent_and_lists_committers() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let op1 = app.api().addr_make("op1");
    let op2 = app.api().addr_make("op2");
    let contract = setup(&mut app, &admin, 2, &[op1.clone(), op2.clone()]);

    exec_commit(&mut app, &op1, &contract, ROOT_A, 12).unwrap();
    exec_commit(&mut app, &op2, &contract, ROOT_A, 12).unwrap();

    // Second finalize is a no-op success.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Finalize {
            robot_id: "r1".to_string(),
            batch_height: 12,
        },
        &[],
    )
    .unwrap();

    let resp: serde_json::Value = app
        .wrap()
        .query_wasm_smart(contract, &QueryMsg::GetAgreed {
            robot_id: "r1".to_string(),
            batch_height: 12,
        })
        .unwrap();
    assert_eq!(resp["merkle_root"], ROOT_A);
    assert_eq!(resp["commits"].as_array().unwrap().len(), 2);
}
