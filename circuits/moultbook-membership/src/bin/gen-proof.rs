//! Generate a Groth16 proof + VK fixture for the moultbook-membership circuit.
//!
//! Outputs to `devnet/proof-artifacts/`:
//!   - vk.b64      — base64-encoded VerifyingKey<Bn254>
//!   - proof.b64   — base64-encoded Proof<Bn254>
//!   - inputs.b64  — base64-encoded public inputs [Fr]
//!   - vk_hash.hex — SHA-256 of the raw VK bytes (for moultbook membership_vk_hash)
//!
//! Usage: cargo run --bin gen-proof

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use moultbook_membership_circuit::{build_merkle_tree, mimc_hash, MembershipCircuit};

fn main() {
    let rng = &mut StdRng::seed_from_u64(42);
    let tree_height = 3; // Small tree for fast setup
    let zero = Fr::from(0u64);

    // ── 1. Build a test membership tree ──
    let primary_keys: Vec<Fr> = (1..=4).map(|i| Fr::from(i as u64)).collect();
    let leaves: Vec<Fr> = primary_keys.iter().map(|k| mimc_hash(*k, zero)).collect();
    let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

    // Agent 2 (index 1) will generate the proof
    let agent_idx = 1usize;
    let primary_key = primary_keys[agent_idx];
    let derivation_salt = Fr::from(12345u64);
    let moult_key = mimc_hash(primary_key, derivation_salt);
    let moult_key_hash = mimc_hash(moult_key, zero);
    let epoch = Fr::from(100u64);

    // ── 2. Setup: generate proving key + verification key ──
    let empty_circuit = MembershipCircuit::empty(tree_height);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng)
        .expect("Groth16 setup failed");

    // ── 3. Prove ──
    let circuit = MembershipCircuit::new(
        moult_key_hash,
        merkle_root,
        epoch,
        primary_key,
        derivation_salt,
        paths[agent_idx].clone(),
        bits[agent_idx].clone(),
        tree_height,
    );

    let proof = Groth16::<Bn254>::prove(&pk, circuit, rng).expect("Proof generation failed");

    // ── 4. Verify locally ──
    let public_inputs = vec![moult_key_hash, merkle_root, epoch];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).expect("Verification failed");
    assert!(valid, "Local verification failed — this is a bug");

    println!("✓ Local verification passed");

    // ── 5. Serialize to base64 ──
    let engine = base64::engine::general_purpose::STANDARD;

    let mut vk_bytes = Vec::new();
    vk.serialize_compressed(&mut vk_bytes)
        .expect("VK serialization failed");
    let vk_b64 = engine.encode(&vk_bytes);

    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes)
        .expect("Proof serialization failed");
    let proof_b64 = engine.encode(&proof_bytes);

    let mut inputs_bytes = Vec::new();
    for input in &public_inputs {
        input.serialize_compressed(&mut inputs_bytes)
            .expect("Input serialization failed");
    }
    let inputs_b64 = engine.encode(&inputs_bytes);

    // ── 6. Compute VK hash ──
    let vk_hash = Sha256::digest(&vk_bytes);
    let vk_hash_hex = hex::encode(&vk_hash);

    // ── 7. Write artifacts ──
    let out_dir = Path::new("devnet/proof-artifacts");
    fs::create_dir_all(out_dir).expect("Failed to create output directory");

    fs::write(out_dir.join("vk.b64"), &vk_b64).expect("Failed to write vk.b64");
    fs::write(out_dir.join("proof.b64"), &proof_b64).expect("Failed to write proof.b64");
    fs::write(out_dir.join("inputs.b64"), &inputs_b64).expect("Failed to write inputs.b64");
    fs::write(out_dir.join("vk_hash.hex"), &vk_hash_hex).expect("Failed to write vk_hash.hex");

    println!("\n═══════════════════════════════════════════════════");
    println!("  Proof artifacts generated");
    println!("═══════════════════════════════════════════════════");
    println!("  VK size:        {} bytes", vk_bytes.len());
    println!("  Proof size:     {} bytes", proof_bytes.len());
    println!("  Inputs size:    {} bytes", inputs_bytes.len());
    println!("  VK SHA-256:     {}", vk_hash_hex);
    println!("\n  Files written to: {}", out_dir.display());
    println!("    vk.b64       → StoreVk on zk-verifier");
    println!("    proof.b64    → PublishAnon proof_base64");
    println!("    inputs.b64   → PublishAnon public_inputs_base64");
    println!("    vk_hash.hex  → moultbook membership_vk_hash");
    println!("═══════════════════════════════════════════════════");
}
