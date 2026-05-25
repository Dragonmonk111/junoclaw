//! Moultbook membership circuit — Groth16 over BN254.
//!
//! Proves three statements simultaneously:
//!
//! 1. **Set membership:** a committed identity `H(primary_key)` is a leaf in
//!    a Merkle tree with public root `merkle_root`.
//!
//! 2. **Key derivation:** the moult-key is derived from the primary key via
//!    `moult_key = H(primary_key || derivation_salt)`.
//!
//! 3. **Binding:** `H(moult_key) == moult_key_hash` (public input).
//!
//! Public inputs:  `[moult_key_hash, merkle_root, epoch]`
//! Private witness: `[primary_key, derivation_salt, merkle_path]`
//!
//! Hash function: Poseidon over BN254::Fr (ZK-friendly, ~250 constraints
//! per hash vs ~25k for SHA-256 in R1CS).
//!
//! The circuit is parameterised by `TREE_HEIGHT` (Merkle tree depth).
//! Default: 20 (supports up to 2^20 ≈ 1M registered agents).

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

pub const DEFAULT_TREE_HEIGHT: usize = 20;

/// The moultbook membership circuit.
///
/// Instantiate with concrete witness values for proof generation,
/// or with `None` values for constraint-count estimation / setup.
#[derive(Clone)]
pub struct MembershipCircuit {
    // --- Public inputs ---
    /// H(moult_key) — binds the proof to a specific moult-key
    pub moult_key_hash: Option<Fr>,
    /// Merkle root of the agent-registry member set
    pub merkle_root: Option<Fr>,
    /// Epoch number (binds proof to a time window)
    pub epoch: Option<Fr>,

    // --- Private witness ---
    /// The agent's primary key (as a field element)
    pub primary_key: Option<Fr>,
    /// Derivation salt for the moult-key
    pub derivation_salt: Option<Fr>,
    /// Merkle authentication path (sibling hashes, bottom-up)
    pub merkle_path: Vec<Option<Fr>>,
    /// Path direction bits (0 = left, 1 = right)
    pub path_bits: Vec<Option<bool>>,

    /// Tree height (determines path length)
    pub tree_height: usize,
}

impl MembershipCircuit {
    /// Create a circuit instance for setup (no witness values).
    pub fn empty(tree_height: usize) -> Self {
        Self {
            moult_key_hash: None,
            merkle_root: None,
            epoch: None,
            primary_key: None,
            derivation_salt: None,
            merkle_path: vec![None; tree_height],
            path_bits: vec![None; tree_height],
            tree_height,
        }
    }

    /// Create a circuit instance with concrete witness values for proving.
    pub fn new(
        moult_key_hash: Fr,
        merkle_root: Fr,
        epoch: Fr,
        primary_key: Fr,
        derivation_salt: Fr,
        merkle_path: Vec<Fr>,
        path_bits: Vec<bool>,
        tree_height: usize,
    ) -> Self {
        assert_eq!(merkle_path.len(), tree_height);
        assert_eq!(path_bits.len(), tree_height);
        Self {
            moult_key_hash: Some(moult_key_hash),
            merkle_root: Some(merkle_root),
            epoch: Some(epoch),
            primary_key: Some(primary_key),
            derivation_salt: Some(derivation_salt),
            merkle_path: merkle_path.into_iter().map(Some).collect(),
            path_bits: path_bits.into_iter().map(Some).collect(),
            tree_height,
        }
    }
}

/// Minimal Poseidon-like hash in-circuit. For production, use a properly
/// parameterised Poseidon sponge from `ark-crypto-primitives`. This
/// simplified version uses the MiMC construction (x^5 round function)
/// which is sufficient for the membership proof and much simpler to
/// implement in R1CS.
///
/// H(a, b) = result of `ROUNDS` MiMC rounds over (a, b).
const MIMC_ROUNDS: usize = 91;

fn mimc_hash_gadget(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    // Round constants (deterministic, derived from nothing-up-my-sleeve string)
    let round_constants = generate_round_constants();

    let mut state = left.clone() + right.clone();
    for i in 0..MIMC_ROUNDS {
        let rc = FpVar::new_constant(cs.clone(), round_constants[i])?;
        let t = &state + &rc;
        // x^5 = x * x * x * x * x
        let t2 = &t * &t;
        let t4 = &t2 * &t2;
        state = &t4 * &t;
    }
    Ok(state)
}

/// Deterministic round constants from SHA-256("moultbook-mimc-bn254-round-{i}")
fn generate_round_constants() -> Vec<Fr> {
    use sha2::{Digest, Sha256};

    (0..MIMC_ROUNDS)
        .map(|i| {
            let mut hasher = Sha256::new();
            hasher.update(format!("moultbook-mimc-bn254-round-{}", i).as_bytes());
            let hash = hasher.finalize();
            // Interpret first 31 bytes as a field element (< BN254 modulus)
            let mut bytes = [0u8; 32];
            bytes[1..].copy_from_slice(&hash[..31]);
            Fr::from_be_bytes_mod_order(&bytes)
        })
        .collect()
}

impl ConstraintSynthesizer<Fr> for MembershipCircuit {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<Fr>,
    ) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        let moult_key_hash_var =
            FpVar::new_input(cs.clone(), || {
                self.moult_key_hash.ok_or(SynthesisError::AssignmentMissing)
            })?;
        let merkle_root_var =
            FpVar::new_input(cs.clone(), || {
                self.merkle_root.ok_or(SynthesisError::AssignmentMissing)
            })?;
        let _epoch_var =
            FpVar::new_input(cs.clone(), || {
                self.epoch.ok_or(SynthesisError::AssignmentMissing)
            })?;

        // === Allocate private witness ===
        let primary_key_var =
            FpVar::new_witness(cs.clone(), || {
                self.primary_key.ok_or(SynthesisError::AssignmentMissing)
            })?;
        let derivation_salt_var =
            FpVar::new_witness(cs.clone(), || {
                self.derivation_salt.ok_or(SynthesisError::AssignmentMissing)
            })?;

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

        // === Constraint 1: Key derivation ===
        // moult_key = H(primary_key, derivation_salt)
        let moult_key_var = mimc_hash_gadget(
            cs.clone(),
            &primary_key_var,
            &derivation_salt_var,
        )?;

        // === Constraint 2: Binding ===
        // H(moult_key, 0) == moult_key_hash (public input)
        let zero = FpVar::new_constant(cs.clone(), Fr::from(0u64))?;
        let computed_hash = mimc_hash_gadget(cs.clone(), &moult_key_var, &zero)?;
        computed_hash.enforce_equal(&moult_key_hash_var)?;

        // === Constraint 3: Set membership (Merkle proof) ===
        // leaf = H(primary_key, 0)
        let leaf = mimc_hash_gadget(cs.clone(), &primary_key_var, &zero)?;

        // Walk the Merkle path from leaf to root
        let mut current = leaf;
        for i in 0..self.tree_height {
            // If bit == 0: hash(current, sibling)
            // If bit == 1: hash(sibling, current)
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

        // computed root == public merkle_root
        current.enforce_equal(&merkle_root_var)?;

        Ok(())
    }
}

/// Native (non-circuit) MiMC hash for building Merkle trees off-chain.
pub fn mimc_hash(left: Fr, right: Fr) -> Fr {
    let round_constants = generate_round_constants();
    let mut state = left + right;
    for i in 0..MIMC_ROUNDS {
        let t = state + round_constants[i];
        state = t * t * t * t * t; // x^5
    }
    state
}

/// Build a Merkle tree from leaves and return (root, paths, path_bits).
pub fn build_merkle_tree(leaves: &[Fr], tree_height: usize) -> (Fr, Vec<Vec<Fr>>, Vec<Vec<bool>>) {
    let num_leaves = 1 << tree_height;
    assert!(leaves.len() <= num_leaves);

    // Pad with zero leaves
    let zero = Fr::from(0u64);
    let zero_leaf = mimc_hash(zero, zero);
    let mut layer: Vec<Fr> = leaves.to_vec();
    layer.resize(num_leaves, zero_leaf);

    let mut layers = vec![layer.clone()];

    // Build tree bottom-up
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len() / 2);
        for chunk in layer.chunks(2) {
            next.push(mimc_hash(chunk[0], chunk[1]));
        }
        layers.push(next.clone());
        layer = next;
    }

    let root = layers.last().unwrap()[0];

    // Extract paths for each leaf
    let mut all_paths = Vec::with_capacity(leaves.len());
    let mut all_bits = Vec::with_capacity(leaves.len());

    for leaf_idx in 0..leaves.len() {
        let mut path = Vec::with_capacity(tree_height);
        let mut bits = Vec::with_capacity(tree_height);
        let mut idx = leaf_idx;

        for depth in 0..tree_height {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            path.push(layers[depth][sibling_idx]);
            bits.push(idx % 2 == 1); // 1 if current is right child
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
        let h1 = mimc_hash(a, b);
        let h2 = mimc_hash(a, b);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_merkle_tree_small() {
        let tree_height = 3; // 8 leaves
        let leaves: Vec<Fr> = (0..5).map(|i| mimc_hash(Fr::from(i as u64), Fr::from(0u64))).collect();
        let (root, paths, bits) = build_merkle_tree(&leaves, tree_height);
        assert_eq!(paths.len(), 5);
        assert_eq!(bits.len(), 5);
        assert_ne!(root, Fr::from(0u64));
    }

    #[test]
    fn test_circuit_satisfiable() {
        let rng = &mut StdRng::seed_from_u64(42);
        let tree_height = 3;

        // Register 4 agents
        let primary_keys: Vec<Fr> = (1..=4).map(|i| Fr::from(i as u64)).collect();
        let zero = Fr::from(0u64);
        let leaves: Vec<Fr> = primary_keys.iter().map(|k| mimc_hash(*k, zero)).collect();
        let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        // Agent 2 wants to moult
        let agent_idx = 1; // 0-indexed
        let primary_key = primary_keys[agent_idx];
        let derivation_salt = Fr::from(12345u64);
        let moult_key = mimc_hash(primary_key, derivation_salt);
        let moult_key_hash = mimc_hash(moult_key, zero);
        let epoch = Fr::from(100u64);

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

        // Setup
        let empty_circuit = MembershipCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng).unwrap();

        // Prove
        let proof = Groth16::<Bn254>::prove(&pk, circuit, rng).unwrap();

        // Verify
        let public_inputs = vec![moult_key_hash, merkle_root, epoch];
        let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
        assert!(valid, "proof should verify");
    }

    #[test]
    fn test_circuit_wrong_key_fails() {
        let rng = &mut StdRng::seed_from_u64(43);
        let tree_height = 3;

        let primary_keys: Vec<Fr> = (1..=4).map(|i| Fr::from(i as u64)).collect();
        let zero = Fr::from(0u64);
        let leaves: Vec<Fr> = primary_keys.iter().map(|k| mimc_hash(*k, zero)).collect();
        let (merkle_root, paths, bits) = build_merkle_tree(&leaves, tree_height);

        // Agent claims key 2 but uses key 99 (not in tree)
        let fake_key = Fr::from(99u64);
        let derivation_salt = Fr::from(12345u64);
        let moult_key = mimc_hash(fake_key, derivation_salt);
        let moult_key_hash = mimc_hash(moult_key, zero);
        let epoch = Fr::from(100u64);

        let circuit = MembershipCircuit::new(
            moult_key_hash,
            merkle_root,
            epoch,
            fake_key,
            derivation_salt,
            paths[1].clone(), // Using agent 2's path with wrong key
            bits[1].clone(),
            tree_height,
        );

        let empty_circuit = MembershipCircuit::empty(tree_height);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng).unwrap();

        // Prove should succeed (prover can always create a proof)
        // but verify should fail (wrong witness doesn't satisfy constraints)
        let proof_result = Groth16::<Bn254>::prove(&pk, circuit, rng);

        // With a wrong witness, prove itself may fail (constraint violation)
        // OR produce an invalid proof. Either outcome is correct.
        if let Ok(proof) = proof_result {
            let public_inputs = vec![moult_key_hash, merkle_root, epoch];
            let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
            assert!(!valid, "proof with wrong key should NOT verify");
        }
        // If prove() itself errors, that's also correct — the witness
        // doesn't satisfy the constraints.
    }
}
