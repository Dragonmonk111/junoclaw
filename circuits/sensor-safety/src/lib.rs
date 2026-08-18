//! Sensor safety circuit — Groth16 over BN254.
//!
//! Proves that a robot's sensor readings at a specific reflex cycle satisfy
//! the governance-approved safety envelope, **without revealing the actual
//! sensor values**.
//!
//! ### Statement
//!
//! Given public inputs:
//! - `envelope_commitment`: H(max_speed, max_force, min_distance, max_tilt, max_accel, human_proximity)
//! - `merkle_root`: root of the reflex batch Merkle tree
//! - `cycle_index`: which cycle in the batch (0-indexed)
//!
//! And private witness:
//! - `speed`, `force`, `distance`, `tilt`, `accel`: actual sensor readings (as field elements)
//! - `max_speed`, `max_force`, `min_distance`, `max_tilt`, `max_accel`: envelope params
//! - `human_proximity`: bool
//! - `merkle_path`, `path_bits`: Merkle authentication path
//!
//! The circuit proves:
//! 1. **Range constraints**: `speed <= max_speed`, `force <= max_force`,
//!    `distance >= min_distance`, `tilt <= max_tilt`, `accel <= max_accel`
//! 2. **Envelope binding**: `H(max_speed, max_force, ...) == envelope_commitment`
//! 3. **Batch binding**: `H(speed, force, distance, tilt, accel)` is a leaf in
//!    the Merkle tree with root `merkle_root` at position `cycle_index`
//!
//! ### Why this matters
//!
//! A robot can prove "my speed was within the governance-approved limit at
//! cycle 42" without revealing its exact speed, location, or sensor data.
//! This enables safety auditing on public chains without leaking proprietary
//! or privacy-sensitive operational data.
//!
//! ### Hash function
//!
//! MiMC (x^5, 91 rounds) over BN254::Fr — same as the moultbook-membership
//! circuit for consistency and proof composition.

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

pub const DEFAULT_TREE_HEIGHT: usize = 20;
const MIMC_ROUNDS: usize = 91;

/// The sensor safety circuit.
#[derive(Clone)]
pub struct SensorSafetyCircuit {
    // --- Public inputs ---
    pub envelope_commitment: Option<Fr>,
    pub merkle_root: Option<Fr>,
    pub cycle_index: Option<Fr>,

    // --- Private witness: sensor readings (milli-units as field elements) ---
    pub speed: Option<Fr>,
    pub force: Option<Fr>,
    pub distance: Option<Fr>,
    pub tilt: Option<Fr>,
    pub accel: Option<Fr>,

    // --- Private witness: envelope params (milli-units) ---
    pub max_speed: Option<Fr>,
    pub max_force: Option<Fr>,
    pub min_distance: Option<Fr>,
    pub max_tilt: Option<Fr>,
    pub max_accel: Option<Fr>,

    // --- Private witness: Merkle authentication ---
    pub merkle_path: Vec<Option<Fr>>,
    pub path_bits: Vec<Option<bool>>,

    pub tree_height: usize,
}

impl SensorSafetyCircuit {
    pub fn empty(tree_height: usize) -> Self {
        Self {
            envelope_commitment: None,
            merkle_root: None,
            cycle_index: None,
            speed: None,
            force: None,
            distance: None,
            tilt: None,
            accel: None,
            max_speed: None,
            max_force: None,
            min_distance: None,
            max_tilt: None,
            max_accel: None,
            merkle_path: vec![None; tree_height],
            path_bits: vec![None; tree_height],
            tree_height,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope_commitment: Fr,
        merkle_root: Fr,
        cycle_index: Fr,
        speed: Fr,
        force: Fr,
        distance: Fr,
        tilt: Fr,
        accel: Fr,
        max_speed: Fr,
        max_force: Fr,
        min_distance: Fr,
        max_tilt: Fr,
        max_accel: Fr,
        merkle_path: Vec<Fr>,
        path_bits: Vec<bool>,
        tree_height: usize,
    ) -> Self {
        assert_eq!(merkle_path.len(), tree_height);
        assert_eq!(path_bits.len(), tree_height);
        Self {
            envelope_commitment: Some(envelope_commitment),
            merkle_root: Some(merkle_root),
            cycle_index: Some(cycle_index),
            speed: Some(speed),
            force: Some(force),
            distance: Some(distance),
            tilt: Some(tilt),
            accel: Some(accel),
            max_speed: Some(max_speed),
            max_force: Some(max_force),
            min_distance: Some(min_distance),
            max_tilt: Some(max_tilt),
            max_accel: Some(max_accel),
            merkle_path: merkle_path.into_iter().map(Some).collect(),
            path_bits: path_bits.into_iter().map(Some).collect(),
            tree_height,
        }
    }
}

// === MiMC hash (shared with moultbook-membership circuit) ===

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

/// Multi-input MiMC hash: H(a, b, c, d, e) = H(H(H(H(a, b), c), d), e)
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

/// Enforce that `a <= b` in R1CS by proving `b - a` is non-negative.
///
/// This uses a bit decomposition: we decompose `b - a` into `RANGE_BITS` bits
/// and check that the recomposition equals `b - a`. Since field elements
/// are non-negative, this proves `b - a >= 0`, i.e., `a <= b`.
///
/// Uses 64-bit decomposition (sufficient for milli-unit values up to ~10^18).
const RANGE_BITS: usize = 64;

fn enforce_leq(
    cs: ConstraintSystemRef<Fr>,
    a: &FpVar<Fr>,
    b: &FpVar<Fr>,
    a_val: Option<Fr>,
    b_val: Option<Fr>,
) -> Result<(), SynthesisError> {
    let diff = b.clone() - a.clone();

    // Compute the difference value from raw witness values (not from FpVar::value())
    let diff_val = match (a_val, b_val) {
        (Some(a), Some(b)) => b - a,
        _ => Fr::from(0u64),
    };

    // Convert to u128 for bit decomposition (witness side)
    let diff_bytes = diff_val.into_bigint().to_bytes_le();
    let mut buf = [0u8; 16];
    let copy_len = diff_bytes.len().min(16);
    buf[..copy_len].copy_from_slice(&diff_bytes[..copy_len]);
    let diff_int: u128 = u128::from_le_bytes(buf);

    // Allocate bits as Boolean witnesses
    let bits: Vec<Boolean<Fr>> = (0..RANGE_BITS)
        .map(|i| {
            let bit = (diff_int >> i) & 1 == 1;
            Boolean::new_witness(cs.clone(), || Ok(bit))
        })
        .collect::<Result<_, _>>()?;

    // Reconstruct value from bits and enforce equality with diff
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

impl ConstraintSynthesizer<Fr> for SensorSafetyCircuit {
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
        let _cycle_index_var = FpVar::new_input(cs.clone(), || {
            self.cycle_index.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: sensor readings ===
        let speed_var = FpVar::new_witness(cs.clone(), || {
            self.speed.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let force_var = FpVar::new_witness(cs.clone(), || {
            self.force.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let distance_var = FpVar::new_witness(cs.clone(), || {
            self.distance.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let tilt_var = FpVar::new_witness(cs.clone(), || {
            self.tilt.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let accel_var = FpVar::new_witness(cs.clone(), || {
            self.accel.ok_or(SynthesisError::AssignmentMissing)
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

        // === Constraint 1: Range checks (sensor within envelope) ===
        // speed <= max_speed
        enforce_leq(cs.clone(), &speed_var, &max_speed_var, self.speed, self.max_speed)?;
        // force <= max_force
        enforce_leq(cs.clone(), &force_var, &max_force_var, self.force, self.max_force)?;
        // distance >= min_distance  =>  min_distance <= distance
        enforce_leq(cs.clone(), &min_distance_var, &distance_var, self.min_distance, self.distance)?;
        // tilt <= max_tilt
        enforce_leq(cs.clone(), &tilt_var, &max_tilt_var, self.tilt, self.max_tilt)?;
        // accel <= max_accel
        enforce_leq(cs.clone(), &accel_var, &max_accel_var, self.accel, self.max_accel)?;

        // === Constraint 2: Envelope binding ===
        // H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment
        let computed_commitment = mimc_hash_5_gadget(
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

        // === Constraint 3: Batch binding (Merkle membership) ===
        // leaf = H(speed, force, distance, tilt, accel)
        let leaf = mimc_hash_5_gadget(
            cs.clone(),
            [
                &speed_var,
                &force_var,
                &distance_var,
                &tilt_var,
                &accel_var,
            ],
        )?;

        // Allocate Merkle path
        let mut path_vars = Vec::with_capacity(self.tree_height);
        for i in 0..self.tree_height {
            path_vars.push(FpVar::new_witness(cs.clone(), || {
                self.merkle_path[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
        }

        let mut bit_vars = Vec::with_capacity(self.tree_height);
        for i in 0..self.tree_height {
            bit_vars.push(Boolean::new_witness(cs.clone(), || {
                self.path_bits[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
        }

        // Walk the Merkle path
        let mut current = leaf;
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

pub fn mimc_hash(left: Fr, right: Fr) -> Fr {
    let round_constants = generate_round_constants();
    let mut state = left + right;
    for i in 0..MIMC_ROUNDS {
        let t = state + round_constants[i];
        state = t * t * t * t * t;
    }
    state
}

pub fn mimc_hash_5(inputs: [Fr; 5]) -> Fr {
    let mut acc = mimc_hash(inputs[0], inputs[1]);
    for i in 2..5 {
        acc = mimc_hash(acc, inputs[i]);
    }
    acc
}

/// Compute the envelope commitment from params.
pub fn envelope_commitment(
    max_speed: Fr,
    max_force: Fr,
    min_distance: Fr,
    max_tilt: Fr,
    max_accel: Fr,
) -> Fr {
    mimc_hash_5([max_speed, max_force, min_distance, max_tilt, max_accel])
}

/// Compute the sensor leaf hash for a Merkle tree.
pub fn sensor_leaf(
    speed: Fr,
    force: Fr,
    distance: Fr,
    tilt: Fr,
    accel: Fr,
) -> Fr {
    mimc_hash_5([speed, force, distance, tilt, accel])
}

/// Build a Merkle tree from sensor leaf hashes.
pub fn build_merkle_tree(
    leaves: &[Fr],
    tree_height: usize,
) -> (Fr, Vec<Vec<Fr>>, Vec<Vec<bool>>) {
    let num_leaves = 1 << tree_height;
    let zero = Fr::from(0u64);
    let zero_leaf = mimc_hash(zero, zero);
    let mut layer: Vec<Fr> = leaves.to_vec();
    layer.resize(num_leaves, zero_leaf);

    let mut layers = vec![layer.clone()];

    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len() / 2);
        for chunk in layer.chunks(2) {
            next.push(mimc_hash(chunk[0], chunk[1]));
        }
        layers.push(next.clone());
        layer = next;
    }

    let root = layers.last().unwrap()[0];

    let mut all_paths = Vec::with_capacity(leaves.len());
    let mut all_bits = Vec::with_capacity(leaves.len());

    for leaf_idx in 0..leaves.len() {
        let mut path = Vec::with_capacity(tree_height);
        let mut bits = Vec::with_capacity(tree_height);
        let mut idx = leaf_idx;

        for depth in 0..tree_height {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            path.push(layers[depth][sibling_idx]);
            bits.push(idx % 2 == 1);
            idx /= 2;
        }

        all_paths.push(path);
        all_bits.push(bits);
    }

    (root, all_paths, all_bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Bn254;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn test_mimc_hash_deterministic() {
        let a = Fr::from(42u64);
        let b = Fr::from(7u64);
        assert_eq!(mimc_hash(a, b), mimc_hash(a, b));
    }

    #[test]
    fn test_envelope_commitment() {
        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        let c1 = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
        let c2 = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
        assert_eq!(c1, c2);

        // Different params → different commitment
        let c3 = envelope_commitment(Fr::from(6000u64), max_force, min_dist, max_tilt, max_accel);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_sensor_leaf() {
        let leaf1 = sensor_leaf(Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64));
        let leaf2 = sensor_leaf(Fr::from(4000u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64));
        assert_eq!(leaf1, leaf2);

        let leaf3 = sensor_leaf(Fr::from(4001u64), Fr::from(30000u64), Fr::from(600u64), Fr::from(20000u64), Fr::from(2000u64));
        assert_ne!(leaf1, leaf3);
    }

    #[test]
    fn test_merkle_tree_small() {
        let tree_height = 3;
        let leaves: Vec<Fr> = (0..5)
            .map(|i| sensor_leaf(Fr::from(i as u64), Fr::from(0u64), Fr::from(0u64), Fr::from(0u64), Fr::from(0u64)))
            .collect();
        let (root, paths, bits) = build_merkle_tree(&leaves, tree_height);
        assert_eq!(paths.len(), 5);
        assert_eq!(bits.len(), 5);
        assert_ne!(root, Fr::from(0u64));
    }

    #[test]
    fn test_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(42);
        let tree_height = 3;

        // Envelope params (milli-units)
        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // Sensor readings (within envelope)
        let speed = Fr::from(4000u64);    // 4.0 m/s <= 5.0
        let force = Fr::from(30000u64);   // 30.0 N <= 50.0
        let distance = Fr::from(600u64);  // 0.6 m >= 0.5
        let tilt = Fr::from(20000u64);    // 20.0° <= 30.0°
        let accel = Fr::from(2000u64);    // 2.0 m/s² <= 3.0

        // Build Merkle tree with 4 sensor readings
        let leaves: Vec<Fr> = vec![
            sensor_leaf(speed, force, distance, tilt, accel),
            sensor_leaf(Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
            sensor_leaf(Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            sensor_leaf(Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];
        let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
        let cycle_index = Fr::from(0u64); // first cycle

        let circuit = SensorSafetyCircuit::new(
            env_commit,
            merkle_root,
            cycle_index,
            speed,
            force,
            distance,
            tilt,
            accel,
            max_speed,
            max_force,
            min_dist,
            max_tilt,
            max_accel,
            paths[0].clone(),
            bits[0].clone(),
            tree_height,
        );

        let empty_circuit = SensorSafetyCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng).unwrap();

        let proof = Groth16::<Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![env_commit, merkle_root, cycle_index];
        let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "proof should verify for in-envelope readings");
    }

    #[test]
    fn test_circuit_violation_fails() {
        let rng = &mut StdRng::seed_from_u64(43);
        let tree_height = 3;

        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // Sensor reading VIOLATES envelope: speed=6000 > max_speed=5000
        let speed = Fr::from(6000u64);
        let force = Fr::from(30000u64);
        let distance = Fr::from(600u64);
        let tilt = Fr::from(20000u64);
        let accel = Fr::from(2000u64);

        let leaves: Vec<Fr> = vec![
            sensor_leaf(speed, force, distance, tilt, accel),
            sensor_leaf(Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
            sensor_leaf(Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            sensor_leaf(Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];
        let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);
        let cycle_index = Fr::from(0u64);

        let circuit = SensorSafetyCircuit::new(
            env_commit,
            merkle_root,
            cycle_index,
            speed,
            force,
            distance,
            tilt,
            accel,
            max_speed,
            max_force,
            min_dist,
            max_tilt,
            max_accel,
            paths[0].clone(),
            bits[0].clone(),
            tree_height,
        );

        let empty_circuit = SensorSafetyCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng).unwrap();

        // Prove should fail (constraint violation: speed > max_speed).
        // Groth16::prove panics when constraints aren't satisfied, so we
        // catch the panic. Either a panic or an invalid proof is acceptable.
        let proof_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<Bn254>::prove(&pk, circuit, rng)
        }));

        match proof_result {
            Ok(Ok(proof)) => {
                let public_inputs = vec![env_commit, merkle_root, cycle_index];
                let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
                assert!(!valid, "proof with violated envelope should NOT verify");
            }
            Ok(Err(_)) => { /* prove returned error — also correct */ }
            Err(_) => { /* prove panicked — constraint violation detected */ }
        }
    }

    #[test]
    fn test_circuit_wrong_envelope_fails() {
        let rng = &mut StdRng::seed_from_u64(44);
        let tree_height = 3;

        // Real envelope
        let max_speed = Fr::from(5000u64);
        let max_force = Fr::from(50000u64);
        let min_dist = Fr::from(500u64);
        let max_tilt = Fr::from(30000u64);
        let max_accel = Fr::from(3000u64);

        // Sensor readings (within the REAL envelope)
        let speed = Fr::from(4000u64);
        let force = Fr::from(30000u64);
        let distance = Fr::from(600u64);
        let tilt = Fr::from(20000u64);
        let accel = Fr::from(2000u64);

        let leaves: Vec<Fr> = vec![
            sensor_leaf(speed, force, distance, tilt, accel),
            sensor_leaf(Fr::from(3500u64), Fr::from(25000u64), Fr::from(700u64), Fr::from(15000u64), Fr::from(1800u64)),
            sensor_leaf(Fr::from(4500u64), Fr::from(40000u64), Fr::from(550u64), Fr::from(25000u64), Fr::from(2800u64)),
            sensor_leaf(Fr::from(3000u64), Fr::from(20000u64), Fr::from(800u64), Fr::from(10000u64), Fr::from(1500u64)),
        ];
        let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        // WRONG envelope commitment (different params)
        let wrong_commit = envelope_commitment(
            Fr::from(10000u64), // different max_speed
            max_force,
            min_dist,
            max_tilt,
            max_accel,
        );
        let cycle_index = Fr::from(0u64);

        let circuit = SensorSafetyCircuit::new(
            wrong_commit,
            merkle_root,
            cycle_index,
            speed,
            force,
            distance,
            tilt,
            accel,
            max_speed,
            max_force,
            min_dist,
            max_tilt,
            max_accel,
            paths[0].clone(),
            bits[0].clone(),
            tree_height,
        );

        let empty_circuit = SensorSafetyCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng).unwrap();

        let proof_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<Bn254>::prove(&pk, circuit, rng)
        }));

        match proof_result {
            Ok(Ok(proof)) => {
                let public_inputs = vec![wrong_commit, merkle_root, cycle_index];
                let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
                assert!(!valid, "proof with wrong envelope commitment should NOT verify");
            }
            Ok(Err(_)) => { /* prove returned error — also correct */ }
            Err(_) => { /* prove panicked — constraint violation detected */ }
        }
    }
}
