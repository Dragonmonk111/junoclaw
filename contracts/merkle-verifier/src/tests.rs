use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};
use sha2::{Digest, Sha256};

use crate::msg::{
    AdminResponse, ExecuteMsg, GetRootResponse, InstantiateMsg, QueryMsg,
};

fn setup_contract(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    let code_id = app.store_code(Box::new(code));

    let msg = InstantiateMsg {
        admin: admin.to_string(),
    };
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &msg,
        &[],
        "merkle-verifier",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Build a Merkle tree from leaf hashes and return (root, proofs).
/// proofs[i] is the Merkle proof for leaf i.
/// Uses the same bit-based left/right scheme as the contract:
/// at each level, bit (index >> level) & 1 determines order.
/// Tree is padded to power-of-2 by duplicating the last leaf.
fn build_merkle_tree(leaves: &[String]) -> (String, Vec<Vec<String>>) {
    let n = leaves.len();
    if n == 0 {
        return (String::new(), Vec::new());
    }
    if n == 1 {
        return (leaves[0].clone(), vec![vec![]]);
    }

    // Pad to power of 2 by duplicating last leaf
    let mut padded = leaves.to_vec();
    let mut next_pow2 = 1;
    while next_pow2 < n {
        next_pow2 *= 2;
    }
    while padded.len() < next_pow2 {
        let last = padded.last().unwrap().clone();
        padded.push(last);
    }

    let total = padded.len();
    let mut proofs = vec![Vec::new(); n];

    // Build level by level
    let mut current: Vec<String> = padded;
    let mut level_size = total;

    while level_size > 1 {
        // For each original leaf, find its sibling at this level
        for j in 0..n {
            let level = proofs[j].len();
            let idx_at_level = j >> level;
            let sibling_idx = idx_at_level ^ 1;
            proofs[j].push(current[sibling_idx].clone());
        }

        // Compute next level
        let mut next: Vec<String> = Vec::new();
        for i in (0..level_size).step_by(2) {
            let mut h = Sha256::new();
            h.update(hex::decode(&current[i]).unwrap());
            h.update(hex::decode(&current[i + 1]).unwrap());
            next.push(hex::encode(h.finalize()));
        }
        current = next;
        level_size = current.len();
    }

    (current[0].clone(), proofs)
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let resp: AdminResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAdmin {})
        .unwrap();
    assert_eq!(resp.admin, admin.to_string());
}

#[test]
fn test_anchor_and_query_root() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let leaves: Vec<String> = (0..4)
        .map(|i| sha256_hex(format!("cycle-{}", i).as_bytes()))
        .collect();
    let (root, _) = build_merkle_tree(&leaves);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AnchorRoot {
            robot_id: "robot-1".to_string(),
            batch_height: 100,
            merkle_root: root.clone(),
            cycle_count: 4,
        },
        &[],
    )
    .unwrap();

    let resp: GetRootResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetRoot {
                robot_id: "robot-1".to_string(),
                batch_height: 100,
            },
        )
        .unwrap();

    assert_eq!(resp.merkle_root, root);
    assert_eq!(resp.cycle_count, 4);
}

#[test]
fn test_verify_valid_proof() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let leaves: Vec<String> = (0..8)
        .map(|i| sha256_hex(format!("cycle-{}", i).as_bytes()))
        .collect();
    let (root, proofs) = build_merkle_tree(&leaves);

    // Anchor the root
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AnchorRoot {
            robot_id: "robot-1".to_string(),
            batch_height: 200,
            merkle_root: root.clone(),
            cycle_count: 8,
        },
        &[],
    )
    .unwrap();

    // Verify proof for leaf 3
    let resp = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::VerifyProof {
            robot_id: "robot-1".to_string(),
            batch_height: 200,
            leaf_hash: leaves[3].clone(),
            leaf_index: 3,
            proof: proofs[3].clone(),
        },
        &[],
    );
    assert!(resp.is_ok());
    let resp = resp.unwrap();
    assert!(resp.events.iter().any(|e| {
        e.attributes.iter().any(|a| a.key == "result" && a.value == "valid")
    }));
}

#[test]
fn test_verify_invalid_proof_wrong_leaf() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let leaves: Vec<String> = (0..4)
        .map(|i| sha256_hex(format!("cycle-{}", i).as_bytes()))
        .collect();
    let (root, proofs) = build_merkle_tree(&leaves);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AnchorRoot {
            robot_id: "robot-1".to_string(),
            batch_height: 300,
            merkle_root: root,
            cycle_count: 4,
        },
        &[],
    )
    .unwrap();

    // Use wrong leaf hash with proof for leaf 0
    let wrong_leaf = sha256_hex(b"wrong-data");
    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                robot_id: "robot-1".to_string(),
                batch_height: 300,
                leaf_hash: wrong_leaf,
                leaf_index: 0,
                proof: proofs[0].clone(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("LeafHashMismatch") || err_str.contains("mismatch") || err_str.contains("invalid leaf hash"),
        "expected mismatch error, got: {}",
        err_str
    );
}

#[test]
fn test_verify_proof_root_not_found() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                robot_id: "robot-1".to_string(),
                batch_height: 999,
                leaf_hash: sha256_hex(b"test"),
                leaf_index: 0,
                proof: vec![],
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("RootNotFound") || err_str.contains("not found"),
        "expected root not found, got: {}",
        err_str
    );
}

#[test]
fn test_unauthorized_anchor() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let attacker = app.api().addr_make("attacker");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            attacker,
            contract.clone(),
            &ExecuteMsg::AnchorRoot {
                robot_id: "robot-1".to_string(),
                batch_height: 100,
                merkle_root: sha256_hex(b"fake"),
                cycle_count: 1,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Unauthorized") || err_str.contains("unauthorized"));
}

#[test]
fn test_empty_root_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let err = app
        .execute_contract(
            admin,
            contract.clone(),
            &ExecuteMsg::AnchorRoot {
                robot_id: "robot-1".to_string(),
                batch_height: 100,
                merkle_root: "".to_string(),
                cycle_count: 1,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("InvalidParams") || err_str.contains("empty"));
}

#[test]
fn test_single_leaf_tree() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = setup_contract(&mut app, &admin);

    let leaf = sha256_hex(b"single-cycle");
    let root = leaf.clone(); // Single leaf: root = leaf

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::AnchorRoot {
            robot_id: "robot-1".to_string(),
            batch_height: 400,
            merkle_root: root,
            cycle_count: 1,
        },
        &[],
    )
    .unwrap();

    // Verify with empty proof
    let resp = app.execute_contract(
        admin,
        contract,
        &ExecuteMsg::VerifyProof {
            robot_id: "robot-1".to_string(),
            batch_height: 400,
            leaf_hash: leaf,
            leaf_index: 0,
            proof: vec![],
        },
        &[],
    );
    assert!(resp.is_ok());
}

#[test]
fn test_transfer_admin() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let new_admin = app.api().addr_make("new_gov");
    let contract = setup_contract(&mut app, &admin);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TransferAdmin {
            new_admin: new_admin.to_string(),
        },
        &[],
    )
    .unwrap();

    let resp: AdminResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetAdmin {})
        .unwrap();
    assert_eq!(resp.admin, new_admin.to_string());
}
