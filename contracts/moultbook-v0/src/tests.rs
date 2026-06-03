use cosmwasm_std::{Addr, Binary};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{EntriesResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{AttestationRef, Config, Disclosure, MoultEntry, Stats, Visibility};

const MAX_SIZE: u64 = 1_048_576;
const MAX_REFS: u32 = 8;
const MAX_CONTENT_TYPE: u32 = 64;
const MAX_GROUP_SIZE: u32 = 50;

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: admin.to_string(),
            whoami_contract: None,
            max_size_bytes: MAX_SIZE,
            max_refs: MAX_REFS,
            max_content_type_len: MAX_CONTENT_TYPE,
            max_group_size: MAX_GROUP_SIZE,
            zk_verifier: None,
            agent_registry: None,
            membership_vk_hash: None,
            entries_per_key_per_epoch: None,
            epoch_blocks: None,
        },
        &[],
        "moultbook-v0",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn make_commitment(byte: u8) -> Binary {
    Binary::from(vec![byte; 32])
}

fn post_entry(
    app: &mut App,
    contract: &Addr,
    sender: &Addr,
    commitment_byte: u8,
    refs: Vec<String>,
) -> Result<cw_multi_test::AppResponse, anyhow::Error> {
    app.execute_contract(
        sender.clone(),
        contract.clone(),
        &ExecuteMsg::Post {
            commitment: make_commitment(commitment_byte),
            content_type: "text/markdown".to_string(),
            size_bytes: 256,
            attestation_ref: None,
            visibility: Visibility::Public,
            refs,
        },
        &[],
    )
}

/// Pull the entry id out of the post response's "id" attribute.
fn extract_id(resp: &cw_multi_test::AppResponse) -> String {
    for ev in &resp.events {
        for attr in &ev.attributes {
            if attr.key == "id" {
                return attr.value.clone();
            }
        }
    }
    panic!("no id attribute in response");
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, admin);
    assert!(cfg.whoami_contract.is_none());
    assert_eq!(cfg.max_size_bytes, MAX_SIZE);

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.total_active, 0);
    assert_eq!(stats.total_redacted, 0);
}

#[test]
fn test_post_happy_path() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x42, vec![]).unwrap();
    let id = extract_id(&resp);
    assert!(id.starts_with("moult:"));
    assert_eq!(id.len(), "moult:".len() + 64);

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id: id.clone() })
        .unwrap();
    assert_eq!(entry.author, alice);
    assert_eq!(entry.commitment.len(), 32);
    assert_eq!(entry.content_type, "text/markdown");
    assert_eq!(entry.size_bytes, 256);
    assert!(entry.refs.is_empty());
    assert!(entry.redacted_at.is_none());

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.total_active, 1);
}

#[test]
fn test_post_invalid_commitment_length() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::Post {
                commitment: Binary::from(vec![0u8; 16]),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                attestation_ref: None,
                visibility: Visibility::Public,
                refs: vec![],
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(
        cerr,
        ContractError::InvalidCommitmentLength { got: 16 }
    ));
}

#[test]
fn test_post_size_too_large() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::Post {
                commitment: make_commitment(0x01),
                content_type: "text/plain".to_string(),
                size_bytes: MAX_SIZE + 1,
                attestation_ref: None,
                visibility: Visibility::Public,
                refs: vec![],
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::SizeTooLarge { .. }));
}

#[test]
fn test_post_with_invalid_ref() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = post_entry(
        &mut app,
        &contract,
        &alice,
        0x01,
        vec!["moult:does_not_exist".to_string()],
    )
    .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::InvalidRef { .. }));
}

#[test]
fn test_post_with_valid_refs_indexes_correctly() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    // Alice posts the foundational entry.
    let r1 = post_entry(&mut app, &contract, &alice, 0xAA, vec![]).unwrap();
    let id_a = extract_id(&r1);

    // Bob cites it.
    let r2 = post_entry(&mut app, &contract, &bob, 0xBB, vec![id_a.clone()]).unwrap();
    let id_b = extract_id(&r2);

    // ListByRef(id_a) should return Bob's entry.
    let resp: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByRef {
                ref_id: id_a.clone(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(resp.entries.len(), 1);
    assert_eq!(resp.entries[0].id, id_b);
    assert_eq!(resp.entries[0].author, bob);
    assert_eq!(resp.entries[0].refs, vec![id_a]);
}

#[test]
fn test_redact_by_author() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x33, vec![]).unwrap();
    let id = extract_id(&resp);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::Redact { id: id.clone() },
        &[],
    )
    .unwrap();

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id })
        .unwrap();
    assert_eq!(entry.commitment.len(), 0, "commitment should be cleared");
    assert!(entry.redacted_at.is_some());
    assert_eq!(entry.author, alice);

    let stats: Stats = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetStats {})
        .unwrap();
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.total_active, 0);
    assert_eq!(stats.total_redacted, 1);
}

#[test]
fn test_redact_by_admin() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x44, vec![]).unwrap();
    let id = extract_id(&resp);

    // Admin (not author) can redact.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::Redact { id: id.clone() },
        &[],
    )
    .unwrap();

    let entry: MoultEntry = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetEntry { id })
        .unwrap();
    assert!(entry.redacted_at.is_some());
}

#[test]
fn test_redact_by_stranger_unauthorized() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let mallory = app.api().addr_make("mallory");
    let contract = store_and_instantiate(&mut app, &admin);

    let resp = post_entry(&mut app, &contract, &alice, 0x55, vec![]).unwrap();
    let id = extract_id(&resp);

    let err = app
        .execute_contract(
            mallory,
            contract,
            &ExecuteMsg::Redact { id },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::Unauthorized {}));
}

#[test]
fn test_update_visibility_cannot_widen_to_public() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let contract = store_and_instantiate(&mut app, &admin);

    // Post as Owner-scope.
    let resp = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::Post {
                commitment: make_commitment(0x77),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                attestation_ref: None,
                visibility: Visibility::Owner,
                refs: vec![],
            },
            &[],
        )
        .unwrap();
    let id = extract_id(&resp);

    // Cannot widen to Public.
    let err = app
        .execute_contract(
            alice,
            contract,
            &ExecuteMsg::UpdateVisibility {
                id,
                visibility: Visibility::Public,
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::CannotWidenVisibility {}));
}

#[test]
fn test_list_by_author_returns_owned_entries() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let alice = app.api().addr_make("alice");
    let bob = app.api().addr_make("bob");
    let contract = store_and_instantiate(&mut app, &admin);

    post_entry(&mut app, &contract, &alice, 0xA1, vec![]).unwrap();
    post_entry(&mut app, &contract, &alice, 0xA2, vec![]).unwrap();
    post_entry(&mut app, &contract, &bob, 0xB1, vec![]).unwrap();

    let alice_entries: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByAuthor {
                author: alice.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(alice_entries.entries.len(), 2);
    for e in &alice_entries.entries {
        assert_eq!(e.author, alice);
    }

    let bob_entries: EntriesResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::ListByAuthor {
                author: bob.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(bob_entries.entries.len(), 1);
    assert_eq!(bob_entries.entries[0].author, bob);
}

#[test]
fn test_update_config_admin_only() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let mallory = app.api().addr_make("mallory");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            mallory,
            contract.clone(),
            &ExecuteMsg::UpdateConfig {
                admin: None,
                whoami_contract: None,
                max_size_bytes: Some(1),
                max_refs: None,
                max_group_size: None,
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::Unauthorized {}));

    // Admin can update.
    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::UpdateConfig {
            admin: None,
            whoami_contract: None,
            max_size_bytes: Some(2_000_000),
            max_refs: None,
            max_group_size: None,
        },
        &[],
    )
    .unwrap();

    let cfg: Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.max_size_bytes, 2_000_000);
}

#[test]
fn group_too_large_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let big_group: Vec<Addr> = (0..MAX_GROUP_SIZE + 1)
        .map(|i| app.api().addr_make(&format!("member{}", i)))
        .collect();

    let err = app
        .execute_contract(
            admin.clone(),
            contract,
            &ExecuteMsg::Post {
                commitment: make_commitment(0xAA),
                content_type: "text/plain".to_string(),
                size_bytes: 100,
                attestation_ref: None,
                visibility: Visibility::Group(big_group),
                refs: vec![],
            },
            &[],
        )
        .unwrap_err();
    let cerr: ContractError = err.downcast().unwrap();
    assert!(matches!(cerr, ContractError::GroupTooLarge { .. }));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration: PublishAnon → zk-verifier SubMsg → reply → entry persisted
// ─────────────────────────────────────────────────────────────────────────────

mod publish_anon_integration {
    use super::*;
    use zk_verifier::contract::{
        execute as zk_execute, instantiate as zk_instantiate, migrate as zk_migrate,
        query as zk_query,
    };
    use zk_verifier::msg::{ExecuteMsg as ZkExecuteMsg, InstantiateMsg as ZkInstantiateMsg};
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
    use ark_serialize::CanonicalSerialize;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};
    use sha2::{Digest, Sha256};

    /// Minimal circuit: proves knowledge of sqrt(y). Public input: y.
    #[derive(Clone)]
    struct SquareCircuit {
        x: Option<Fr>,
    }

    impl ConstraintSynthesizer<Fr> for SquareCircuit {
        fn generate_constraints(
            self,
            cs: ConstraintSystemRef<Fr>,
        ) -> Result<(), SynthesisError> {
            let x_val = self.x.unwrap_or(Fr::from(1u64));
            let y_val = x_val * x_val;
            let x_var = cs.new_witness_variable(|| Ok(x_val))?;
            let y_var = cs.new_input_variable(|| Ok(y_val))?;
            cs.enforce_constraint(
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + y_var,
            )?;
            Ok(())
        }
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const ALPHA: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;
            out.push(ALPHA[((triple >> 18) & 0x3F) as usize] as char);
            out.push(ALPHA[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 { out.push(ALPHA[((triple >> 6) & 0x3F) as usize] as char); } else { out.push('='); }
            if chunk.len() > 2 { out.push(ALPHA[(triple & 0x3F) as usize] as char); } else { out.push('='); }
        }
        out
    }

    fn hex_encode(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0x0f) as usize] as char);
        }
        s
    }

    /// Generate (vk_bytes, vk_b64, proof_b64, inputs_b64) for x=3, y=9.
    fn gen_proof() -> (Vec<u8>, String, String, String) {
        let mut rng = StdRng::seed_from_u64(101);
        let circuit = SquareCircuit { x: None };
        let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
            Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng).unwrap();
        let circuit = SquareCircuit { x: Some(Fr::from(3u64)) };
        let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

        let mut vk_bytes: Vec<u8> = Vec::new();
        CanonicalSerialize::serialize_compressed(&vk, &mut vk_bytes).unwrap();
        let mut proof_bytes: Vec<u8> = Vec::new();
        CanonicalSerialize::serialize_compressed(&proof, &mut proof_bytes).unwrap();
        let public_input = Fr::from(9u64);
        let mut input_bytes: Vec<u8> = Vec::new();
        CanonicalSerialize::serialize_compressed(&public_input, &mut input_bytes).unwrap();

        (
            vk_bytes.clone(),
            base64_encode(&vk_bytes),
            base64_encode(&proof_bytes),
            base64_encode(&input_bytes),
        )
    }

    /// Deploy zk-verifier, store a VK, return (zk_contract, vk_hash_string).
    fn deploy_zk_verifier_with_vk(
        app: &mut App,
        admin: &Addr,
        vk_bytes: &[u8],
        vk_b64: &str,
    ) -> (Addr, String) {
        let zk_code = ContractWrapper::new(zk_execute, zk_instantiate, zk_query)
            .with_migrate(zk_migrate);
        let zk_code_id = app.store_code(Box::new(zk_code));
        let zk_contract = app
            .instantiate_contract(
                zk_code_id,
                admin.clone(),
                &ZkInstantiateMsg { admin: Some(admin.to_string()) },
                &[],
                "zk-verifier",
                Some(admin.to_string()),
            )
            .unwrap();

        app.execute_contract(
            admin.clone(),
            zk_contract.clone(),
            &ZkExecuteMsg::StoreVk { vk_base64: vk_b64.to_string() },
            &[],
        )
        .unwrap();

        let mut hasher = Sha256::new();
        hasher.update(vk_bytes);
        let vk_hash = format!("sha256:{}", hex_encode(&hasher.finalize()));

        (zk_contract, vk_hash)
    }

    /// Deploy moultbook-v0 (with reply handler) connected to a zk-verifier.
    fn deploy_moultbook(
        app: &mut App,
        admin: &Addr,
        zk_contract: &Addr,
        vk_hash: &str,
    ) -> Addr {
        let mb_code = ContractWrapper::new(execute, instantiate, query)
            .with_migrate(migrate)
            .with_reply(crate::contract::reply);
        let mb_code_id = app.store_code(Box::new(mb_code));
        app.instantiate_contract(
            mb_code_id,
            admin.clone(),
            &InstantiateMsg {
                admin: admin.to_string(),
                whoami_contract: None,
                max_size_bytes: 1_048_576,
                max_refs: 8,
                max_content_type_len: 64,
                max_group_size: 50,
                zk_verifier: Some(zk_contract.to_string()),
                agent_registry: None,
                membership_vk_hash: Some(vk_hash.to_string()),
                entries_per_key_per_epoch: None,
                epoch_blocks: None,
            },
            &[],
            "moultbook-v0",
            Some(admin.to_string()),
        )
        .unwrap()
    }

    /// Happy path: valid Groth16 proof → entry created with ZkProof attestation ref.
    #[test]
    fn publish_anon_valid_proof_creates_entry_with_zk_attestation() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let moult_key = app.api().addr_make("moult_key_1");

        let (vk_bytes, vk_b64, proof_b64, inputs_b64) = gen_proof();
        let (zk_contract, vk_hash) =
            deploy_zk_verifier_with_vk(&mut app, &admin, &vk_bytes, &vk_b64);
        let mb_contract = deploy_moultbook(&mut app, &admin, &zk_contract, &vk_hash);

        app.execute_contract(
            moult_key.clone(),
            mb_contract.clone(),
            &ExecuteMsg::PublishAnon {
                topic_hash: "sha256:cafebabe00".to_string(),
                content_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                    .to_string(),
                proof_base64: proof_b64,
                public_inputs_base64: inputs_b64,
            },
            &[],
        )
        .unwrap();

        // Verify entry was created via reply handler
        let entries: EntriesResponse = app
            .wrap()
            .query_wasm_smart(
                &mb_contract,
                &QueryMsg::ListByMoultKey {
                    moult_key: moult_key.to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(entries.entries.len(), 1, "one entry should be created");

        let entry = &entries.entries[0];
        assert_eq!(entry.author, moult_key);
        assert!(entry.redacted_at.is_none());
        assert!(
            matches!(entry.attestation_ref, Some(AttestationRef::ZkProof { .. })),
            "expected ZkProof attestation ref, got {:?}",
            entry.attestation_ref
        );

        let stats: Stats = app
            .wrap()
            .query_wasm_smart(&mb_contract, &QueryMsg::GetStats {})
            .unwrap();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_active, 1);
    }

    /// Adversarial: garbage proof bytes → SubMsg fails → entire tx rolls back → no entry.
    #[test]
    fn publish_anon_invalid_proof_rejected_atomically() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let moult_key = app.api().addr_make("moult_key_bad");

        let (vk_bytes, vk_b64, _good_proof, inputs_b64) = gen_proof();
        let (zk_contract, vk_hash) =
            deploy_zk_verifier_with_vk(&mut app, &admin, &vk_bytes, &vk_b64);
        let mb_contract = deploy_moultbook(&mut app, &admin, &zk_contract, &vk_hash);

        // 16 zero bytes — not a valid Groth16 proof
        let garbage_proof = base64_encode(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00,
                                            0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00]);

        let err = app
            .execute_contract(
                moult_key.clone(),
                mb_contract.clone(),
                &ExecuteMsg::PublishAnon {
                    topic_hash: "sha256:badbadbad".to_string(),
                    content_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                        .to_string(),
                    proof_base64: garbage_proof,
                    public_inputs_base64: inputs_b64,
                },
                &[],
            )
            .unwrap_err();

        // The zk-verifier sub-message fails → whole tx aborts (reply_on_success)
        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("proof") || err_str.contains("deserialization") || err_str.contains("ProofInvalid"),
            "expected proof-related error, got: {}",
            err_str
        );

        // Atomicity check: no entry persisted, stats unchanged
        let entries: EntriesResponse = app
            .wrap()
            .query_wasm_smart(
                &mb_contract,
                &QueryMsg::ListByMoultKey {
                    moult_key: moult_key.to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(entries.entries.len(), 0, "no entries should survive a rolled-back tx");

        let stats: Stats = app
            .wrap()
            .query_wasm_smart(&mb_contract, &QueryMsg::GetStats {})
            .unwrap();
        assert_eq!(stats.total_entries, 0);
    }

    /// Helper: publish an anonymous entry and return its id.
    fn publish_and_get_id(
        app: &mut App,
        mb_contract: &Addr,
        moult_key: &Addr,
        proof_b64: &str,
        inputs_b64: &str,
        topic: &str,
    ) -> String {
        app.execute_contract(
            moult_key.clone(),
            mb_contract.clone(),
            &ExecuteMsg::PublishAnon {
                topic_hash: topic.to_string(),
                content_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
                    .to_string(),
                proof_base64: proof_b64.to_string(),
                public_inputs_base64: inputs_b64.to_string(),
            },
            &[],
        )
        .unwrap();

        let entries: EntriesResponse = app
            .wrap()
            .query_wasm_smart(
                mb_contract,
                &QueryMsg::ListByMoultKey {
                    moult_key: moult_key.to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        entries.entries[0].id.clone()
    }

    /// Happy path: a valid derivation proof persists the disclosure via reply.
    #[test]
    fn voluntary_disclose_valid_proof_persists_disclosure() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let moult_key = app.api().addr_make("moult_key_disc");
        let primary = app.api().addr_make("primary_identity");

        let (vk_bytes, vk_b64, proof_b64, inputs_b64) = gen_proof();
        let (zk_contract, vk_hash) =
            deploy_zk_verifier_with_vk(&mut app, &admin, &vk_bytes, &vk_b64);
        let mb_contract = deploy_moultbook(&mut app, &admin, &zk_contract, &vk_hash);

        let entry_id = publish_and_get_id(
            &mut app,
            &mb_contract,
            &moult_key,
            &proof_b64,
            &inputs_b64,
            "sha256:disc01",
        );

        app.execute_contract(
            moult_key.clone(),
            mb_contract.clone(),
            &ExecuteMsg::VoluntaryDisclose {
                entry_id: entry_id.clone(),
                primary_key: primary.to_string(),
                derivation_proof_base64: proof_b64.clone(),
                derivation_public_inputs_base64: inputs_b64.clone(),
            },
            &[],
        )
        .unwrap();

        let disclosure: Option<Disclosure> = app
            .wrap()
            .query_wasm_smart(
                &mb_contract,
                &QueryMsg::GetDisclosure {
                    entry_id: entry_id.clone(),
                },
            )
            .unwrap();
        let disclosure = disclosure.expect("disclosure should be persisted");
        assert_eq!(disclosure.entry_id, entry_id);
        assert_eq!(disclosure.primary_key, primary);
    }

    /// Adversarial: garbage derivation proof → SubMsg fails → tx rolls back → no disclosure.
    #[test]
    fn voluntary_disclose_invalid_proof_rejected_atomically() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let moult_key = app.api().addr_make("moult_key_disc_bad");
        let primary = app.api().addr_make("primary_identity_bad");

        let (vk_bytes, vk_b64, proof_b64, inputs_b64) = gen_proof();
        let (zk_contract, vk_hash) =
            deploy_zk_verifier_with_vk(&mut app, &admin, &vk_bytes, &vk_b64);
        let mb_contract = deploy_moultbook(&mut app, &admin, &zk_contract, &vk_hash);

        let entry_id = publish_and_get_id(
            &mut app,
            &mb_contract,
            &moult_key,
            &proof_b64,
            &inputs_b64,
            "sha256:disc02",
        );

        let garbage_proof = base64_encode(&[
            0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00,
            0x00, 0x00,
        ]);

        let err = app
            .execute_contract(
                moult_key.clone(),
                mb_contract.clone(),
                &ExecuteMsg::VoluntaryDisclose {
                    entry_id: entry_id.clone(),
                    primary_key: primary.to_string(),
                    derivation_proof_base64: garbage_proof,
                    derivation_public_inputs_base64: inputs_b64.clone(),
                },
                &[],
            )
            .unwrap_err();

        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("proof")
                || err_str.contains("deserialization")
                || err_str.contains("ProofInvalid"),
            "expected proof-related error, got: {}",
            err_str
        );

        // Atomicity: no disclosure persisted.
        let disclosure: Option<Disclosure> = app
            .wrap()
            .query_wasm_smart(&mb_contract, &QueryMsg::GetDisclosure { entry_id })
            .unwrap();
        assert!(
            disclosure.is_none(),
            "no disclosure should survive a rolled-back tx"
        );
    }

    /// Authorization: a non-author cannot disclose someone else's entry.
    #[test]
    fn voluntary_disclose_rejects_non_author() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let moult_key = app.api().addr_make("moult_key_owner");
        let intruder = app.api().addr_make("intruder");
        let primary = app.api().addr_make("primary_identity_2");

        let (vk_bytes, vk_b64, proof_b64, inputs_b64) = gen_proof();
        let (zk_contract, vk_hash) =
            deploy_zk_verifier_with_vk(&mut app, &admin, &vk_bytes, &vk_b64);
        let mb_contract = deploy_moultbook(&mut app, &admin, &zk_contract, &vk_hash);

        let entry_id = publish_and_get_id(
            &mut app,
            &mb_contract,
            &moult_key,
            &proof_b64,
            &inputs_b64,
            "sha256:disc03",
        );

        let err = app
            .execute_contract(
                intruder.clone(),
                mb_contract.clone(),
                &ExecuteMsg::VoluntaryDisclose {
                    entry_id: entry_id.clone(),
                    primary_key: primary.to_string(),
                    derivation_proof_base64: proof_b64.clone(),
                    derivation_public_inputs_base64: inputs_b64.clone(),
                },
                &[],
            )
            .unwrap_err();

        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("NotEntryAuthor") || err_str.contains("not authored"),
            "expected author-mismatch error, got: {}",
            err_str
        );

        let disclosure: Option<Disclosure> = app
            .wrap()
            .query_wasm_smart(&mb_contract, &QueryMsg::GetDisclosure { entry_id })
            .unwrap();
        assert!(disclosure.is_none());
    }
}
