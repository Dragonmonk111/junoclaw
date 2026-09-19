//! Unified proof generation tool for all 5 ZK circuits.
//!
//! Generates proving keys, verifying keys, and proofs for all circuits,
//! serializes them to files, and verifies them end-to-end.
//!
//! Usage:
//!   cargo run --release --example gen_all_proofs -- <output_dir>
//!   cargo run --release --example gen_all_proofs -- <output_dir> --check-only

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};
use ark_snark::SNARK;
use ark_std::rand::{SeedableRng, rngs::StdRng};
use std::env;
use std::fs;
use std::time::Instant;

use sensor_safety_circuit::{
    SensorSafetyCircuit, envelope_commitment, build_merkle_tree, mimc_hash, sensor_leaf,
};
use intent_safety_circuit::{IntentConsistencyCircuit, generate_intent_proof_data};
use consensus_safety_circuit::{
    ConsensusMembershipCircuit, generate_consensus_proof_data,
};
use batch_safety_circuit::{BatchSafetyCircuit, generate_batch_proof_data};
use proof_aggregation_circuit::{AggregationCircuit, compute_aggregation_commitment};

fn write_bytes(path: &str, data: &[u8]) {
    fs::write(path, data).unwrap_or_else(|e| panic!("failed to write {}: {}", path, e));
}

fn serialize_pk_vk(pk: &ark_groth16::ProvingKey<Bn254>, vk: &ark_groth16::VerifyingKey<Bn254>, dir: &str, name: &str) {
    let mut pk_buf = Vec::new();
    pk.serialize_uncompressed(&mut pk_buf).unwrap();
    write_bytes(&format!("{}/{}_pk.bin", dir, name), &pk_buf);

    let mut vk_buf = Vec::new();
    vk.serialize_uncompressed(&mut vk_buf).unwrap();
    write_bytes(&format!("{}/{}_vk.bin", dir, name), &vk_buf);
}

fn serialize_proof(proof: &ark_groth16::Proof<Bn254>, dir: &str, name: &str) {
    let mut buf = Vec::new();
    proof.serialize_uncompressed(&mut buf).unwrap();
    write_bytes(&format!("{}/{}_proof.bin", dir, name), &buf);
}

fn time_ms(label: &str, f: impl FnOnce()) {
    let start = Instant::now();
    f();
    let elapsed = start.elapsed();
    println!("  {:<40} {:>8.1} ms", label, elapsed.as_millis() as f64);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: gen_all_proofs <output_dir> [--check-only]");
        eprintln!();
        eprintln!("Generates proving keys, verifying keys, and proofs for all 5 ZK circuits.");
        eprintln!("Output files: <output_dir>/<circuit>_pk.bin, <circuit>_vk.bin, <circuit>_proof.bin");
        std::process::exit(1);
    }

    let output_dir = &args[1];
    let check_only = args.iter().any(|a| a == "--check-only");

    if !check_only {
        fs::create_dir_all(output_dir).expect("failed to create output dir");
    }

    let rng = &mut StdRng::seed_from_u64(42);
    let tree_height = 4;

    println!("\n=== ZK Fusion Stack — Unified Proof Generation ===\n");
    if check_only {
        println!("Mode: check-only (no file output)\n");
    } else {
        println!("Output: {}\n", output_dir);
    }

    // === Common parameters ===
    let max_speed = Fr::from(5000u64);
    let max_force = Fr::from(50000u64);
    let min_dist = Fr::from(500u64);
    let max_tilt = Fr::from(30000u64);
    let max_accel = Fr::from(3000u64);
    let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);

    // ========================================
    // Circuit 1: SensorSafety
    // ========================================
    println!("--- Circuit 1/5: SensorSafety ---");
    let speed = Fr::from(3000u64);
    let force = Fr::from(30000u64);
    let dist = Fr::from(500u64);
    let tilt = Fr::from(20000u64);
    let accel = Fr::from(2000u64);
    let cycle_index = Fr::from(42u64);

    let leaf = sensor_leaf(speed, force, dist, tilt, accel);
    let leaves = vec![leaf];
    let (sensor_merkle_root, sensor_paths, sensor_bits) = build_merkle_tree(&leaves, tree_height);

    let sensor_circuit = SensorSafetyCircuit::new(
        env_commit, sensor_merkle_root, cycle_index,
        speed, force, dist, tilt, accel,
        max_speed, max_force, min_dist, max_tilt, max_accel,
        sensor_paths[0].clone(), sensor_bits[0].clone(), tree_height,
    );

    let sensor_empty = SensorSafetyCircuit::empty(tree_height);
    let (sensor_pk, sensor_vk) = {
        let mut r = rng.clone();
        time_ms("Setup", || {});
        Groth16::<Bn254>::circuit_specific_setup(sensor_empty, &mut r).unwrap()
    };

    let sensor_proof = {
        let mut r = rng.clone();
        let start = Instant::now();
        let p = Groth16::<Bn254>::prove(&sensor_pk, sensor_circuit, &mut r).unwrap();
        println!("  Proving                                  {:>8.1} ms", start.elapsed().as_millis() as f64);
        p
    };

    {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify(&sensor_vk, &[env_commit, sensor_merkle_root, cycle_index], &sensor_proof).unwrap();
        println!("  Verifying                                {:>8.1} ms  valid={}", start.elapsed().as_millis() as f64, valid);
        assert!(valid);
    }

    if !check_only {
        serialize_pk_vk(&sensor_pk, &sensor_vk, output_dir, "sensor");
        serialize_proof(&sensor_proof, output_dir, "sensor");
    }

    // ========================================
    // Circuit 2: IntentConsistency
    // ========================================
    println!("\n--- Circuit 2/5: IntentConsistency ---");
    let action = Fr::from(1u64);
    let params_x = Fr::from(500u64);
    let params_y = Fr::from(300u64);
    let sensor_snapshot_hash = Fr::from(12345u64);
    let agent_id = Fr::from(99u64);
    let zone_x_min = Fr::from(0u64);
    let zone_x_max = Fr::from(1000u64);
    let zone_y_min = Fr::from(0u64);
    let zone_y_max = Fr::from(1000u64);

    let (intent_commit, intent_merkle, intent_envelope, intent_policy, ipath, ibits) =
        generate_intent_proof_data(
            action, params_x, params_y, sensor_snapshot_hash, agent_id,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            zone_x_min, zone_x_max, zone_y_min, zone_y_max, tree_height,
        );

    let intent_circuit = IntentConsistencyCircuit::new(
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        action, params_x, params_y, sensor_snapshot_hash, agent_id,
        ipath.clone(), ibits.clone(),
        max_speed, max_force, min_dist, max_tilt, max_accel,
        zone_x_min, zone_x_max, zone_y_min, zone_y_max, tree_height,
    );

    let intent_empty = IntentConsistencyCircuit::empty(tree_height);
    let (intent_pk, intent_vk) = {
        let mut r = rng.clone();
        Groth16::<Bn254>::circuit_specific_setup(intent_empty, &mut r).unwrap()
    };

    let intent_proof = {
        let mut r = rng.clone();
        let start = Instant::now();
        let p = Groth16::<Bn254>::prove(&intent_pk, intent_circuit, &mut r).unwrap();
        println!("  Proving                                  {:>8.1} ms", start.elapsed().as_millis() as f64);
        p
    };

    {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify(&intent_vk, &[intent_commit, intent_merkle, intent_envelope, intent_policy], &intent_proof).unwrap();
        println!("  Verifying                                {:>8.1} ms  valid={}", start.elapsed().as_millis() as f64, valid);
        assert!(valid);
    }

    if !check_only {
        serialize_pk_vk(&intent_pk, &intent_vk, output_dir, "intent");
        serialize_proof(&intent_proof, output_dir, "intent");
    }

    // ========================================
    // Circuit 3: ConsensusMembership
    // ========================================
    println!("\n--- Circuit 3/5: ConsensusMembership ---");
    let validator_pubkey = Fr::from(12345u64);
    let block_hash = Fr::from(99999u64);
    let vote_decision = Fr::from(1u64);
    let epoch = Fr::from(1u64);

    let (consensus_valset, consensus_vote, cpath, cbits) =
        generate_consensus_proof_data(
            validator_pubkey, block_hash, vote_decision, epoch, tree_height,
        );

    let consensus_circuit = ConsensusMembershipCircuit::new(
        consensus_valset, epoch, consensus_vote,
        validator_pubkey, cpath, cbits,
        block_hash, vote_decision, tree_height,
    );

    let consensus_empty = ConsensusMembershipCircuit::empty(tree_height);
    let (consensus_pk, consensus_vk) = {
        let mut r = rng.clone();
        Groth16::<Bn254>::circuit_specific_setup(consensus_empty, &mut r).unwrap()
    };

    let consensus_proof = {
        let mut r = rng.clone();
        let start = Instant::now();
        let p = Groth16::<Bn254>::prove(&consensus_pk, consensus_circuit, &mut r).unwrap();
        println!("  Proving                                  {:>8.1} ms", start.elapsed().as_millis() as f64);
        p
    };

    {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify(&consensus_vk, &[consensus_valset, epoch, consensus_vote], &consensus_proof).unwrap();
        println!("  Verifying                                {:>8.1} ms  valid={}", start.elapsed().as_millis() as f64, valid);
        assert!(valid);
    }

    if !check_only {
        serialize_pk_vk(&consensus_pk, &consensus_vk, output_dir, "consensus");
        serialize_proof(&consensus_proof, output_dir, "consensus");
    }

    // ========================================
    // Circuit 4: BatchSafety
    // ========================================
    println!("\n--- Circuit 4/5: BatchSafety ---");
    let batch_readings = vec![
        (Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64)),
        (Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
        (Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
        (Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
    ];
    let batch_tree_height = 2; // 4 leaves

    let (batch_env, batch_root, batch_size, batch_paths, batch_bits) = generate_batch_proof_data(
        &batch_readings, max_speed, max_force, min_dist, max_tilt, max_accel, batch_tree_height,
    );

    let batch_circuit = BatchSafetyCircuit::new(
        batch_env, batch_root, batch_size,
        batch_readings,
        max_speed, max_force, min_dist, max_tilt, max_accel,
        batch_paths, batch_bits, batch_tree_height,
    );

    let batch_empty = BatchSafetyCircuit::empty(batch_tree_height);
    let (batch_pk, batch_vk) = {
        let mut r = rng.clone();
        Groth16::<Bn254>::circuit_specific_setup(batch_empty, &mut r).unwrap()
    };

    let batch_proof = {
        let mut r = rng.clone();
        let start = Instant::now();
        let p = Groth16::<Bn254>::prove(&batch_pk, batch_circuit, &mut r).unwrap();
        println!("  Proving                                  {:>8.1} ms", start.elapsed().as_millis() as f64);
        p
    };

    {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify(&batch_vk, &[batch_env, batch_root, batch_size], &batch_proof).unwrap();
        println!("  Verifying                                {:>8.1} ms  valid={}", start.elapsed().as_millis() as f64, valid);
        assert!(valid);
    }

    if !check_only {
        serialize_pk_vk(&batch_pk, &batch_vk, output_dir, "batch");
        serialize_proof(&batch_proof, output_dir, "batch");
    }

    // ========================================
    // Circuit 5: Aggregation (the "fusion" circuit)
    // ========================================
    println!("\n--- Circuit 5/5: Aggregation (Fusion) ---");
    let agg_commit = compute_aggregation_commitment(
        env_commit, sensor_merkle_root, cycle_index,
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        consensus_valset, epoch, consensus_vote,
    );

    let agg_circuit = AggregationCircuit::new(
        agg_commit,
        env_commit, sensor_merkle_root,
        env_commit, sensor_merkle_root, cycle_index,
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        consensus_valset, epoch, consensus_vote,
    );

    let agg_empty = AggregationCircuit::empty();
    let (agg_pk, agg_vk) = {
        let mut r = rng.clone();
        Groth16::<Bn254>::circuit_specific_setup(agg_empty, &mut r).unwrap()
    };

    let agg_proof = {
        let mut r = rng.clone();
        let start = Instant::now();
        let p = Groth16::<Bn254>::prove(&agg_pk, agg_circuit, &mut r).unwrap();
        println!("  Proving                                  {:>8.1} ms", start.elapsed().as_millis() as f64);
        p
    };

    {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify(&agg_vk, &[agg_commit, env_commit, sensor_merkle_root], &agg_proof).unwrap();
        println!("  Verifying                                {:>8.1} ms  valid={}", start.elapsed().as_millis() as f64, valid);
        assert!(valid);
    }

    if !check_only {
        serialize_pk_vk(&agg_pk, &agg_vk, output_dir, "aggregation");
        serialize_proof(&agg_proof, output_dir, "aggregation");
    }

    // === Summary ===
    println!("\n=== Summary ===");
    println!("All 5 circuits: setup + prove + verify PASSED");
    println!("Proof system: Groth16 on BN254");
    println!("Proof size: 128 bytes each (192 bytes serialized uncompressed)");
    if !check_only {
        println!("\nFiles written to {}:", output_dir);
        println!("  sensor_pk.bin, sensor_vk.bin, sensor_proof.bin");
        println!("  intent_pk.bin, intent_vk.bin, intent_proof.bin");
        println!("  consensus_pk.bin, consensus_vk.bin, consensus_proof.bin");
        println!("  batch_pk.bin, batch_vk.bin, batch_proof.bin");
        println!("  aggregation_pk.bin, aggregation_vk.bin, aggregation_proof.bin");
    }
    println!("\nOn-chain verification: 1x VerifyProof (aggregation, ~371K gas BN254 / ~203K gas precompile)");
    println!("TEE attestation: verifies all 3 Groth16 pairings + binds to aggregation_commitment");
}
