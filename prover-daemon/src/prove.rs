// Proof generation and verification.
//
// Wraps the sensor-safety, intent-safety, consensus-safety, and aggregation
// circuits to generate Groth16 proofs from robot sensor data.

use anyhow::{Context, Result};
use ark_bn254::{Bn254, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;
use sensor_safety_circuit::{
    build_merkle_tree, envelope_commitment, mimc_hash, sensor_leaf,
    SensorSafetyCircuit,
};
use std::path::Path;

use crate::merkle::compute_root_from_hashes;
use crate::{BatchResponse, CycleData};

/// Generate proving and verifying keys for all circuits.
pub fn setup_keys(output_dir: &Path, tree_height: usize) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    // Sensor safety keys
    let rng = &mut StdRng::seed_from_u64(42);
    let empty_circuit = SensorSafetyCircuit::empty(tree_height);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng)
        .map_err(|e| anyhow::anyhow!("sensor setup failed: {}", e))?;

    write_key(&pk, output_dir.join("sensor_proving_key.bin"))?;
    write_key(&vk, output_dir.join("sensor_verifying_key.bin"))?;

    tracing::info!("Sensor safety keys generated (tree_height={})", tree_height);

    // Intent consistency keys
    let rng = &mut StdRng::seed_from_u64(43);
    let empty_intent = intent_safety_circuit::IntentConsistencyCircuit::empty();
    let (pk_i, vk_i) = Groth16::<Bn254>::circuit_specific_setup(empty_intent, rng)
        .map_err(|e| anyhow::anyhow!("intent setup failed: {}", e))?;

    write_key(&pk_i, output_dir.join("intent_proving_key.bin"))?;
    write_key(&vk_i, output_dir.join("intent_verifying_key.bin"))?;

    tracing::info!("Intent consistency keys generated");

    // Consensus membership keys
    let rng = &mut StdRng::seed_from_u64(44);
    let empty_consensus = consensus_safety_circuit::ConsensusMembershipCircuit::empty();
    let (pk_c, vk_c) = Groth16::<Bn254>::circuit_specific_setup(empty_consensus, rng)
        .map_err(|e| anyhow::anyhow!("consensus setup failed: {}", e))?;

    write_key(&pk_c, output_dir.join("consensus_proving_key.bin"))?;
    write_key(&vk_c, output_dir.join("consensus_verifying_key.bin"))?;

    tracing::info!("Consensus membership keys generated");

    // Aggregation keys
    let rng = &mut StdRng::seed_from_u64(45);
    let empty_agg = proof_aggregation_circuit::AggregationCircuit::empty();
    let (pk_a, vk_a) = Groth16::<Bn254>::circuit_specific_setup(empty_agg, rng)
        .map_err(|e| anyhow::anyhow!("aggregation setup failed: {}", e))?;

    write_key(&pk_a, output_dir.join("aggregation_proving_key.bin"))?;
    write_key(&vk_a, output_dir.join("aggregation_verifying_key.bin"))?;

    tracing::info!("Aggregation keys generated");

    Ok(())
}

/// Generate a sensor safety proof from a reflex batch.
pub fn generate_sensor_proof(
    keys_dir: &Path,
    batch: &BatchResponse,
    robot_id: &str,
) -> Result<Vec<u8>> {
    let pk_path = keys_dir.join("sensor_proving_key.bin");
    let pk_bytes = std::fs::read(&pk_path)
        .with_context(|| format!("failed to read proving key from {}", pk_path.display()))?;
    let pk = ProvingKey::<Bn254>::deserialize_uncompressed(&pk_bytes[..])
        .context("failed to deserialize proving key")?;

    // Extract sensor readings from the first cycle (representative)
    let first_cycle = batch.cycles.first()
        .ok_or_else(|| anyhow::anyhow!("batch has no cycles"))?;

    let speed = extract_sensor_value(&first_cycle.sensor_readings, "speed");
    let force = extract_sensor_value(&first_cycle.sensor_readings, "force");
    let distance = extract_sensor_value(&first_cycle.sensor_readings, "distance");
    let tilt = extract_sensor_value(&first_cycle.sensor_readings, "tilt");
    let accel = extract_sensor_value(&first_cycle.sensor_readings, "accel");

    // Safety envelope (from on-chain governance — hardcoded for now)
    let max_speed = Fr::from(2000u64); // 2.0 m/s in milli-units
    let max_force = Fr::from(50000u64); // 50 N in milli-units
    let min_dist = Fr::from(500u64); // 0.5 m in milli-units
    let max_tilt = Fr::from(15000u64); // 15 degrees in milli-degrees
    let max_accel = Fr::from(3000u64); // 3.0 m/s² in milli-units

    // Build Merkle tree from cycle hashes
    let cycle_hashes: Vec<String> = batch.cycles.iter()
        .map(|c| c.cycle_hash.clone())
        .collect();
    let merkle_root_hex = compute_root_from_hashes(&cycle_hashes);

    // Build the leaf for the first cycle
    let leaf = sensor_leaf(speed, force, distance, tilt, accel);
    let zero = Fr::from(0u64);
    let zero_leaf = mimc_hash(zero, zero);
    let tree_height = 1;
    let (merkle_root, paths, bits) = build_merkle_tree(&[leaf, zero_leaf], tree_height);

    let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
    let cycle_index = Fr::from(0u64);

    let circuit = SensorSafetyCircuit::new(
        env_commit,
        merkle_root,
        cycle_index,
        speed, force, distance, tilt, accel,
        max_speed, max_force, min_dist, max_tilt, max_accel,
        paths[0].clone(),
        bits[0].clone(),
        tree_height,
    );

    let rng = &mut StdRng::seed_from_u64(123);
    let start = std::time::Instant::now();
    let proof = Groth16::<Bn254>::prove(&pk, circuit, rng)
        .context("proof generation failed")?;
    let elapsed = start.elapsed();

    tracing::info!(
        "Sensor safety proof generated in {}ms (robot={}, cycles={})",
        elapsed.as_millis(),
        robot_id,
        batch.cycle_count,
    );

    // Serialize proof
    let mut buf = Vec::new();
    proof.serialize_uncompressed(&mut buf)
        .context("failed to serialize proof")?;

    Ok(buf)
}

/// Verify a proof locally (before submitting on-chain).
pub fn verify_proof_local(
    vk_path: &Path,
    proof_path: &Path,
    public_inputs_json: &str,
) -> Result<bool> {
    let vk_bytes = std::fs::read(vk_path)?;
    let vk = VerifyingKey::<Bn254>::deserialize_uncompressed(&vk_bytes[..])?;

    let proof_bytes = std::fs::read(proof_path)?;
    let proof = Proof::<Bn254>::deserialize_uncompressed(&proof_bytes[..])?;

    let inputs: Vec<String> = serde_json::from_str(public_inputs_json)?;
    let public_inputs: Vec<Fr> = inputs.iter()
        .map(|s| {
            use std::str::FromStr;
            Fr::from_str(s).unwrap_or(Fr::from(0u64))
        })
        .collect();

    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof)
        .map_err(|e| anyhow::anyhow!("verify failed: {}", e))?;

    Ok(valid)
}

fn extract_sensor_value(readings: &serde_json::Value, key: &str) -> Fr {
    readings
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|f| Fr::from((f * 1000.0) as u64))
        .unwrap_or(Fr::from(0u64))
}

fn write_key<T: CanonicalSerialize>(key: &T, path: std::path::PathBuf) -> Result<()> {
    let mut buf = Vec::new();
    key.serialize_uncompressed(&mut buf)?;
    std::fs::write(&path, &buf)?;
    Ok(())
}
