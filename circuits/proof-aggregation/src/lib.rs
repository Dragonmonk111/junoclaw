//! Proof aggregation circuit (Plan D) — proves cross-tier consistency across
//! sensor safety, intent consistency, and consensus membership proofs.
//!
//! ### Architecture
//!
//! Three Groth16 proofs are generated independently on BN254:
//! 1. **Sensor safety proof** — public inputs: [envelope_commitment, merkle_root, cycle_index]
//! 2. **Intent consistency proof** — public inputs: [intent_commitment, merkle_root, envelope_commitment, policy_commitment]
//! 3. **Consensus membership proof** — public inputs: [validator_set_root, epoch, vote_commitment]
//!
//! The TEE verifies all three Groth16 pairings and binds "all proofs valid" to
//! `aggregation_commitment` via attestation report data.
//!
//! This circuit proves:
//! 1. **Proof knowledge**: the prover knows the public inputs of all three proofs
//! 2. **Cross-tier consistency**:
//!    - `envelope_commitment` matches between sensor and intent proofs
//!    - `merkle_root` matches between sensor and intent proofs
//! 3. **Aggregation binding**: `aggregation_commitment == H(all public inputs)`
//!
//! On-chain verification:
//! 1. `zk-verifier.VerifyProof(aggregation_proof)` — verifies this circuit (128 bytes)
//! 2. `tee-attestation-verifier.VerifyAttestation(report)` — verifies TEE attested
//!    to the same `aggregation_commitment` and all three pairings passed
//!
//! ### Why this is novel
//!
//! Traditional recursive SNARKs verify proofs cryptographically inside a circuit
//! (expensive pairing checks in R1CS). This pattern splits the job:
//! - **ZK circuit** proves knowledge + consistency (cheap, ~8K constraints)
//! - **TEE** proves cryptographic validity (free, hardware-attested)
//!
//! No recursion, no Grumpkin, no pairing in R1CS. 128-byte proof + TEE attestation.

use ark_bn254::Fr;
use ark_ff::PrimeField;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_r1cs_std::{
    alloc::AllocVar,
    eq::EqGadget,
    fields::fp::FpVar,
};
use ark_std::vec::Vec;
use sensor_safety_circuit::mimc_hash;

const MIMC_ROUNDS: usize = 91;

/// The aggregation circuit.
///
/// Public inputs:
/// - `aggregation_commitment`: H(sensor_pi || intent_pi || consensus_pi)
/// - `cross_tier_envelope`: envelope commitment (must match sensor + intent)
/// - `cross_tier_merkle_root`: Merkle root (must match sensor + intent)
///
/// Private witness:
/// - Sensor proof public inputs: [envelope_commitment, merkle_root, cycle_index]
/// - Intent proof public inputs: [intent_commitment, merkle_root, envelope_commitment, policy_commitment]
/// - Consensus proof public inputs: [validator_set_root, epoch, vote_commitment]
#[derive(Clone)]
pub struct AggregationCircuit {
    // --- Public inputs ---
    pub aggregation_commitment: Option<Fr>,
    pub cross_tier_envelope: Option<Fr>,
    pub cross_tier_merkle_root: Option<Fr>,

    // --- Private witness: sensor proof public inputs ---
    pub sensor_envelope_commitment: Option<Fr>,
    pub sensor_merkle_root: Option<Fr>,
    pub sensor_cycle_index: Option<Fr>,

    // --- Private witness: intent proof public inputs ---
    pub intent_commitment: Option<Fr>,
    pub intent_merkle_root: Option<Fr>,
    pub intent_envelope_commitment: Option<Fr>,
    pub intent_policy_commitment: Option<Fr>,

    // --- Private witness: consensus proof public inputs ---
    pub consensus_validator_set_root: Option<Fr>,
    pub consensus_epoch: Option<Fr>,
    pub consensus_vote_commitment: Option<Fr>,
}

impl AggregationCircuit {
    pub fn empty() -> Self {
        Self {
            aggregation_commitment: None,
            cross_tier_envelope: None,
            cross_tier_merkle_root: None,
            sensor_envelope_commitment: None,
            sensor_merkle_root: None,
            sensor_cycle_index: None,
            intent_commitment: None,
            intent_merkle_root: None,
            intent_envelope_commitment: None,
            intent_policy_commitment: None,
            consensus_validator_set_root: None,
            consensus_epoch: None,
            consensus_vote_commitment: None,
        }
    }

    pub fn new(
        aggregation_commitment: Fr,
        cross_tier_envelope: Fr,
        cross_tier_merkle_root: Fr,
        sensor_envelope_commitment: Fr,
        sensor_merkle_root: Fr,
        sensor_cycle_index: Fr,
        intent_commitment: Fr,
        intent_merkle_root: Fr,
        intent_envelope_commitment: Fr,
        intent_policy_commitment: Fr,
        consensus_validator_set_root: Fr,
        consensus_epoch: Fr,
        consensus_vote_commitment: Fr,
    ) -> Self {
        Self {
            aggregation_commitment: Some(aggregation_commitment),
            cross_tier_envelope: Some(cross_tier_envelope),
            cross_tier_merkle_root: Some(cross_tier_merkle_root),
            sensor_envelope_commitment: Some(sensor_envelope_commitment),
            sensor_merkle_root: Some(sensor_merkle_root),
            sensor_cycle_index: Some(sensor_cycle_index),
            intent_commitment: Some(intent_commitment),
            intent_merkle_root: Some(intent_merkle_root),
            intent_envelope_commitment: Some(intent_envelope_commitment),
            intent_policy_commitment: Some(intent_policy_commitment),
            consensus_validator_set_root: Some(consensus_validator_set_root),
            consensus_epoch: Some(consensus_epoch),
            consensus_vote_commitment: Some(consensus_vote_commitment),
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

fn mimc_hash_many_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: &[&FpVar<Fr>],
) -> Result<FpVar<Fr>, SynthesisError> {
    assert!(!inputs.is_empty(), "mimc_hash_many requires at least one input");
    if inputs.len() == 1 {
        return Ok(inputs[0].clone());
    }
    let mut acc = mimc_hash_gadget(cs.clone(), inputs[0], inputs[1])?;
    for i in 2..inputs.len() {
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

impl ConstraintSynthesizer<Fr> for AggregationCircuit {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<Fr>,
    ) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        let aggregation_commitment_var = FpVar::new_input(cs.clone(), || {
            self.aggregation_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let cross_tier_envelope_var = FpVar::new_input(cs.clone(), || {
            self.cross_tier_envelope.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let cross_tier_merkle_root_var = FpVar::new_input(cs.clone(), || {
            self.cross_tier_merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: sensor proof public inputs ===
        let sensor_envelope_var = FpVar::new_witness(cs.clone(), || {
            self.sensor_envelope_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let sensor_merkle_var = FpVar::new_witness(cs.clone(), || {
            self.sensor_merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let sensor_cycle_var = FpVar::new_witness(cs.clone(), || {
            self.sensor_cycle_index.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: intent proof public inputs ===
        let intent_commitment_var = FpVar::new_witness(cs.clone(), || {
            self.intent_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let intent_merkle_var = FpVar::new_witness(cs.clone(), || {
            self.intent_merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let intent_envelope_var = FpVar::new_witness(cs.clone(), || {
            self.intent_envelope_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let intent_policy_var = FpVar::new_witness(cs.clone(), || {
            self.intent_policy_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: consensus proof public inputs ===
        let consensus_valset_var = FpVar::new_witness(cs.clone(), || {
            self.consensus_validator_set_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let consensus_epoch_var = FpVar::new_witness(cs.clone(), || {
            self.consensus_epoch.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let consensus_vote_var = FpVar::new_witness(cs.clone(), || {
            self.consensus_vote_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Constraint 1: Cross-tier consistency ===
        // Envelope commitment must match between sensor and intent proofs
        sensor_envelope_var.enforce_equal(&intent_envelope_var)?;
        sensor_envelope_var.enforce_equal(&cross_tier_envelope_var)?;

        // Merkle root must match between sensor and intent proofs
        sensor_merkle_var.enforce_equal(&intent_merkle_var)?;
        sensor_merkle_var.enforce_equal(&cross_tier_merkle_root_var)?;

        // === Constraint 2: Aggregation binding ===
        // aggregation_commitment = H(
        //     sensor_envelope, sensor_merkle, sensor_cycle,
        //     intent_commitment, intent_merkle, intent_envelope, intent_policy,
        //     consensus_valset, consensus_epoch, consensus_vote
        // )
        let all_inputs = [
            &sensor_envelope_var, &sensor_merkle_var, &sensor_cycle_var,
            &intent_commitment_var, &intent_merkle_var, &intent_envelope_var, &intent_policy_var,
            &consensus_valset_var, &consensus_epoch_var, &consensus_vote_var,
        ];
        let computed_commitment = mimc_hash_many_gadget(cs.clone(), &all_inputs)?;
        computed_commitment.enforce_equal(&aggregation_commitment_var)?;

        Ok(())
    }
}

// === Native (off-circuit) helpers ===

pub fn compute_aggregation_commitment(
    sensor_envelope_commitment: Fr,
    sensor_merkle_root: Fr,
    sensor_cycle_index: Fr,
    intent_commitment: Fr,
    intent_merkle_root: Fr,
    intent_envelope_commitment: Fr,
    intent_policy_commitment: Fr,
    consensus_validator_set_root: Fr,
    consensus_epoch: Fr,
    consensus_vote_commitment: Fr,
) -> Fr {
    let inputs = [
        sensor_envelope_commitment, sensor_merkle_root, sensor_cycle_index,
        intent_commitment, intent_merkle_root, intent_envelope_commitment, intent_policy_commitment,
        consensus_validator_set_root, consensus_epoch, consensus_vote_commitment,
    ];
    if inputs.len() == 1 {
        return inputs[0];
    }
    let mut acc = mimc_hash(inputs[0], inputs[1]);
    for i in 2..inputs.len() {
        acc = mimc_hash(acc, inputs[i]);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};
    use sensor_safety_circuit::envelope_commitment as compute_env_commit;
    use intent_safety_circuit::{
        generate_intent_proof_data,
    };
    use consensus_safety_circuit::{
        generate_consensus_proof_data,
    };

    #[test]
    fn test_aggregation_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(42);

        // Sensor proof public inputs
        let sensor_envelope = compute_env_commit(
            Fr::from(5000u64), Fr::from(50000u64), Fr::from(500u64),
            Fr::from(30000u64), Fr::from(3000u64),
        );
        let sensor_merkle_root = Fr::from(12345u64);
        let sensor_cycle_index = Fr::from(42u64);

        // Intent proof public inputs (envelope + merkle must match sensor)
        let intent_commitment = Fr::from(77777u64);
        let intent_merkle_root = sensor_merkle_root; // cross-tier consistency
        let intent_envelope = sensor_envelope;       // cross-tier consistency
        let intent_policy = Fr::from(88888u64);

        // Consensus proof public inputs
        let consensus_valset = Fr::from(99999u64);
        let consensus_epoch = Fr::from(1u64);
        let consensus_vote = Fr::from(55555u64);

        // Compute aggregation commitment
        let agg_commit = compute_aggregation_commitment(
            sensor_envelope, sensor_merkle_root, sensor_cycle_index,
            intent_commitment, intent_merkle_root, intent_envelope, intent_policy,
            consensus_valset, consensus_epoch, consensus_vote,
        );

        let circuit = AggregationCircuit::new(
            agg_commit,
            sensor_envelope,    // cross_tier_envelope
            sensor_merkle_root, // cross_tier_merkle_root
            sensor_envelope, sensor_merkle_root, sensor_cycle_index,
            intent_commitment, intent_merkle_root, intent_envelope, intent_policy,
            consensus_valset, consensus_epoch, consensus_vote,
        );

        let empty = AggregationCircuit::empty();
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![agg_commit, sensor_envelope, sensor_merkle_root];
        let valid = Groth16::<ark_bn254::Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "aggregation proof should verify");
    }

    #[test]
    fn test_aggregation_circuit_envelope_mismatch_fails() {
        let rng = &mut StdRng::seed_from_u64(99);

        let sensor_envelope = Fr::from(100u64);
        let sensor_merkle = Fr::from(200u64);
        let wrong_envelope = Fr::from(999u64); // different!

        let agg_commit = compute_aggregation_commitment(
            sensor_envelope, sensor_merkle, Fr::from(42u64),
            Fr::from(777u64), sensor_merkle, wrong_envelope, Fr::from(888u64),
            Fr::from(999u64), Fr::from(1u64), Fr::from(555u64),
        );

        // Circuit with mismatched envelopes — should fail
        let circuit = AggregationCircuit::new(
            agg_commit,
            sensor_envelope, sensor_merkle,
            sensor_envelope, sensor_merkle, Fr::from(42u64),
            Fr::from(777u64), sensor_merkle, wrong_envelope, Fr::from(888u64),
            Fr::from(999u64), Fr::from(1u64), Fr::from(555u64),
        );

        let empty = AggregationCircuit::empty();
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for envelope mismatch");
    }

    #[test]
    fn test_aggregation_circuit_merkle_mismatch_fails() {
        let rng = &mut StdRng::seed_from_u64(77);

        let sensor_envelope = Fr::from(100u64);
        let sensor_merkle = Fr::from(200u64);
        let wrong_merkle = Fr::from(999u64); // different!

        let agg_commit = compute_aggregation_commitment(
            sensor_envelope, sensor_merkle, Fr::from(42u64),
            Fr::from(777u64), wrong_merkle, sensor_envelope, Fr::from(888u64),
            Fr::from(999u64), Fr::from(1u64), Fr::from(555u64),
        );

        let circuit = AggregationCircuit::new(
            agg_commit,
            sensor_envelope, sensor_merkle,
            sensor_envelope, sensor_merkle, Fr::from(42u64),
            Fr::from(777u64), wrong_merkle, sensor_envelope, Fr::from(888u64),
            Fr::from(999u64), Fr::from(1u64), Fr::from(555u64),
        );

        let empty = AggregationCircuit::empty();
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for merkle root mismatch");
    }

    #[test]
    fn test_aggregation_circuit_wrong_commitment_fails() {
        let rng = &mut StdRng::seed_from_u64(55);

        let sensor_envelope = Fr::from(100u64);
        let sensor_merkle = Fr::from(200u64);

        let wrong_commit = Fr::from(123456789u64); // wrong aggregation commitment

        let circuit = AggregationCircuit::new(
            wrong_commit,
            sensor_envelope, sensor_merkle,
            sensor_envelope, sensor_merkle, Fr::from(42u64),
            Fr::from(777u64), sensor_merkle, sensor_envelope, Fr::from(888u64),
            Fr::from(999u64), Fr::from(1u64), Fr::from(555u64),
        );

        let empty = AggregationCircuit::empty();
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong aggregation commitment");
    }

    #[test]
    fn test_full_three_tier_aggregation() {
        let rng = &mut StdRng::seed_from_u64(11);

        // === Generate sensor proof data ===
        let sensor_envelope = compute_env_commit(
            Fr::from(5000u64), Fr::from(50000u64), Fr::from(500u64),
            Fr::from(30000u64), Fr::from(3000u64),
        );
        let sensor_cycle_index = Fr::from(42u64);

        // === Generate intent proof data ===
        let action = Fr::from(1u64);
        let params_x = Fr::from(500u64);
        let params_y = Fr::from(300u64);
        let sensor_snapshot_hash = Fr::from(12345u64); // sensor reading hash
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
        let (intent_commit, intent_merkle, intent_envelope, intent_policy, _path, _bits) =
            generate_intent_proof_data(
                action, params_x, params_y, sensor_snapshot_hash, agent_id,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                zone_x_min, zone_x_max, zone_y_min, zone_y_max,
                tree_height,
            );

        // Cross-tier consistency: the sensor batch Merkle root IS the intent Merkle root
        let sensor_merkle_root = intent_merkle;
        assert_eq!(intent_envelope, sensor_envelope, "envelope must match across tiers");

        // === Generate consensus proof data ===
        let validator_pubkey = Fr::from(12345u64);
        let block_hash = Fr::from(99999u64);
        let vote_decision = Fr::from(1u64);
        let epoch = Fr::from(1u64);

        let (consensus_valset, consensus_vote, _cpath, _cbits) =
            generate_consensus_proof_data(
                validator_pubkey, block_hash, vote_decision, epoch, tree_height,
            );

        // === Generate aggregation proof ===
        let agg_commit = compute_aggregation_commitment(
            sensor_envelope, sensor_merkle_root, sensor_cycle_index,
            intent_commit, intent_merkle, intent_envelope, intent_policy,
            consensus_valset, epoch, consensus_vote,
        );

        let circuit = AggregationCircuit::new(
            agg_commit,
            sensor_envelope, sensor_merkle_root,
            sensor_envelope, sensor_merkle_root, sensor_cycle_index,
            intent_commit, intent_merkle, intent_envelope, intent_policy,
            consensus_valset, epoch, consensus_vote,
        );

        let empty = AggregationCircuit::empty();
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![agg_commit, sensor_envelope, sensor_merkle_root];
        let valid = Groth16::<ark_bn254::Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "full three-tier aggregation proof should verify");
    }

    #[test]
    fn test_aggregation_commitment_deterministic() {
        let c1 = compute_aggregation_commitment(
            Fr::from(1u64), Fr::from(2u64), Fr::from(3u64),
            Fr::from(4u64), Fr::from(5u64), Fr::from(6u64), Fr::from(7u64),
            Fr::from(8u64), Fr::from(9u64), Fr::from(10u64),
        );
        let c2 = compute_aggregation_commitment(
            Fr::from(1u64), Fr::from(2u64), Fr::from(3u64),
            Fr::from(4u64), Fr::from(5u64), Fr::from(6u64), Fr::from(7u64),
            Fr::from(8u64), Fr::from(9u64), Fr::from(10u64),
        );
        assert_eq!(c1, c2);
    }
}
