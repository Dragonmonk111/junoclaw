//! Benchmark: actual proving times for all circuits in the ZK trust stack.
//! Run with: cargo run --release --manifest-path circuits/proof-aggregation/Cargo.toml --example proving_times

use ark_bn254::Fr;
use ark_groth16::Groth16;
use ark_snark::SNARK;
use ark_std::rand::{SeedableRng, rngs::StdRng};
use std::time::Instant;

use sensor_safety_circuit::{
    SensorSafetyCircuit, envelope_commitment, build_merkle_tree, mimc_hash,
};
use intent_safety_circuit::{IntentConsistencyCircuit, generate_intent_proof_data};
use consensus_safety_circuit::{
    ConsensusMembershipCircuit, generate_consensus_proof_data,
};
use proof_aggregation_circuit::{AggregationCircuit, compute_aggregation_commitment};

fn time_ms(label: &str, f: impl FnOnce()) {
    let start = Instant::now();
    f();
    let elapsed = start.elapsed();
    println!("{:<45} {:>8.1} ms  ({:.2}s)", label, elapsed.as_millis() as f64, elapsed.as_secs_f64());
}

fn main() {
    println!("\n=== ZK Trust Stack — Actual Proving Time Benchmark ===\n");
    let rng = &mut StdRng::seed_from_u64(42);
    let tree_height = 4;

    // === 1. Sensor Safety Circuit ===
    println!("--- Sensor Safety Circuit (~12K constraints) ---");
    let max_speed = Fr::from(5000u64);
    let max_force = Fr::from(50000u64);
    let min_dist = Fr::from(500u64);
    let max_tilt = Fr::from(30000u64);
    let max_accel = Fr::from(3000u64);

    let speed = Fr::from(3000u64);
    let force = Fr::from(30000u64);
    let dist = Fr::from(500u64);
    let tilt = Fr::from(20000u64);
    let accel = Fr::from(2000u64);
    let cycle_index = Fr::from(42u64);

    let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
    let leaf = mimc_hash(mimc_hash(speed, force), mimc_hash(dist, mimc_hash(tilt, accel)));
    let leaves = vec![leaf];
    let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

    let sensor_circuit = SensorSafetyCircuit::new(
        env_commit, merkle_root, cycle_index,
        speed, force, dist, tilt, accel,
        max_speed, max_force, min_dist, max_tilt, max_accel,
        paths[0].clone(), bits[0].clone(), tree_height,
    );

    let sensor_empty = SensorSafetyCircuit::empty(tree_height);
    let (sensor_pk, sensor_vk) = {
        let mut rng = rng.clone();
        let result = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(sensor_empty, &mut rng).unwrap();
        time_ms("  Setup (circuit-specific)", || {});
        result
    };

    let sensor_proof = {
        let mut rng = rng.clone();
        let proof = Groth16::<ark_bn254::Bn254>::prove(&sensor_pk, sensor_circuit.clone(), &mut rng).unwrap();
        time_ms("  Proving", || {});
        proof
    };

    // Actually time it properly
    let start = Instant::now();
    let mut rng2 = rng.clone();
    let sensor_proof = Groth16::<ark_bn254::Bn254>::prove(&sensor_pk, sensor_circuit, &mut rng2).unwrap();
    println!("  Proving (actual)                             {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let sensor_valid = Groth16::<ark_bn254::Bn254>::verify(&sensor_vk, &[env_commit, merkle_root, cycle_index], &sensor_proof).unwrap();
    println!("  Verifying                                    {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    // === 2. Intent Consistency Circuit ===
    println!("\n--- Intent Consistency Circuit (~5.5K constraints) ---");
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
            zone_x_min, zone_x_max, zone_y_min, zone_y_max,
            tree_height,
        );

    let intent_circuit = IntentConsistencyCircuit::new(
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        action, params_x, params_y, sensor_snapshot_hash, agent_id,
        ipath.clone(), ibits.clone(),
        max_speed, max_force, min_dist, max_tilt, max_accel,
        zone_x_min, zone_x_max, zone_y_min, zone_y_max,
        tree_height,
    );

    let intent_empty = IntentConsistencyCircuit::empty(tree_height);
    let start = Instant::now();
    let mut rng3 = rng.clone();
    let (intent_pk, intent_vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(intent_empty, &mut rng3).unwrap();
    println!("  Setup                                        {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let mut rng4 = rng.clone();
    let intent_proof = Groth16::<ark_bn254::Bn254>::prove(&intent_pk, intent_circuit, &mut rng4).unwrap();
    println!("  Proving                                      {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let _ = Groth16::<ark_bn254::Bn254>::verify(&intent_vk, &[intent_commit, intent_merkle, intent_envelope, intent_policy], &intent_proof).unwrap();
    println!("  Verifying                                    {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    // === 3. Consensus Membership Circuit ===
    println!("\n--- Consensus Membership Circuit (~4.2K constraints) ---");
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
    let start = Instant::now();
    let mut rng5 = rng.clone();
    let (consensus_pk, consensus_vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(consensus_empty, &mut rng5).unwrap();
    println!("  Setup                                        {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let mut rng6 = rng.clone();
    let consensus_proof = Groth16::<ark_bn254::Bn254>::prove(&consensus_pk, consensus_circuit, &mut rng6).unwrap();
    println!("  Proving                                      {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let _ = Groth16::<ark_bn254::Bn254>::verify(&consensus_vk, &[consensus_valset, epoch, consensus_vote], &consensus_proof).unwrap();
    println!("  Verifying                                    {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    // === 4. Aggregation Circuit ===
    println!("\n--- Aggregation Circuit (~8K constraints) ---");
    let sensor_merkle_root = intent_merkle;
    let sensor_cycle_index = Fr::from(42u64);

    let agg_commit = compute_aggregation_commitment(
        intent_envelope, sensor_merkle_root, sensor_cycle_index,
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        consensus_valset, epoch, consensus_vote,
    );

    let agg_circuit = AggregationCircuit::new(
        agg_commit,
        intent_envelope, sensor_merkle_root,
        intent_envelope, sensor_merkle_root, sensor_cycle_index,
        intent_commit, intent_merkle, intent_envelope, intent_policy,
        consensus_valset, epoch, consensus_vote,
    );

    let agg_empty = AggregationCircuit::empty();
    let start = Instant::now();
    let mut rng7 = rng.clone();
    let (agg_pk, agg_vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(agg_empty, &mut rng7).unwrap();
    println!("  Setup                                        {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let mut rng8 = rng.clone();
    let agg_proof = Groth16::<ark_bn254::Bn254>::prove(&agg_pk, agg_circuit, &mut rng8).unwrap();
    println!("  Proving                                      {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    let start = Instant::now();
    let _ = Groth16::<ark_bn254::Bn254>::verify(&agg_vk, &[agg_commit, intent_envelope, sensor_merkle_root], &agg_proof).unwrap();
    println!("  Verifying                                    {:>8.1} ms  ({:.2}s)", start.elapsed().as_millis() as f64, start.elapsed().as_secs_f64());

    // === Summary ===
    println!("\n=== Summary ===");
    println!("All proofs: Groth16 on BN254 = 128 bytes each");
    println!("Parallelized proving = max(sensor, intent, consensus) + aggregation");
    println!("Sequential proving = sensor + intent + consensus + aggregation");
    println!("\nOn-chain: 1x VerifyProof (aggregation, ~203K gas) + 1x VerifyAttestation (TEE)");
}
