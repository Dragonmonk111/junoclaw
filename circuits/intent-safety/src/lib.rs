//! Intent-tier ZK circuit — proves an intent is legitimate without revealing
//! proprietary parameters.
//!
//! ### Statement
//!
//! Given public inputs:
//! - `intent_commitment`: H(action, params_hash, sensor_snapshot_hash, envelope_commitment, agent_id_hash)
//! - `merkle_root`: root of the reflex batch Merkle tree
//! - `envelope_commitment`: safety envelope commitment (links to reflex tier)
//! - `policy_commitment`: governance policy commitment
//!
//! And private witness:
//! - `action`: intent action type (field element encoding)
//! - `params_x`, `params_y`: intent parameters (e.g., destination coordinates)
//! - `sensor_snapshot_hash`: hash of sensor snapshot at intent time
//! - `agent_id`: agent identity (field element)
//! - `merkle_path`, `path_bits`: Merkle authentication path
//! - `envelope_params`: (max_speed, max_force, min_distance, max_tilt, max_accel)
//! - `policy_zone_x_min`, `policy_zone_x_max`, `policy_zone_y_min`, `policy_zone_y_max`: authorized zone
//!
//! The circuit proves:
//! 1. **Intent binding**: `intent_commitment == H(action, H(params), sensor_snapshot_hash, envelope_commitment, H(agent_id))`
//! 2. **Sensor consistency**: `sensor_snapshot_hash` is a leaf in the Merkle tree with root `merkle_root`
//! 3. **Envelope binding**: `H(envelope_params) == envelope_commitment` (links intent to reflex tier)
//! 4. **Policy compliance**: `params_x` and `params_y` are within the authorized zone
//! 5. **Policy binding**: `H(policy_zone) == policy_commitment`
//!
//! ### Why this matters
//!
//! The J-Lens gate can verify the ZK proof instead of seeing raw intent content.
//! This means a robot can prove "my intent to navigate to point (x,y) is within
//! the governance-authorized zone, and my sensor snapshot at decision time is
//! anchored in the reflex batch" — without revealing the actual destination,
//! the sensor data, or the agent identity.

use ark_bn254::Fr;
use ark_ff::{PrimeField, AdditiveGroup, BigInteger};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_r1cs_std::{
    alloc::AllocVar,
    eq::EqGadget,
    fields::fp::FpVar,
    boolean::Boolean,
    select::CondSelectGadget,
};
use ark_std::vec::Vec;
use sensor_safety_circuit::{
    mimc_hash, mimc_hash_5, envelope_commitment as compute_envelope_commitment,
    build_merkle_tree,
};

pub const DEFAULT_TREE_HEIGHT: usize = 20;
const MIMC_ROUNDS: usize = 91;

/// The intent consistency circuit.
#[derive(Clone)]
pub struct IntentConsistencyCircuit {
    // --- Public inputs ---
    pub intent_commitment: Option<Fr>,
    pub merkle_root: Option<Fr>,
    pub envelope_commitment: Option<Fr>,
    pub policy_commitment: Option<Fr>,

    // --- Private witness: intent data ---
    pub action: Option<Fr>,
    pub params_x: Option<Fr>,
    pub params_y: Option<Fr>,
    pub sensor_snapshot_hash: Option<Fr>,
    pub agent_id: Option<Fr>,

    // --- Private witness: Merkle proof ---
    pub merkle_path: Vec<Option<Fr>>,
    pub path_bits: Vec<Option<bool>>,

    // --- Private witness: envelope params ---
    pub max_speed: Option<Fr>,
    pub max_force: Option<Fr>,
    pub min_distance: Option<Fr>,
    pub max_tilt: Option<Fr>,
    pub max_accel: Option<Fr>,

    // --- Private witness: policy zone ---
    pub zone_x_min: Option<Fr>,
    pub zone_x_max: Option<Fr>,
    pub zone_y_min: Option<Fr>,
    pub zone_y_max: Option<Fr>,

    pub tree_height: usize,
}

impl IntentConsistencyCircuit {
    pub fn empty(tree_height: usize) -> Self {
        Self {
            intent_commitment: None,
            merkle_root: None,
            envelope_commitment: None,
            policy_commitment: None,
            action: None,
            params_x: None,
            params_y: None,
            sensor_snapshot_hash: None,
            agent_id: None,
            merkle_path: vec![None; tree_height],
            path_bits: vec![None; tree_height],
            max_speed: None,
            max_force: None,
            min_distance: None,
            max_tilt: None,
            max_accel: None,
            zone_x_min: None,
            zone_x_max: None,
            zone_y_min: None,
            zone_y_max: None,
            tree_height,
        }
    }

    pub fn new(
        intent_commitment: Fr,
        merkle_root: Fr,
        envelope_commitment: Fr,
        policy_commitment: Fr,
        action: Fr,
        params_x: Fr,
        params_y: Fr,
        sensor_snapshot_hash: Fr,
        agent_id: Fr,
        merkle_path: Vec<Fr>,
        path_bits: Vec<bool>,
        max_speed: Fr,
        max_force: Fr,
        min_distance: Fr,
        max_tilt: Fr,
        max_accel: Fr,
        zone_x_min: Fr,
        zone_x_max: Fr,
        zone_y_min: Fr,
        zone_y_max: Fr,
        tree_height: usize,
    ) -> Self {
        Self {
            intent_commitment: Some(intent_commitment),
            merkle_root: Some(merkle_root),
            envelope_commitment: Some(envelope_commitment),
            policy_commitment: Some(policy_commitment),
            action: Some(action),
            params_x: Some(params_x),
            params_y: Some(params_y),
            sensor_snapshot_hash: Some(sensor_snapshot_hash),
            agent_id: Some(agent_id),
            merkle_path: merkle_path.into_iter().map(Some).collect(),
            path_bits: path_bits.into_iter().map(Some).collect(),
            max_speed: Some(max_speed),
            max_force: Some(max_force),
            min_distance: Some(min_distance),
            max_tilt: Some(max_tilt),
            max_accel: Some(max_accel),
            zone_x_min: Some(zone_x_min),
            zone_x_max: Some(zone_x_max),
            zone_y_min: Some(zone_y_min),
            zone_y_max: Some(zone_y_max),
            tree_height,
        }
    }
}

// === Hash gadgets (on-circuit) ===

fn mimc_hash_gadget(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let round_constants = generate_round_constants();
    let mut state = left.clone() + right.clone();
    for i in 0..MIMC_ROUNDS {
        let rc = FpVar::new_constant(cs.clone(), round_constants[i])?;
        let t = &state + &rc;
        let t2 = &t * &t;
        let t4 = &t2 * &t2;
        state = &t4 * &t;
    }
    Ok(state)
}

fn mimc_hash_5_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: [&FpVar<Fr>; 5],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut acc = mimc_hash_gadget(cs.clone(), inputs[0], inputs[1])?;
    for i in 2..5 {
        acc = mimc_hash_gadget(cs.clone(), &acc, inputs[i])?;
    }
    Ok(acc)
}

fn mimc_hash_4_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: [&FpVar<Fr>; 4],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut acc = mimc_hash_gadget(cs.clone(), inputs[0], inputs[1])?;
    for i in 2..4 {
        acc = mimc_hash_gadget(cs.clone(), &acc, inputs[i])?;
    }
    Ok(acc)
}

fn generate_round_constants() -> Vec<Fr> {
    use sha2::{Digest, Sha256};
    (0..MIMC_ROUNDS)
        .map(|i| {
            let mut hasher = Sha256::new();
            hasher.update(format!("moultbook-mimc-bn254-round-{}", i).as_bytes());
            let hash = hasher.finalize();
            let mut bytes = [0u8; 32];
            bytes[1..].copy_from_slice(&hash[..31]);
            Fr::from_be_bytes_mod_order(&bytes)
        })
        .collect()
}

// === Range check (same as sensor-safety) ===

const RANGE_BITS: usize = 64;

fn enforce_leq(
    cs: ConstraintSystemRef<Fr>,
    a: &FpVar<Fr>,
    b: &FpVar<Fr>,
    a_val: Option<Fr>,
    b_val: Option<Fr>,
) -> Result<(), SynthesisError> {
    let diff = b.clone() - a.clone();

    let diff_val = match (a_val, b_val) {
        (Some(a), Some(b)) => b - a,
        _ => Fr::from(0u64),
    };

    let diff_bytes = diff_val.into_bigint().to_bytes_le();
    let mut buf = [0u8; 16];
    let copy_len = diff_bytes.len().min(16);
    buf[..copy_len].copy_from_slice(&diff_bytes[..copy_len]);
    let diff_int: u128 = u128::from_le_bytes(buf);

    let bits: Vec<Boolean<Fr>> = (0..RANGE_BITS)
        .map(|i| {
            let bit = (diff_int >> i) & 1 == 1;
            Boolean::new_witness(cs.clone(), || Ok(bit))
        })
        .collect::<Result<_, _>>()?;

    let zero = FpVar::new_constant(cs.clone(), Fr::from(0u64))?;
    let mut reconstructed = zero.clone();
    let mut power = Fr::from(1u64);
    for bit in &bits {
        let bit_fp = FpVar::from(bit.clone());
        let term = bit_fp * FpVar::new_constant(cs.clone(), power)?;
        reconstructed = reconstructed + term;
        power.double_in_place();
    }
    reconstructed.enforce_equal(&diff)?;

    Ok(())
}

impl ConstraintSynthesizer<Fr> for IntentConsistencyCircuit {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<Fr>,
    ) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        let intent_commitment_var = FpVar::new_input(cs.clone(), || {
            self.intent_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let merkle_root_var = FpVar::new_input(cs.clone(), || {
            self.merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let envelope_commitment_var = FpVar::new_input(cs.clone(), || {
            self.envelope_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let policy_commitment_var = FpVar::new_input(cs.clone(), || {
            self.policy_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: intent data ===
        let action_var = FpVar::new_witness(cs.clone(), || {
            self.action.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let params_x_var = FpVar::new_witness(cs.clone(), || {
            self.params_x.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let params_y_var = FpVar::new_witness(cs.clone(), || {
            self.params_y.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let sensor_snapshot_hash_var = FpVar::new_witness(cs.clone(), || {
            self.sensor_snapshot_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let agent_id_var = FpVar::new_witness(cs.clone(), || {
            self.agent_id.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: envelope params ===
        let max_speed_var = FpVar::new_witness(cs.clone(), || {
            self.max_speed.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let max_force_var = FpVar::new_witness(cs.clone(), || {
            self.max_force.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let min_distance_var = FpVar::new_witness(cs.clone(), || {
            self.min_distance.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let max_tilt_var = FpVar::new_witness(cs.clone(), || {
            self.max_tilt.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let max_accel_var = FpVar::new_witness(cs.clone(), || {
            self.max_accel.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: policy zone ===
        let zone_x_min_var = FpVar::new_witness(cs.clone(), || {
            self.zone_x_min.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let zone_x_max_var = FpVar::new_witness(cs.clone(), || {
            self.zone_x_max.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let zone_y_min_var = FpVar::new_witness(cs.clone(), || {
            self.zone_y_min.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let zone_y_max_var = FpVar::new_witness(cs.clone(), || {
            self.zone_y_max.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Constraint 1: Intent binding ===
        // intent_commitment = H(action, H(params_x, params_y), sensor_snapshot_hash,
        //                        envelope_commitment, H(agent_id))
        let params_hash = mimc_hash_gadget(cs.clone(), &params_x_var, &params_y_var)?;
        let agent_id_hash = mimc_hash_gadget(cs.clone(), &agent_id_var, &agent_id_var)?;

        let computed_intent_commitment = mimc_hash_5_gadget(
            cs.clone(),
            [
                &action_var,
                &params_hash,
                &sensor_snapshot_hash_var,
                &envelope_commitment_var,
                &agent_id_hash,
            ],
        )?;
        computed_intent_commitment.enforce_equal(&intent_commitment_var)?;

        // === Constraint 2: Envelope binding ===
        // H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment
        let computed_envelope = mimc_hash_5_gadget(
            cs.clone(),
            [
                &max_speed_var,
                &max_force_var,
                &min_distance_var,
                &max_tilt_var,
                &max_accel_var,
            ],
        )?;
        computed_envelope.enforce_equal(&envelope_commitment_var)?;

        // === Constraint 3: Policy compliance ===
        // zone_x_min <= params_x <= zone_x_max
        // zone_y_min <= params_y <= zone_y_max
        enforce_leq(cs.clone(), &zone_x_min_var, &params_x_var, self.zone_x_min, self.params_x)?;
        enforce_leq(cs.clone(), &params_x_var, &zone_x_max_var, self.params_x, self.zone_x_max)?;
        enforce_leq(cs.clone(), &zone_y_min_var, &params_y_var, self.zone_y_min, self.params_y)?;
        enforce_leq(cs.clone(), &params_y_var, &zone_y_max_var, self.params_y, self.zone_y_max)?;

        // === Constraint 4: Policy binding ===
        // H(zone_x_min, zone_x_max, zone_y_min, zone_y_max) == policy_commitment
        let computed_policy = mimc_hash_4_gadget(
            cs.clone(),
            [
                &zone_x_min_var,
                &zone_x_max_var,
                &zone_y_min_var,
                &zone_y_max_var,
            ],
        )?;
        computed_policy.enforce_equal(&policy_commitment_var)?;

        // === Constraint 5: Sensor consistency (Merkle membership) ===
        // sensor_snapshot_hash is a leaf in the Merkle tree with root merkle_root
        let mut path_vars = Vec::with_capacity(self.tree_height);
        let mut bit_vars = Vec::with_capacity(self.tree_height);

        for i in 0..self.tree_height {
            path_vars.push(FpVar::new_witness(cs.clone(), || {
                self.merkle_path[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
            bit_vars.push(Boolean::new_witness(cs.clone(), || {
                self.path_bits[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
        }

        // Walk the Merkle path
        let mut current = sensor_snapshot_hash_var;
        for i in 0..self.tree_height {
            let left = CondSelectGadget::conditionally_select(
                &bit_vars[i],
                &path_vars[i],
                &current,
            )?;
            let right = CondSelectGadget::conditionally_select(
                &bit_vars[i],
                &current,
                &path_vars[i],
            )?;
            current = mimc_hash_gadget(cs.clone(), &left, &right)?;
        }

        current.enforce_equal(&merkle_root_var)?;

        Ok(())
    }
}

// === Native (off-circuit) helpers ===

pub fn compute_intent_commitment(
    action: Fr,
    params_x: Fr,
    params_y: Fr,
    sensor_snapshot_hash: Fr,
    envelope_commitment: Fr,
    agent_id: Fr,
) -> Fr {
    let params_hash = mimc_hash(params_x, params_y);
    let agent_id_hash = mimc_hash(agent_id, agent_id);
    mimc_hash_5([
        action,
        params_hash,
        sensor_snapshot_hash,
        envelope_commitment,
        agent_id_hash,
    ])
}

pub fn compute_policy_commitment(
    zone_x_min: Fr,
    zone_x_max: Fr,
    zone_y_min: Fr,
    zone_y_max: Fr,
) -> Fr {
    let mut acc = mimc_hash(zone_x_min, zone_x_max);
    acc = mimc_hash(acc, zone_y_min);
    acc = mimc_hash(acc, zone_y_max);
    acc
}

/// Generate all proof data for an intent consistency proof.
pub fn generate_intent_proof_data(
    action: Fr,
    params_x: Fr,
    params_y: Fr,
    sensor_snapshot_hash: Fr,
    agent_id: Fr,
    max_speed: Fr,
    max_force: Fr,
    min_distance: Fr,
    max_tilt: Fr,
    max_accel: Fr,
    zone_x_min: Fr,
    zone_x_max: Fr,
    zone_y_min: Fr,
    zone_y_max: Fr,
    tree_height: usize,
) -> (
    Fr, // intent_commitment
    Fr, // merkle_root
    Fr, // envelope_commitment
    Fr, // policy_commitment
    Vec<Fr>,
    Vec<bool>,
) {
    let env_commit = compute_envelope_commitment(max_speed, max_force, min_distance, max_tilt, max_accel);
    let policy_commit = compute_policy_commitment(zone_x_min, zone_x_max, zone_y_min, zone_y_max);
    let intent_commit = compute_intent_commitment(
        action, params_x, params_y, sensor_snapshot_hash, env_commit, agent_id,
    );

    // Build Merkle tree with sensor_snapshot_hash as a leaf
    let leaves = vec![sensor_snapshot_hash];
    let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

    (intent_commit, merkle_root, env_commit, policy_commit, paths[0].clone(), bits[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn test_intent_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(42);

        let action = Fr::from(1u64); // e.g., 1 = "navigate"
        let params_x = Fr::from(500u64); // destination x
        let params_y = Fr::from(300u64); // destination y
        let sensor_snapshot_hash = Fr::from(12345u64);
        let agent_id = Fr::from(99u64);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // Authorized zone: x in [0, 1000], y in [0, 1000]
        let zone_x_min = Fr::from(0u64);
        let zone_x_max = Fr::from(1000u64);
        let zone_y_min = Fr::from(0u64);
        let zone_y_max = Fr::from(1000u64);

        let tree_height = 4;
        let (intent_commit, merkle_root, env_commit, policy_commit, path, bits) =
            generate_intent_proof_data(
                action, params_x, params_y, sensor_snapshot_hash, agent_id,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                zone_x_min, zone_x_max, zone_y_min, zone_y_max,
                tree_height,
            );

        let circuit = IntentConsistencyCircuit::new(
            intent_commit, merkle_root, env_commit, policy_commit,
            action, params_x, params_y, sensor_snapshot_hash, agent_id,
            path, bits,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            zone_x_min, zone_x_max, zone_y_min, zone_y_max,
            tree_height,
        );

        let empty = IntentConsistencyCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![intent_commit, merkle_root, env_commit, policy_commit];
        let valid = Groth16::<ark_bn254::Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "intent proof should verify");
    }

    #[test]
    fn test_intent_circuit_policy_violation_fails() {
        let rng = &mut StdRng::seed_from_u64(99);

        let action = Fr::from(1u64);
        let params_x = Fr::from(1500u64); // OUTSIDE zone (x > 1000)
        let params_y = Fr::from(300u64);
        let sensor_snapshot_hash = Fr::from(12345u64);
        let agent_id = Fr::from(99u64);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        let zone_x_min = Fr::from(0u64);
        let zone_x_max = Fr::from(1000u64);
        let zone_y_min = Fr::from(0u64);
        let zone_y_max = Fr::from(1000u64);

        let tree_height = 4;
        let (intent_commit, merkle_root, env_commit, policy_commit, path, bits) =
            generate_intent_proof_data(
                action, params_x, params_y, sensor_snapshot_hash, agent_id,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                zone_x_min, zone_x_max, zone_y_min, zone_y_max,
                tree_height,
            );

        let circuit = IntentConsistencyCircuit::new(
            intent_commit, merkle_root, env_commit, policy_commit,
            action, params_x, params_y, sensor_snapshot_hash, agent_id,
            path, bits,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            zone_x_min, zone_x_max, zone_y_min, zone_y_max,
            tree_height,
        );

        let empty = IntentConsistencyCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for policy violation");
    }

    #[test]
    fn test_intent_circuit_wrong_envelope_fails() {
        let rng = &mut StdRng::seed_from_u64(77);

        let action = Fr::from(1u64);
        let params_x = Fr::from(500u64);
        let params_y = Fr::from(300u64);
        let sensor_snapshot_hash = Fr::from(12345u64);
        let agent_id = Fr::from(99u64);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        let zone_x_min = Fr::from(0u64);
        let zone_x_max = Fr::from(1000u64);
        let zone_y_min = Fr::from(0u64);
        let zone_y_max = Fr::from(1000u64);

        let tree_height = 4;
        let (intent_commit, merkle_root, _env_commit, policy_commit, path, bits) =
            generate_intent_proof_data(
                action, params_x, params_y, sensor_snapshot_hash, agent_id,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                zone_x_min, zone_x_max, zone_y_min, zone_y_max,
                tree_height,
            );

        // Use WRONG envelope commitment
        let wrong_env = compute_envelope_commitment(
            Fr::from(9999u64), // different max_speed
            max_force, min_dist, max_tilt, max_accel,
        );

        let circuit = IntentConsistencyCircuit::new(
            intent_commit, merkle_root, wrong_env, policy_commit,
            action, params_x, params_y, sensor_snapshot_hash, agent_id,
            path, bits,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            zone_x_min, zone_x_max, zone_y_min, zone_y_max,
            tree_height,
        );

        let empty = IntentConsistencyCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong envelope");
    }

    #[test]
    fn test_intent_circuit_wrong_merkle_root_fails() {
        let rng = &mut StdRng::seed_from_u64(55);

        let action = Fr::from(1u64);
        let params_x = Fr::from(500u64);
        let params_y = Fr::from(300u64);
        let sensor_snapshot_hash = Fr::from(12345u64);
        let agent_id = Fr::from(99u64);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        let zone_x_min = Fr::from(0u64);
        let zone_x_max = Fr::from(1000u64);
        let zone_y_min = Fr::from(0u64);
        let zone_y_max = Fr::from(1000u64);

        let tree_height = 4;
        let (intent_commit, _merkle_root, env_commit, policy_commit, path, bits) =
            generate_intent_proof_data(
                action, params_x, params_y, sensor_snapshot_hash, agent_id,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                zone_x_min, zone_x_max, zone_y_min, zone_y_max,
                tree_height,
            );

        // Use WRONG merkle root
        let wrong_root = Fr::from(999999u64);

        let circuit = IntentConsistencyCircuit::new(
            intent_commit, wrong_root, env_commit, policy_commit,
            action, params_x, params_y, sensor_snapshot_hash, agent_id,
            path, bits,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            zone_x_min, zone_x_max, zone_y_min, zone_y_max,
            tree_height,
        );

        let empty = IntentConsistencyCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong merkle root");
    }

    #[test]
    fn test_compute_intent_commitment_deterministic() {
        let action = Fr::from(1u64);
        let params_x = Fr::from(500u64);
        let params_y = Fr::from(300u64);
        let sensor_hash = Fr::from(12345u64);
        let env = Fr::from(999u64);
        let agent = Fr::from(99u64);

        let c1 = compute_intent_commitment(action, params_x, params_y, sensor_hash, env, agent);
        let c2 = compute_intent_commitment(action, params_x, params_y, sensor_hash, env, agent);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_compute_policy_commitment_deterministic() {
        let p1 = compute_policy_commitment(Fr::from(0u64), Fr::from(1000u64), Fr::from(0u64), Fr::from(1000u64));
        let p2 = compute_policy_commitment(Fr::from(0u64), Fr::from(1000u64), Fr::from(0u64), Fr::from(1000u64));
        assert_eq!(p1, p2);
    }
}
