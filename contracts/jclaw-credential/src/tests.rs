use cosmwasm_std::{Addr, Empty};
use cw_multi_test::{App, ContractWrapper, Executor, IntoAddr};

use crate::contract::{execute, instantiate, migrate, query};
use crate::msg::{
    ExecuteMsg, InstantiateMsg, ListMembersResponse, MayoPkHashResponse, MemberResponse, MigrateMsg,
    MlDsaPkHashResponse, MlDsaVariant, QueryMsg, SunsetStatusResponse, TotalWeightResponse,
};
use crate::state::MemberRole;

fn contract() -> impl cw_multi_test::Contract<Empty> {
    ContractWrapper::new(execute, instantiate, query).with_migrate(migrate)
}

fn setup_app() -> (App, Addr, Addr, Addr) {
    let mut app = App::default();
    let code_id = app.store_code(Box::new(contract()));
    let owner = "owner".into_addr();
    let genesis = "genesis".into_addr();

    let contract = app
        .instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                admin: Some(owner.to_string()),
                genesis: Some(genesis.to_string()),
                sunset_grace_seconds: 10,
            },
            &[],
            "jclaw-credential",
            None,
        )
        .unwrap();

    (app, contract, owner, genesis)
}

// ═════════════════════════════════════════════════════════════════════════════
// Instantiation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn instantiate_creates_genesis_with_full_weight() {
    let (app, contract, _owner, genesis) = setup_app();

    let resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();

    assert_eq!(resp.addr, genesis.to_string());
    assert_eq!(resp.weight, 10_000);
    assert_eq!(resp.role, MemberRole::Genesis);
    assert_eq!(resp.parent, None);
    assert_eq!(resp.depth, 0);
}

#[test]
fn instantiate_sets_total_weight() {
    let (app, contract, _owner, genesis) = setup_app();

    let resp: TotalWeightResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::TotalWeight {})
        .unwrap();

    assert_eq!(resp.weight, 10_000);
}

// ═════════════════════════════════════════════════════════════════════════════
// Bud
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn bud_splits_weight_from_parent() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let genesis_resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();
    assert_eq!(genesis_resp.weight, 7_000);

    let bud1: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: "bud1".into_addr().to_string() })
        .unwrap();
    assert_eq!(bud1.weight, 3_000);
    assert_eq!(bud1.role, MemberRole::Bud);
    assert_eq!(bud1.parent, Some(genesis.to_string()));
    assert_eq!(bud1.depth, 1);
}

#[test]
fn bud_chain_creates_depth() {
    let (mut app, contract, owner, genesis) = setup_app();

    // Genesis -> Bud1 -> Bud2
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 5_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: "bud1".into_addr().to_string(),
            child: "bud2".into_addr().to_string(),
            child_weight: 2_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let bud2: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: "bud2".into_addr().to_string() })
        .unwrap();
    assert_eq!(bud2.depth, 2);
    assert_eq!(bud2.parent, Some("bud1".into_addr().to_string()));
}

#[test]
fn bud_rejects_duplicate() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Duplicate member"));
}

#[test]
fn bud_rejects_insufficient_parent_weight() {
    let (mut app, contract, owner, genesis) = setup_app();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 20_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Invalid weights"));
}

#[test]
fn bud_rejects_non_admin() {
    let (mut app, contract, _owner, genesis) = setup_app();
    let rando = "rando".into_addr();

    let err = app
        .execute_contract(
            rando,
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}

// ═════════════════════════════════════════════════════════════════════════════
// BreakChannel
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn break_channel_prunes_subtree_and_returns_weight_to_genesis() {
    let (mut app, contract, owner, genesis) = setup_app();

    // Genesis (10k) -> Bud1 (3k) -> Bud2 (1k)
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: "bud1".into_addr().to_string(),
            child: "bud2".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    // Break bud1 (should also remove bud2)
    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::BreakChannel {
            addr: "bud1".into_addr().to_string(),
        },
        &[],
    )
    .unwrap();

    // bud1 and bud2 gone
    let err = app
        .wrap()
        .query_wasm_smart::<MemberResponse>(&contract, &QueryMsg::Member { addr: "bud1".into_addr().to_string() });
    assert!(err.is_err());

    let err = app
        .wrap()
        .query_wasm_smart::<MemberResponse>(&contract, &QueryMsg::Member { addr: "bud2".into_addr().to_string() });
    assert!(err.is_err());

    // Genesis regained 3k + 1k = 4k -> back to 10k
    let genesis_resp: MemberResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::Member { addr: genesis.to_string() })
        .unwrap();
    assert_eq!(genesis_resp.weight, 10_000);
}

#[test]
fn break_channel_rejects_root() {
    let (mut app, contract, owner, genesis) = setup_app();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::BreakChannel {
                addr: genesis.to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Cannot break channel on root member"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Sunset
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn sunset_blocked_if_members_have_children() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::InitiateSunset {},
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("has not passed their bud"));
}

#[test]
fn sunset_success_when_all_leaves() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::InitiateSunset {},
        &[],
    )
    .unwrap();

    let status: SunsetStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::SunsetStatus {})
        .unwrap();
    assert!(status.initiated);
    assert!(!status.executed);
}

#[test]
fn execute_sunset_after_grace_period() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::InitiateSunset {},
        &[],
    )
    .unwrap();

    // Too early
    let err = app
        .execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::ExecuteSunset {},
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Sunset grace period"));

    // Advance blocks past grace period (10 blocks)
    app.update_block(|b| b.height += 20);

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::ExecuteSunset {},
        &[],
    )
    .unwrap();

    let status: SunsetStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::SunsetStatus {})
        .unwrap();
    assert!(status.executed);
}

// ═════════════════════════════════════════════════════════════════════════════
// ═════════════════════════════════════════════════════════════════════════════
// MAYO post-quantum attestation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn mayo_bud_stores_pk_hash() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    let resp: crate::msg::MayoPkHashResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::MayoPkHash {
                addr: "bud_mayo".into_addr().to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        resp.mayo_pk_hash,
        Some(crate::mayo_vectors::PK_HASH.to_string())
    );
}

#[test]
fn mayo_verify_valid_signature() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        "bud_mayo".into_addr(),
        contract,
        &ExecuteMsg::VerifyMayoAttestation {
            addr: "bud_mayo".into_addr().to_string(),
            message: crate::mayo_vectors::MSG.to_vec(),
            signature: sig,
            public_key: pk,
            variant: crate::msg::MayoVariant::Mayo2,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mayo_verify_invalid_signature() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            "bud_mayo".into_addr(),
            contract,
            &ExecuteMsg::VerifyMayoAttestation {
                addr: "bud_mayo".into_addr().to_string(),
                message: b"tampered message".to_vec(),
                signature: sig,
                public_key: pk,
                variant: crate::msg::MayoVariant::Mayo2,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("MAYO signature verification failed"));
}

#[test]
fn mayo_verify_hash_mismatch() {
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    // Mutate the last byte of the public key so the hash no longer matches
    let mut bad_pk = pk.clone();
    let last = bad_pk.len() - 1;
    bad_pk[last] = bad_pk[last].wrapping_add(1);

    let err = app
        .execute_contract(
            "bud_mayo".into_addr(),
            contract,
            &ExecuteMsg::VerifyMayoAttestation {
                addr: "bud_mayo".into_addr().to_string(),
                message: crate::mayo_vectors::MSG.to_vec(),
                signature: sig,
                public_key: bad_pk,
                variant: crate::msg::MayoVariant::Mayo2,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("MAYO public key hash mismatch"));
}

/// Generic multi-variant Bud + Verify round-trip. Also validates the vector
/// integrity: the SHA-256 of the decoded PK must match the recorded hash.
fn mayo_variant_roundtrip(
    pk_hex: &str,
    sig_hex: &str,
    pk_hash: &str,
    variant: crate::msg::MayoVariant,
) {
    use sha2::{Digest, Sha256};

    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(pk_hex).unwrap();
    let sig = hex::decode(sig_hex).unwrap();
    assert_eq!(hex::encode(Sha256::digest(&pk)), pk_hash, "vector PK hash mismatch");

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        "bud_mayo".into_addr(),
        contract,
        &ExecuteMsg::VerifyMayoAttestation {
            addr: "bud_mayo".into_addr().to_string(),
            message: crate::mayo_vectors::MSG.to_vec(),
            signature: sig,
            public_key: pk,
            variant,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mayo3_verify_valid_signature() {
    mayo_variant_roundtrip(
        crate::mayo_vectors::MAYO3_PK_HEX,
        crate::mayo_vectors::MAYO3_SIG_HEX,
        crate::mayo_vectors::MAYO3_PK_HASH,
        crate::msg::MayoVariant::Mayo3,
    );
}

#[test]
fn mayo5_verify_valid_signature() {
    mayo_variant_roundtrip(
        crate::mayo_vectors::MAYO5_PK_HEX,
        crate::mayo_vectors::MAYO5_SIG_HEX,
        crate::mayo_vectors::MAYO5_PK_HASH,
        crate::msg::MayoVariant::Mayo5,
    );
}

#[test]
fn mayo_verify_wrong_variant_rejected() {
    // MAYO-3 PK/sig presented as MAYO-5 must fail (length mismatch → verify error).
    let (mut app, contract, owner, genesis) = setup_app();
    let pk = hex::decode(crate::mayo_vectors::MAYO3_PK_HEX).unwrap();
    let sig = hex::decode(crate::mayo_vectors::MAYO3_SIG_HEX).unwrap();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mayo".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: Some(pk.clone()),
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            "bud_mayo".into_addr(),
            contract,
            &ExecuteMsg::VerifyMayoAttestation {
                addr: "bud_mayo".into_addr().to_string(),
                message: crate::mayo_vectors::MSG.to_vec(),
                signature: sig,
                public_key: pk,
                variant: crate::msg::MayoVariant::Mayo5,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("MAYO signature verification failed"));
}

// ═════════════════════════════════════════════════════════════════════════════
// ListMembers & TotalWeight (cw4-compatible)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn list_members_paginates() {
    let (mut app, contract, owner, genesis) = setup_app();

    for i in 1..=5 {
        let label = format!("bud{}", i);
        app.execute_contract(
            owner.clone(),
            contract.clone(),
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: label.as_str().into_addr().to_string(),
                child_weight: 100,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap();
    }

    let resp: ListMembersResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListMembers { start_after: None, limit: Some(3) })
        .unwrap();
    assert_eq!(resp.members.len(), 3);
}

#[test]
fn total_weight_unchanged_after_bud() {
    let (mut app, contract, owner, genesis) = setup_app();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud1".into_addr().to_string(),
            child_weight: 3_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let resp: TotalWeightResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::TotalWeight {})
        .unwrap();
    assert_eq!(resp.weight, 10_000);
}

// ═════════════════════════════════════════════════════════════════════════════
// Admin transfer
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn transfer_admin_works() {
    let (mut app, contract, owner, genesis) = setup_app();
    let new_admin = "new_admin".into_addr();

    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::TransferAdmin {
            new_admin: new_admin.to_string(),
        },
        &[],
    )
    .unwrap();

    // Old owner can no longer bud
    let err = app
        .execute_contract(
            "owner".into_addr(),
            contract,
            &ExecuteMsg::Bud {
                parent: genesis.to_string(),
                child: "bud1".into_addr().to_string(),
                child_weight: 1_000,
                mayo_pk: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}

// ═════════════════════════════════════════════════════════════════════════════
// ML-DSA (FIPS 204) attestation — exercises the in-contract `fips204` verifier
// (default build). These tests fail under `--features mldsa-precompile`, which
// routes through the host function only available on a wasmvm-fork chain.
// ═════════════════════════════════════════════════════════════════════════════

/// Deterministic ML-DSA-44 (pk, sig, msg) vector via seeded keygen + sign.
fn mldsa44_vector() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    use fips204::ml_dsa_44 as m;
    use fips204::traits::{KeyGen, SerDes, Signer};
    let (pk, sk) = m::KG::keygen_from_seed(&[42u8; 32]);
    let msg = b"aegis :: ml-dsa-44 attestation".to_vec();
    let sig = sk.try_sign_with_seed(&[7u8; 32], &msg, &[]).unwrap();
    (pk.into_bytes().to_vec(), sig.to_vec(), msg)
}

/// Deterministic ML-DSA-65 (pk, sig, msg) vector via seeded keygen + sign.
fn mldsa65_vector() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    use fips204::ml_dsa_65 as m;
    use fips204::traits::{KeyGen, SerDes, Signer};
    let (pk, sk) = m::KG::keygen_from_seed(&[24u8; 32]);
    let msg = b"aegis :: ml-dsa-65 attestation".to_vec();
    let sig = sk.try_sign_with_seed(&[11u8; 32], &msg, &[]).unwrap();
    (pk.into_bytes().to_vec(), sig.to_vec(), msg)
}

/// Bud a member and register its ML-DSA public key (admin-driven).
fn bud_and_set_mldsa(app: &mut App, contract: &Addr, owner: &Addr, genesis: &Addr, child: &str, pk: &[u8]) {
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: child.into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::SetMlDsaPk {
            addr: child.into_addr().to_string(),
            mldsa_pk: pk.to_vec(),
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mldsa_pk_lens_match_fips204() {
    use crate::msg::MLDSA_PK_LENS;
    assert_eq!(MLDSA_PK_LENS[0], fips204::ml_dsa_44::PK_LEN);
    assert_eq!(MLDSA_PK_LENS[1], fips204::ml_dsa_65::PK_LEN);
    assert_eq!(MLDSA_PK_LENS[2], fips204::ml_dsa_87::PK_LEN);
}

#[test]
fn mldsa_set_pk_stores_hash() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, _sig, _msg) = mldsa44_vector();
    bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);

    use sha2::{Digest, Sha256};
    let expected = hex::encode(Sha256::digest(&pk));
    let resp: MlDsaPkHashResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::MlDsaPkHash {
                addr: "bud_mldsa".into_addr().to_string(),
            },
        )
        .unwrap();
    assert_eq!(resp.mldsa_pk_hash, Some(expected));
}

#[test]
fn mldsa_verify_valid_signature_44() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, sig, msg) = mldsa44_vector();
    bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);

    app.execute_contract(
        "bud_mldsa".into_addr(),
        contract,
        &ExecuteMsg::VerifyMlDsaAttestation {
            addr: "bud_mldsa".into_addr().to_string(),
            message: msg,
            signature: sig,
            public_key: pk,
            variant: MlDsaVariant::MlDsa44,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mldsa_verify_valid_signature_65() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, sig, msg) = mldsa65_vector();
    bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);

    app.execute_contract(
        "bud_mldsa".into_addr(),
        contract,
        &ExecuteMsg::VerifyMlDsaAttestation {
            addr: "bud_mldsa".into_addr().to_string(),
            message: msg,
            signature: sig,
            public_key: pk,
            variant: MlDsaVariant::MlDsa65,
        },
        &[],
    )
    .unwrap();
}

#[test]
fn mldsa_verify_invalid_signature() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, sig, _msg) = mldsa44_vector();
    bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);

    let err = app
        .execute_contract(
            "bud_mldsa".into_addr(),
            contract,
            &ExecuteMsg::VerifyMlDsaAttestation {
                addr: "bud_mldsa".into_addr().to_string(),
                message: b"tampered message".to_vec(),
                signature: sig,
                public_key: pk,
                variant: MlDsaVariant::MlDsa44,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("ML-DSA signature verification failed"));
}

#[test]
fn mldsa_verify_hash_mismatch() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, sig, msg) = mldsa44_vector();
    bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);

    // Same valid length, different bytes → hash no longer matches.
    let mut bad_pk = pk.clone();
    let last = bad_pk.len() - 1;
    bad_pk[last] = bad_pk[last].wrapping_add(1);

    let err = app
        .execute_contract(
            "bud_mldsa".into_addr(),
            contract,
            &ExecuteMsg::VerifyMlDsaAttestation {
                addr: "bud_mldsa".into_addr().to_string(),
                message: msg,
                signature: sig,
                public_key: bad_pk,
                variant: MlDsaVariant::MlDsa44,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("ML-DSA public key hash mismatch"));
}

#[test]
fn mldsa_verify_pk_not_registered() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, sig, msg) = mldsa44_vector();

    // Bud the member but never call SetMlDsaPk.
    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mldsa".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            "bud_mldsa".into_addr(),
            contract,
            &ExecuteMsg::VerifyMlDsaAttestation {
                addr: "bud_mldsa".into_addr().to_string(),
                message: msg,
                signature: sig,
                public_key: pk,
                variant: MlDsaVariant::MlDsa44,
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("ML-DSA public key not registered"));
}

#[test]
fn mldsa_set_pk_invalid_length() {
    let (mut app, contract, owner, genesis) = setup_app();
    app.execute_contract(
        owner.clone(),
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mldsa".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner,
            contract,
            &ExecuteMsg::SetMlDsaPk {
                addr: "bud_mldsa".into_addr().to_string(),
                mldsa_pk: vec![0u8; 100],
            },
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("Invalid ML-DSA public key length"));
}

#[test]
fn mldsa_static_vectors_verify() {
    use crate::mldsa_vectors as v;
    let cases = [
        (MlDsaVariant::MlDsa44, v::MLDSA44_PK_HEX, v::MLDSA44_SIG_HEX),
        (MlDsaVariant::MlDsa65, v::MLDSA65_PK_HEX, v::MLDSA65_SIG_HEX),
        (MlDsaVariant::MlDsa87, v::MLDSA87_PK_HEX, v::MLDSA87_SIG_HEX),
    ];
    for (variant, pk_hex, sig_hex) in cases {
        let (mut app, contract, owner, genesis) = setup_app();
        let pk = hex::decode(pk_hex).unwrap();
        let sig = hex::decode(sig_hex).unwrap();
        bud_and_set_mldsa(&mut app, &contract, &owner, &genesis, "bud_mldsa", &pk);
        app.execute_contract(
            "bud_mldsa".into_addr(),
            contract,
            &ExecuteMsg::VerifyMlDsaAttestation {
                addr: "bud_mldsa".into_addr().to_string(),
                message: v::MLDSA_MSG.to_vec(),
                signature: sig,
                public_key: pk,
                variant,
            },
            &[],
        )
        .unwrap();
    }
}

#[test]
fn mldsa_variant_json_names() {
    // The benchmark driver (deploy/benchmark-mldsa-devnet.cjs) sends these
    // exact strings; pin them so a serde rename never silently breaks it.
    let to = |v: &MlDsaVariant| String::from_utf8(cosmwasm_std::to_json_vec(v).unwrap()).unwrap();
    assert_eq!(to(&MlDsaVariant::MlDsa44), "\"ml_dsa44\"");
    assert_eq!(to(&MlDsaVariant::MlDsa65), "\"ml_dsa65\"");
    assert_eq!(to(&MlDsaVariant::MlDsa87), "\"ml_dsa87\"");
}

#[test]
fn mldsa_set_pk_unauthorized() {
    let (mut app, contract, owner, genesis) = setup_app();
    let (pk, _sig, _msg) = mldsa44_vector();
    app.execute_contract(
        owner,
        contract.clone(),
        &ExecuteMsg::Bud {
            parent: genesis.to_string(),
            child: "bud_mldsa".into_addr().to_string(),
            child_weight: 1_000,
            mayo_pk: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            "rando".into_addr(),
            contract,
            &ExecuteMsg::SetMlDsaPk {
                addr: "bud_mldsa".into_addr().to_string(),
                mldsa_pk: pk,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("Unauthorized"));
}
