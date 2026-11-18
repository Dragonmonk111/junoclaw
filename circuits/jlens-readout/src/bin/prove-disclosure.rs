//! Prove a real J-Lens disclosure end-to-end: takes the same
//! `JlensReadout` JSON jc-mem stores in `CommitJlens`, builds the MiMC
//! witness, runs Groth16, and writes vk/proof/inputs artifacts +
//! `meta.json` (the anchor fields: MiMC activation root as the contract
//! expects it — hex of public_inputs[0] — plus the declared claims).
//!
//! Scope: v1 proof path mirrors the devnet fixture — the activation leaf
//! sits at `leaf_index` in a tree whose remaining leaves are the zero
//! leaf (mimc_hash(0,0)). A production capture pipeline must anchor the
//! MiMC root of the full tree; the sha256 `merkle_path` in the
//! disclosure belongs to the `resolve_with_recompute` path and is not
//! consumed here.
//!
//! Usage:
//!   prove-disclosure --disclosure readout.json [--height 4] \
//!     [--out devnet/proof-artifacts-disclosure]

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use jlens_readout_circuit::{
    bank_commitment_native, build_merkle_tree_for_fixture, detection_native,
    readout_leaf_native, JLensReadoutCircuit, DIM, SCALE,
};
use sensor_safety_circuit::mimc_hash;

#[derive(Deserialize)]
struct Disclosure {
    activations: Vec<i8>,
    bank_vec: Vec<i8>,
    threshold: u64,
    leaf_index: u32,
}

#[derive(Serialize)]
struct Meta {
    /// Hex of the first public input — the value the contract stores as
    /// `activation_root` on the anchored attestation.
    activation_root: String,
    /// Hex MiMC bank commitment (second public input).
    bank_commitment: String,
    score: u64,
    integrity: u64,
    detection: bool,
    /// The `attestation_id` the anchor should use; caller may override.
    suggested_attestation_id: String,
    tree_height: usize,
    leaf_index: u32,
    disclosure_sha256: String,
}

fn i8_to_fr(v: i8) -> Fr {
    if v >= 0 {
        Fr::from(v as u64)
    } else {
        -Fr::from((-v) as u64)
    }
}

fn main() {
    let mut disclosure_path: Option<PathBuf> = None;
    let mut height = 4usize;
    let mut out_dir = PathBuf::from("devnet/proof-artifacts-disclosure");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--disclosure" => disclosure_path = Some(PathBuf::from(args.next().expect("--disclosure value"))),
            "--height" => height = args.next().expect("--height value").parse().expect("height int"),
            "--out" => out_dir = PathBuf::from(args.next().expect("--out value")),
            other => panic!("unknown arg {other}"),
        }
    }
    let disclosure_path = disclosure_path.expect("--disclosure <path.json> required");
    let raw = fs::read(&disclosure_path).expect("read disclosure");
    let d: Disclosure = serde_json::from_slice(&raw).expect("parse disclosure JSON");
    use sha2::Digest;
    let disclosure_sha256 = hex::encode(sha2::Sha256::digest(&raw));

    assert_eq!(d.activations.len(), DIM, "activations must be {DIM} int8s");
    assert_eq!(d.bank_vec.len(), DIM, "bank_vec must be {DIM} int8s");
    let leaf_idx = d.leaf_index as usize;
    assert!(leaf_idx < (1 << height), "leaf_index {leaf_idx} >= 2^{height}");

    let acts: Vec<Fr> = d.activations.iter().map(|v| i8_to_fr(*v)).collect();
    let bank: Vec<Fr> = d.bank_vec.iter().map(|v| i8_to_fr(*v)).collect();
    let threshold = Fr::from(d.threshold);

    let score_i64: i64 = d
        .activations
        .iter()
        .zip(d.bank_vec.iter())
        .map(|(a, b)| (*a as i64) * (*b as i64))
        .sum();
    assert!(score_i64 >= 0, "negative score can never resolve Clean on-chain");
    let score_i64 = score_i64 as u64;
    assert!(score_i64 <= SCALE, "score {score_i64} exceeds SCALE {SCALE}");

    let score = Fr::from(score_i64);
    let integrity = Fr::from(SCALE) - score;
    let detection = detection_native(score, threshold);
    let bank_commit = bank_commitment_native(&bank, threshold);

    let leaf = readout_leaf_native(&acts);
    let zero_leaf = mimc_hash(Fr::from(0u64), Fr::from(0u64));
    let mut leaves = vec![zero_leaf; 1 << height];
    leaves[leaf_idx] = leaf;
    let (root, paths, bits) = build_merkle_tree_for_fixture(&leaves, height);

    let mut rng = StdRng::seed_from_u64(42);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(
        JLensReadoutCircuit::empty(height),
        &mut rng,
    )
    .expect("setup failed");

    let circuit = JLensReadoutCircuit {
        merkle_root: Some(root),
        bank_commitment: Some(bank_commit),
        score_out: Some(score),
        integrity_out: Some(integrity),
        detection_out: Some(detection),
        activations: acts.iter().map(|a| Some(*a)).collect(),
        bank_vec: bank.iter().map(|b| Some(*b)).collect(),
        threshold: Some(threshold),
        merkle_path: paths[leaf_idx].iter().map(|p| Some(*p)).collect(),
        path_bits: bits[leaf_idx].iter().map(|b| Some(*b)).collect(),
        tree_height: height,
    };

    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).expect("prove failed");
    let public_inputs = vec![root, bank_commit, score, integrity, detection];
    assert!(
        Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).expect("verify failed"),
        "local verification failed"
    );
    println!("local Groth16 verification: VALID");

    let engine = base64::engine::general_purpose::STANDARD;
    let mut vk_bytes = Vec::new();
    vk.serialize_compressed(&mut vk_bytes).unwrap();
    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes).unwrap();
    let mut inputs_bytes = Vec::new();
    for i in &public_inputs {
        i.serialize_compressed(&mut inputs_bytes).unwrap();
    }

    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("jlens-vk.b64"), engine.encode(&vk_bytes)).unwrap();
    fs::write(out_dir.join("jlens-proof.b64"), engine.encode(&proof_bytes)).unwrap();
    fs::write(out_dir.join("jlens-inputs.b64"), engine.encode(&inputs_bytes)).unwrap();

    let meta = Meta {
        activation_root: hex::encode(&inputs_bytes[0..32]),
        bank_commitment: hex::encode(&inputs_bytes[32..64]),
        score: score_i64,
        integrity: SCALE - score_i64,
        detection: detection == Fr::from(1u64),
        suggested_attestation_id: format!("jlens-{}", &disclosure_sha256[..12]),
        tree_height: height,
        leaf_index: d.leaf_index,
        disclosure_sha256,
    };
    fs::write(
        out_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();

    println!("score={score_i64} integrity={} detection={} root={}",
        SCALE - score_i64, meta.detection, meta.activation_root);
    println!("artifacts -> {}", out_dir.display());
}
