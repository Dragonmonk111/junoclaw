//! Batch safety circuit — proves all N cycles in a reflex batch satisfy
//! the safety envelope in a single Groth16 proof.
//!
//! Instead of verifying N individual SensorSafetyCircuit proofs on-chain,
//! this circuit folds N cycles into one proof. The prover iterates over
//! all N sensor readings, checks each against the envelope, and builds
//! the Merkle tree — all inside one constraint system.
//!
//! ### Statement
//!
//! Given public inputs:
//! - `envelope_commitment`: H(max_speed, max_force, min_distance, max_tilt, max_accel)
//! - `merkle_root`: root of the reflex batch Merkle tree
//! - `batch_size`: number of cycles in the batch
//!
//! And private witness:
//! - `sensor_readings`: [(speed, force, distance, tilt, accel); N]
//! - `envelope_params`: (max_speed, max_force, min_distance, max_tilt, max_accel)
//! - `merkle_paths`, `path_bits`: Merkle authentication paths for each cycle
//!
//! The circuit proves:
//! 1. All N sensor readings satisfy the envelope (range constraints)
//! 2. Each sensor leaf is correctly computed and included in the Merkle tree
//! 3. The envelope commitment matches the public input
//!
//! ### Constraint savings
//!
//! - N individual proofs: N × (5 range checks + 2 hash_5 + tree_height × hash_2)
//! - 1 batch proof: N × (5 range checks + N × hash_5 + tree_height × hash_2) + 1 hash_5
//! - On-chain: 1 verification instead of N (saves N-1 × ~203k gas)

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
    mimc_hash, mimc_hash_5, envelope_commitment, sensor_leaf, build_merkle_tree,
};

/// Maximum batch size (power of 2 for Merkle tree).
pub const MAX_BATCH_SIZE: usize = 8;
pub const BATCH_TREE_HEIGHT: usize = 3; // log2(8)

/// The batch safety circuit.
#[derive(Clone)]
pub struct BatchSafetyCircuit {
    // --- Public inputs ---
    pub envelope_commitment: Option<Fr>,
    pub merkle_root: Option<Fr>,
    pub batch_size: Option<Fr>,

    // --- Private witness: sensor readings for all cycles ---
    pub sensor_readings: Vec<(Fr, Fr, Fr, Fr, Fr)>, // (speed, force, distance, tilt, accel) × N

    // --- Private witness: envelope params ---
    pub max_speed: Option<Fr>,
    pub max_force: Option<Fr>,
    pub min_distance: Option<Fr>,
    pub max_tilt: Option<Fr>,
    pub max_accel: Option<Fr>,

    // --- Private witness: Merkle paths ---
    pub merkle_paths: Vec<Vec<Fr>>,  // [N][tree_height]
    pub path_bits: Vec<Vec<bool>>,   // [N][tree_height]

    pub tree_height: usize,
}

impl BatchSafetyCircuit {
    pub fn empty(tree_height: usize) -> Self {
        let n = 1 << tree_height;
        Self {
            envelope_commitment: None,
            merkle_root: None,
            batch_size: None,
            sensor_readings: (0..n).map(|_| (Fr::from(0u64), Fr::from(0u64), Fr::from(0u64), Fr::from(0u64), Fr::from(0u64))).collect(),
            max_speed: None,
            max_force: None,
            min_distance: None,
            max_tilt: None,
            max_accel: None,
            merkle_paths: vec![vec![Fr::from(0u64); tree_height]; n],
            path_bits: vec![vec![false; tree_height]; n],
            tree_height,
        }
    }

    pub fn new(
        envelope_commitment: Fr,
        merkle_root: Fr,
        batch_size: Fr,
        sensor_readings: Vec<(Fr, Fr, Fr, Fr, Fr)>,
        max_speed: Fr,
        max_force: Fr,
        min_distance: Fr,
        max_tilt: Fr,
        max_accel: Fr,
        merkle_paths: Vec<Vec<Fr>>,
        path_bits: Vec<Vec<bool>>,
        tree_height: usize,
    ) -> Self {
        Self {
            envelope_commitment: Some(envelope_commitment),
            merkle_root: Some(merkle_root),
            batch_size: Some(batch_size),
            sensor_readings,
            max_speed: Some(max_speed),
            max_force: Some(max_force),
            min_distance: Some(min_distance),
            max_tilt: Some(max_tilt),
            max_accel: Some(max_accel),
            merkle_paths,
            path_bits,
            tree_height,
        }
    }
}

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

// Re-use the hash gadget from sensor-safety-circuit by re-implementing locally.
// In production, these would be shared via a common crate.

fn hash_2_gadget(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    // MiMC hash gadget (same as sensor-safety-circuit)
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

fn hash_5_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: [&FpVar<Fr>; 5],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut acc = hash_2_gadget(cs.clone(), inputs[0], inputs[1])?;
    for i in 2..5 {
        acc = hash_2_gadget(cs.clone(), &acc, inputs[i])?;
    }
    Ok(acc)
}

const MIMC_ROUNDS: usize = 91;

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

impl ConstraintSynthesizer<Fr> for BatchSafetyCircuit {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<Fr>,
    ) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        let envelope_commitment_var = FpVar::new_input(cs.clone(), || {
            self.envelope_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let merkle_root_var = FpVar::new_input(cs.clone(), || {
            self.merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let _batch_size_var = FpVar::new_input(cs.clone(), || {
            self.batch_size.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate envelope params (private) ===
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

        // === Constraint 1: Envelope binding ===
        let computed_commitment = hash_5_gadget(
            cs.clone(),
            [
                &max_speed_var,
                &max_force_var,
                &min_distance_var,
                &max_tilt_var,
                &max_accel_var,
            ],
        )?;
        computed_commitment.enforce_equal(&envelope_commitment_var)?;

        // === Constraint 2: For each cycle, range check + compute leaf ===
        let n = self.sensor_readings.len();
        let mut leaf_vars = Vec::with_capacity(n);

        for i in 0..n {
            let (speed, force, distance, tilt, accel) = self.sensor_readings[i];

            let speed_var = FpVar::new_witness(cs.clone(), || Ok(speed))?;
            let force_var = FpVar::new_witness(cs.clone(), || Ok(force))?;
            let distance_var = FpVar::new_witness(cs.clone(), || Ok(distance))?;
            let tilt_var = FpVar::new_witness(cs.clone(), || Ok(tilt))?;
            let accel_var = FpVar::new_witness(cs.clone(), || Ok(accel))?;

            // Range checks against envelope
            enforce_leq(cs.clone(), &speed_var, &max_speed_var, Some(speed), self.max_speed)?;
            enforce_leq(cs.clone(), &force_var, &max_force_var, Some(force), self.max_force)?;
            enforce_leq(cs.clone(), &min_distance_var, &distance_var, self.min_distance, Some(distance))?;
            enforce_leq(cs.clone(), &tilt_var, &max_tilt_var, Some(tilt), self.max_tilt)?;
            enforce_leq(cs.clone(), &accel_var, &max_accel_var, Some(accel), self.max_accel)?;

            // Compute leaf = H(speed, force, distance, tilt, accel)
            let leaf = hash_5_gadget(
                cs.clone(),
                [&speed_var, &force_var, &distance_var, &tilt_var, &accel_var],
            )?;
            leaf_vars.push(leaf);
        }

        // === Constraint 3: Merkle tree construction from leaves ===
        // We build the tree level by level, checking that each cycle's
        // Merkle path leads to the public root.

        for cycle_idx in 0..n {
            let mut path_vars = Vec::with_capacity(self.tree_height);
            let mut bit_vars = Vec::with_capacity(self.tree_height);

            for j in 0..self.tree_height {
                path_vars.push(FpVar::new_witness(cs.clone(), || {
                    Ok(self.merkle_paths[cycle_idx][j])
                })?);
                bit_vars.push(Boolean::new_witness(cs.clone(), || {
                    Ok(self.path_bits[cycle_idx][j])
                })?);
            }

            // Walk the Merkle path for this cycle
            let mut current = leaf_vars[cycle_idx].clone();
            for j in 0..self.tree_height {
                let left = CondSelectGadget::conditionally_select(
                    &bit_vars[j],
                    &path_vars[j],
                    &current,
                )?;
                let right = CondSelectGadget::conditionally_select(
                    &bit_vars[j],
                    &current,
                    &path_vars[j],
                )?;
                current = hash_2_gadget(cs.clone(), &left, &right)?;
            }

            // Each cycle's path must lead to the same root
            current.enforce_equal(&merkle_root_var)?;
        }

        Ok(())
    }
}

// === Native helpers ===

/// Generate a batch safety proof for N cycles.
pub fn generate_batch_proof_data(
    sensor_readings: &[(Fr, Fr, Fr, Fr, Fr)],
    max_speed: Fr,
    max_force: Fr,
    min_distance: Fr,
    max_tilt: Fr,
    max_accel: Fr,
    tree_height: usize,
) -> (Fr, Fr, Fr, Vec<Vec<Fr>>, Vec<Vec<bool>>) {
    let env_commit = envelope_commitment(max_speed, max_force, min_distance, max_tilt, max_accel);

    // Compute leaves
    let leaves: Vec<Fr> = sensor_readings
        .iter()
        .map(|&(s, f, d, t, a)| sensor_leaf(s, f, d, t, a))
        .collect();

    // Build Merkle tree
    let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);
    let batch_size = Fr::from(sensor_readings.len() as u64);

    (env_commit, merkle_root, batch_size, paths, bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn test_batch_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(123);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // 4 cycles, all within envelope
        let readings = vec![
            (Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64)),
            (Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
            (Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            (Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];

        let tree_height = 2; // 4 leaves
        let (env_commit, merkle_root, batch_size, paths, bits) = generate_batch_proof_data(
            &readings, max_speed, max_force, min_dist, max_tilt, max_accel, tree_height,
        );

        let circuit = BatchSafetyCircuit::new(
            env_commit, merkle_root, batch_size,
            readings,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            paths, bits, tree_height,
        );

        let empty = BatchSafetyCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![env_commit, merkle_root, batch_size];
        let valid = Groth16::<ark_bn254::Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "batch proof should verify");
    }

    #[test]
    fn test_batch_circuit_violation_fails() {
        let rng = &mut StdRng::seed_from_u64(456);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // 4 cycles, one VIOLATES (speed 6000 > 5000)
        let readings = vec![
            (Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64)),
            (Fr::from(6000u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)), // violation!
            (Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            (Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];

        let tree_height = 2;
        let (env_commit, merkle_root, batch_size, paths, bits) = generate_batch_proof_data(
            &readings, max_speed, max_force, min_dist, max_tilt, max_accel, tree_height,
        );

        let circuit = BatchSafetyCircuit::new(
            env_commit, merkle_root, batch_size,
            readings,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            paths, bits, tree_height,
        );

        let empty = BatchSafetyCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        // Proof generation should fail (constraint violation)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof generation should fail for violating batch");
    }

    #[test]
    fn test_batch_circuit_wrong_envelope_fails() {
        let rng = &mut StdRng::seed_from_u64(789);

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        let readings = vec![
            (Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64)),
            (Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
            (Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            (Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];

        let tree_height = 2;
        let (env_commit, merkle_root, batch_size, paths, bits) = generate_batch_proof_data(
            &readings, max_speed, max_force, min_dist, max_tilt, max_accel, tree_height,
        );

        // Use WRONG envelope commitment (different params)
        let wrong_commit = envelope_commitment(
            Fr::from(4000u64), // different max_speed
            max_force, min_dist, max_tilt, max_accel,
        );

        let circuit = BatchSafetyCircuit::new(
            wrong_commit, merkle_root, batch_size, // wrong commitment
            readings,
            max_speed, max_force, min_dist, max_tilt, max_accel,
            paths, bits, tree_height,
        );

        let empty = BatchSafetyCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof generation should fail for wrong envelope");
    }
}
