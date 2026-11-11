//! Generate Groth16 proof artifacts for the J-Lens readout circuit.
//! Writes vk.b64 / proof.b64 / inputs.b64 — consumable by `StoreVk` +
//! `VerifyProof` on the zk-verifier contract.
//!
//! Usage: cargo run -p jlens-readout-circuit --bin gen-proof

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use base64::Engine;
use std::fs;
use std::path::Path;

use jlens_readout_circuit::{
    bank_commitment_native, build_merkle_tree_for_fixture, readout_leaf_native,
    JLensReadoutCircuit, DIM, SCALE,
};
use sensor_safety_circuit::mimc_hash;

fn main() {
    let mut rng = StdRng::seed_from_u64(42);

    // Deterministic fixture readout — stands in for a real probe capture.
    let acts: Vec<Fr> = (0..DIM).map(|i| Fr::from(10u64 + i as u64)).collect();
    let bank: Vec<Fr> = (0..DIM).map(|i| Fr::from(3u64 + i as u64)).collect();
    let threshold = Fr::from(500u64);

    let score: Fr = acts.iter().zip(bank.iter()).map(|(a, b)| *a * *b).sum();
    let integrity = Fr::from(SCALE) - score;
    let bank_commit = bank_commitment_native(&bank, threshold);

    let height = 4usize;
    let leaf = readout_leaf_native(&acts);
    let zero_leaf = mimc_hash(Fr::from(0u64), Fr::from(0u64));
    let mut leaves = vec![zero_leaf; (1 << height) - 1];
    leaves.push(leaf);
    let (root, paths, bits) = build_merkle_tree_for_fixture(&leaves, height);
    let leaf_idx = leaves.len() - 1;

    // Trusted setup (circuit-specific; production uses a universal/Powers-of-Tau ceremony)
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
        activations: acts.iter().map(|a| Some(*a)).collect(),
        bank_vec: bank.iter().map(|b| Some(*b)).collect(),
        threshold: Some(threshold),
        merkle_path: paths[leaf_idx].iter().map(|p| Some(*p)).collect(),
        path_bits: bits[leaf_idx].iter().map(|b| Some(*b)).collect(),
        tree_height: height,
    };

    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).expect("prove failed");

    // Public inputs in allocation order: merkle_root, bank_commitment,
    // score_out, integrity_out.
    let public_inputs = vec![root, bank_commit, score, integrity];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).expect("verify failed");
    assert!(valid, "local verification failed");
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

    let out_dir = Path::new("devnet/proof-artifacts");
    fs::create_dir_all(out_dir).unwrap();
    fs::write(out_dir.join("jlens-vk.b64"), engine.encode(&vk_bytes)).unwrap();
    fs::write(out_dir.join("jlens-proof.b64"), engine.encode(&proof_bytes)).unwrap();
    fs::write(out_dir.join("jlens-inputs.b64"), engine.encode(&inputs_bytes)).unwrap();

    println!("vk bytes: {}  proof bytes: {}  inputs bytes: {}",
        vk_bytes.len(), proof_bytes.len(), inputs_bytes.len());
    println!("score={} integrity={} root={}", score, integrity, root);
    println!("artifacts -> {}", out_dir.display());
}
