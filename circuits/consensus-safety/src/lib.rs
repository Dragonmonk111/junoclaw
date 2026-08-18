//! Consensus-tier ZK circuit — proves a validator is registered and voted
//! correctly without revealing which validator.
//!
//! ### Statement
//!
//! Given public inputs:
//! - `validator_set_root`: Merkle root of the validator set for this epoch
//! - `epoch`: consensus epoch number
//! - `vote_commitment`: H(block_hash, vote_decision, epoch)
//!
//! And private witness:
//! - `validator_pubkey`: the validator's public key (field element encoding)
//! - `merkle_path`, `path_bits`: Merkle authentication path
//! - `block_hash`: hash of the block being voted on
//! - `vote_decision`: vote decision (e.g., 1 = yes, 2 = no, 3 = abstain)
//!
//! The circuit proves:
//! 1. **Validator membership**: `H(validator_pubkey)` is a leaf in the Merkle tree
//!    with root `validator_set_root`
//! 2. **Vote binding**: `H(block_hash, vote_decision, epoch) == vote_commitment`
//!
//! ### Design rationale
//!
//! BLS aggregate signature verification happens outside the circuit (on-chain or
//! by TEE). The aggregate already proves 2f+1 threshold was met. This circuit
//! adds the privacy layer: "a registered validator participated" without revealing
//! which one. No BLS-in-R1CS needed — the hard crypto is delegated to the TEE.
//!
//! ### Aggregation (Plan D)
//!
//! This proof is one of three Groth16 proofs aggregated by the AggregationCircuit.
//! The TEE verifies all three Groth16 pairings and binds the result to a
//! commitment hash. The aggregation circuit proves cross-tier consistency.

use ark_bn254::Fr;
use ark_ff::PrimeField;
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
    mimc_hash, build_merkle_tree,
};

pub const DEFAULT_TREE_HEIGHT: usize = 20;
const MIMC_ROUNDS: usize = 91;

/// The consensus membership circuit.
#[derive(Clone)]
pub struct ConsensusMembershipCircuit {
    // --- Public inputs ---
    pub validator_set_root: Option<Fr>,
    pub epoch: Option<Fr>,
    pub vote_commitment: Option<Fr>,

    // --- Private witness: validator identity ---
    pub validator_pubkey: Option<Fr>,
    pub merkle_path: Vec<Option<Fr>>,
    pub path_bits: Vec<Option<bool>>,

    // --- Private witness: vote data ---
    pub block_hash: Option<Fr>,
    pub vote_decision: Option<Fr>,

    pub tree_height: usize,
}

impl ConsensusMembershipCircuit {
    pub fn empty(tree_height: usize) -> Self {
        Self {
            validator_set_root: None,
            epoch: None,
            vote_commitment: None,
            validator_pubkey: None,
            merkle_path: vec![None; tree_height],
            path_bits: vec![None; tree_height],
            block_hash: None,
            vote_decision: None,
            tree_height,
        }
    }

    pub fn new(
        validator_set_root: Fr,
        epoch: Fr,
        vote_commitment: Fr,
        validator_pubkey: Fr,
        merkle_path: Vec<Fr>,
        path_bits: Vec<bool>,
        block_hash: Fr,
        vote_decision: Fr,
        tree_height: usize,
    ) -> Self {
        Self {
            validator_set_root: Some(validator_set_root),
            epoch: Some(epoch),
            vote_commitment: Some(vote_commitment),
            validator_pubkey: Some(validator_pubkey),
            merkle_path: merkle_path.into_iter().map(Some).collect(),
            path_bits: path_bits.into_iter().map(Some).collect(),
            block_hash: Some(block_hash),
            vote_decision: Some(vote_decision),
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

fn mimc_hash_3_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: [&FpVar<Fr>; 3],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut acc = mimc_hash_gadget(cs.clone(), inputs[0], inputs[1])?;
    acc = mimc_hash_gadget(cs.clone(), &acc, inputs[2])?;
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

impl ConstraintSynthesizer<Fr> for ConsensusMembershipCircuit {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<Fr>,
    ) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        let validator_set_root_var = FpVar::new_input(cs.clone(), || {
            self.validator_set_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let epoch_var = FpVar::new_input(cs.clone(), || {
            self.epoch.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let vote_commitment_var = FpVar::new_input(cs.clone(), || {
            self.vote_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: validator identity ===
        let validator_pubkey_var = FpVar::new_witness(cs.clone(), || {
            self.validator_pubkey.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate private witness: vote data ===
        let block_hash_var = FpVar::new_witness(cs.clone(), || {
            self.block_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let vote_decision_var = FpVar::new_witness(cs.clone(), || {
            self.vote_decision.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Constraint 1: Validator membership (Merkle proof) ===
        // Leaf = H(validator_pubkey, validator_pubkey) — prevents trivial collisions
        let leaf = mimc_hash_gadget(cs.clone(), &validator_pubkey_var, &validator_pubkey_var)?;

        // Allocate Merkle path
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

        current.enforce_equal(&validator_set_root_var)?;

        // === Constraint 2: Vote binding ===
        // vote_commitment = H(block_hash, vote_decision, epoch)
        let computed_vote_commitment = mimc_hash_3_gadget(
            cs.clone(),
            [&block_hash_var, &vote_decision_var, &epoch_var],
        )?;
        computed_vote_commitment.enforce_equal(&vote_commitment_var)?;

        Ok(())
    }
}

// === Native (off-circuit) helpers ===

pub fn compute_vote_commitment(block_hash: Fr, vote_decision: Fr, epoch: Fr) -> Fr {
    let h1 = mimc_hash(block_hash, vote_decision);
    mimc_hash(h1, epoch)
}

pub fn compute_validator_leaf(validator_pubkey: Fr) -> Fr {
    mimc_hash(validator_pubkey, validator_pubkey)
}

/// Generate all proof data for a consensus membership proof.
pub fn generate_consensus_proof_data(
    validator_pubkey: Fr,
    block_hash: Fr,
    vote_decision: Fr,
    epoch: Fr,
    tree_height: usize,
) -> (
    Fr, // validator_set_root
    Fr, // vote_commitment
    Vec<Fr>,
    Vec<bool>,
) {
    let leaf = compute_validator_leaf(validator_pubkey);
    let leaves = vec![leaf];
    let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);
    let vote_commit = compute_vote_commitment(block_hash, vote_decision, epoch);

    (merkle_root, vote_commit, paths[0].clone(), bits[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn test_consensus_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(42);

        let validator_pubkey = Fr::from(12345u64);
        let block_hash = Fr::from(99999u64);
        let vote_decision = Fr::from(1u64); // 1 = yes
        let epoch = Fr::from(42u64);

        let tree_height = 4;
        let (val_set_root, vote_commit, path, bits) = generate_consensus_proof_data(
            validator_pubkey, block_hash, vote_decision, epoch, tree_height,
        );

        let circuit = ConsensusMembershipCircuit::new(
            val_set_root, epoch, vote_commit,
            validator_pubkey, path, bits,
            block_hash, vote_decision, tree_height,
        );

        let empty = ConsensusMembershipCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng).unwrap();

        let public_inputs = vec![val_set_root, epoch, vote_commit];
        let valid = Groth16::<ark_bn254::Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "consensus membership proof should verify");
    }

    #[test]
    fn test_consensus_circuit_wrong_validator_set_fails() {
        let rng = &mut StdRng::seed_from_u64(99);

        let validator_pubkey = Fr::from(12345u64);
        let block_hash = Fr::from(99999u64);
        let vote_decision = Fr::from(1u64);
        let epoch = Fr::from(42u64);

        let tree_height = 4;
        let (_val_set_root, vote_commit, path, bits) = generate_consensus_proof_data(
            validator_pubkey, block_hash, vote_decision, epoch, tree_height,
        );

        // Wrong validator set root
        let wrong_root = Fr::from(888888u64);

        let circuit = ConsensusMembershipCircuit::new(
            wrong_root, epoch, vote_commit,
            validator_pubkey, path, bits,
            block_hash, vote_decision, tree_height,
        );

        let empty = ConsensusMembershipCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong validator set root");
    }

    #[test]
    fn test_consensus_circuit_wrong_vote_commitment_fails() {
        let rng = &mut StdRng::seed_from_u64(77);

        let validator_pubkey = Fr::from(12345u64);
        let block_hash = Fr::from(99999u64);
        let vote_decision = Fr::from(1u64);
        let epoch = Fr::from(42u64);

        let tree_height = 4;
        let (val_set_root, _vote_commit, path, bits) = generate_consensus_proof_data(
            validator_pubkey, block_hash, vote_decision, epoch, tree_height,
        );

        // Wrong vote commitment (different vote decision)
        let wrong_vote_commit = compute_vote_commitment(block_hash, Fr::from(2u64), epoch);

        let circuit = ConsensusMembershipCircuit::new(
            val_set_root, epoch, wrong_vote_commit,
            validator_pubkey, path, bits,
            block_hash, vote_decision, tree_height,
        );

        let empty = ConsensusMembershipCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong vote commitment");
    }

    #[test]
    fn test_consensus_circuit_wrong_epoch_fails() {
        let rng = &mut StdRng::seed_from_u64(55);

        let validator_pubkey = Fr::from(12345u64);
        let block_hash = Fr::from(99999u64);
        let vote_decision = Fr::from(1u64);
        let epoch = Fr::from(42u64);

        let tree_height = 4;
        let (val_set_root, vote_commit, path, bits) = generate_consensus_proof_data(
            validator_pubkey, block_hash, vote_decision, epoch, tree_height,
        );

        // Wrong epoch
        let wrong_epoch = Fr::from(43u64);

        let circuit = ConsensusMembershipCircuit::new(
            val_set_root, wrong_epoch, vote_commit,
            validator_pubkey, path, bits,
            block_hash, vote_decision, tree_height,
        );

        let empty = ConsensusMembershipCircuit::empty(tree_height);
        let (pk, _vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Groth16::<ark_bn254::Bn254>::prove(&pk, circuit, rng)
        }));
        assert!(result.is_err(), "proof should fail for wrong epoch");
    }

    #[test]
    fn test_compute_vote_commitment_deterministic() {
        let bh = Fr::from(99999u64);
        let vd = Fr::from(1u64);
        let ep = Fr::from(42u64);

        let c1 = compute_vote_commitment(bh, vd, ep);
        let c2 = compute_vote_commitment(bh, vd, ep);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_different_validators_same_root() {
        // Two validators in the same set — both should prove membership
        let rng = &mut StdRng::seed_from_u64(33);

        let val1 = Fr::from(111u64);
        let val2 = Fr::from(222u64);

        let leaf1 = compute_validator_leaf(val1);
        let leaf2 = compute_validator_leaf(val2);

        let tree_height = 4;
        let leaves = vec![leaf1, leaf2];
        let (root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        // Validator 1 proves membership
        let block_hash = Fr::from(100u64);
        let vote_decision = Fr::from(1u64);
        let epoch = Fr::from(1u64);
        let vote_commit = compute_vote_commitment(block_hash, vote_decision, epoch);

        let circuit1 = ConsensusMembershipCircuit::new(
            root, epoch, vote_commit,
            val1, paths[0].clone(), bits[0].clone(),
            block_hash, vote_decision, tree_height,
        );

        let empty = ConsensusMembershipCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<ark_bn254::Bn254>::circuit_specific_setup(empty, rng).unwrap();

        let proof1 = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit1, rng).unwrap();
        let pis = vec![root, epoch, vote_commit];
        assert!(Groth16::<ark_bn254::Bn254>::verify(&vk, &pis, &proof1).unwrap());

        // Validator 2 proves membership
        let circuit2 = ConsensusMembershipCircuit::new(
            root, epoch, vote_commit,
            val2, paths[1].clone(), bits[1].clone(),
            block_hash, vote_decision, tree_height,
        );

        let proof2 = Groth16::<ark_bn254::Bn254>::prove(&pk, circuit2, rng).unwrap();
        assert!(Groth16::<ark_bn254::Bn254>::verify(&vk, &pis, &proof2).unwrap());
    }
}
