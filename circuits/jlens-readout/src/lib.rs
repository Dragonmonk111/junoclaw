//! J-Lens readout circuit — Groth16 over BN254.
//!
//! ### Statement (v0)
//!
//! Proves that a J-Lens activation readout committed on-chain produces the
//! recorded deterministic score and integrity — **without re-running the
//! probe and without revealing the activation vectors**.
//!
//! Public inputs:
//! - `merkle_root`: root of the reflex/attestation Merkle batch the readout
//!   leaf is committed under
//! - `bank_commitment`: H(bank direction ‖ threshold) — binds the proof to a
//!   registered probe bank
//! - `score_out`: the recorded cosine score for one concept
//! - `integrity_out`: the recorded integrity value (SCALE − score, scaled)
//!
//! Private witness:
//! - `activations[0..DIM]`: the int8-quantized activation projection
//!   (field elements, already dequantized to fixed-point integers)
//! - `bank_vec[0..DIM]`: the registered probe direction for the concept
//! - `threshold`: the concept's detection threshold (inside bank commitment)
//! - `merkle_path`, `path_bits`: Merkle authentication path to `merkle_root`
//!
//! The circuit proves:
//! 1. **Leaf binding**: `leaf = H(activations)` is a leaf in the batch tree
//!    with root `merkle_root` (same MiMC gadget + path walk as
//!    sensor-safety)
//! 2. **Bank binding**: `bank_commitment == H(bank_vec ‖ threshold)`
//! 3. **Readout arithmetic**: `score_out == Σᵢ activationsᵢ · bank_vecᵢ`
//!    (each product is one R1CS constraint; the sum is linear)
//! 4. **Integrity**: `integrity_out == SCALE − score_out`
//!
//! ### Honest scope
//!
//! v0 proves the *arithmetic* of the deterministic readout over committed
//! inputs. It does NOT yet prove:
//! - threshold comparison / detection-set membership (needs a range gadget —
//!   next increment)
//! - the full multi-concept bank or the int8 quantization/dequantization step
//!   (production DIM is larger; DIM=8 here is the spike fixture)
//! - that the leaf hash equals the on-chain `activation_root` byte-for-byte
//!   (the leaf scheme must be pinned to `jlens.integrity.v1` canonical bytes
//!   before production use — tracked in docs/ZK_READOUT_SPEC.md)

use ark_bn254::Fr;
use ark_ff::{PrimeField, AdditiveGroup, BigInteger};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_r1cs_std::{
    alloc::AllocVar,
    eq::EqGadget,
    fields::{fp::FpVar, FieldVar},
    boolean::Boolean,
    select::CondSelectGadget,
};
use ark_std::vec::Vec;
use sha2::{Digest, Sha256};

use sensor_safety_circuit::mimc_hash;

/// Activation vector dimension for the v0 fixture. Production J-Lens uses a
/// larger probe projection; the circuit is DIM-agnostic in structure.
pub const DIM: usize = 8;

/// Fixed-point scale for score/integrity (mirrors int8 fixed-point domain).
pub const SCALE: u64 = 1_000_000;

const MIMC_ROUNDS: usize = 91;

/// The J-Lens readout circuit.
#[derive(Clone)]
pub struct JLensReadoutCircuit {
    // --- Public inputs ---
    pub merkle_root: Option<Fr>,
    pub bank_commitment: Option<Fr>,
    pub score_out: Option<Fr>,
    pub integrity_out: Option<Fr>,

    // --- Private witness: committed activation readout ---
    pub activations: Vec<Option<Fr>>,

    // --- Private witness: probe bank direction + threshold ---
    pub bank_vec: Vec<Option<Fr>>,
    pub threshold: Option<Fr>,

    // --- Private witness: Merkle authentication ---
    pub merkle_path: Vec<Option<Fr>>,
    pub path_bits: Vec<Option<bool>>,

    pub tree_height: usize,
}

impl JLensReadoutCircuit {
    pub fn empty(tree_height: usize) -> Self {
        Self {
            merkle_root: None,
            bank_commitment: None,
            score_out: None,
            integrity_out: None,
            activations: vec![None; DIM],
            bank_vec: vec![None; DIM],
            threshold: None,
            merkle_path: vec![None; tree_height],
            path_bits: vec![None; tree_height],
            tree_height,
        }
    }
}

// === MiMC gadgets (same construction as sensor-safety / intent-safety) ===

fn round_constants() -> Vec<Fr> {
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

fn mimc_gadget(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let rcs = round_constants();
    let mut state = left.clone() + right.clone();
    for rc in &rcs {
        let rc_var = FpVar::new_constant(cs.clone(), *rc)?;
        let t = &state + &rc_var;
        let t2 = &t * &t;
        let t4 = &t2 * &t2;
        state = &t4 * &t;
    }
    Ok(state)
}

/// H(a₀,…,aₙ₋₁) = fold of pairwise MiMC: acc = H(H(…H(a₀,a₁),a₂…),aₙ₋₁)
fn hash_chain_gadget(
    cs: ConstraintSystemRef<Fr>,
    inputs: &[FpVar<Fr>],
) -> Result<FpVar<Fr>, SynthesisError> {
    assert!(inputs.len() >= 2, "hash chain needs >= 2 inputs");
    let mut acc = mimc_gadget(cs.clone(), &inputs[0], &inputs[1])?;
    for i in 2..inputs.len() {
        acc = mimc_gadget(cs.clone(), &acc, &inputs[i])?;
    }
    Ok(acc)
}

/// Native equivalent of `hash_chain_gadget` — must match bit-for-bit.
pub fn hash_chain_native(inputs: &[Fr]) -> Fr {
    assert!(inputs.len() >= 2);
    let mut acc = mimc_hash(inputs[0], inputs[1]);
    for i in 2..inputs.len() {
        acc = mimc_hash(acc, inputs[i]);
    }
    acc
}

/// Native bank commitment: H(bank_vec ‖ threshold).
pub fn bank_commitment_native(bank_vec: &[Fr], threshold: Fr) -> Fr {
    let mut inputs = bank_vec.to_vec();
    inputs.push(threshold);
    hash_chain_native(&inputs)
}

/// Native readout leaf: H(activations).
pub fn readout_leaf_native(activations: &[Fr]) -> Fr {
    hash_chain_native(activations)
}

impl ConstraintSynthesizer<Fr> for JLensReadoutCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // --- Public inputs ---
        let merkle_root_var = FpVar::new_input(cs.clone(), || {
            self.merkle_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let bank_commitment_var = FpVar::new_input(cs.clone(), || {
            self.bank_commitment
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let score_out_var = FpVar::new_input(cs.clone(), || {
            self.score_out.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let integrity_out_var = FpVar::new_input(cs.clone(), || {
            self.integrity_out
                .ok_or(SynthesisError::AssignmentMissing)
        })?;

        // --- Witness variables ---
        let mut act_vars = Vec::with_capacity(DIM);
        for i in 0..DIM {
            act_vars.push(FpVar::new_witness(cs.clone(), || {
                self.activations[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
        }
        let mut bank_vars = Vec::with_capacity(DIM);
        for i in 0..DIM {
            bank_vars.push(FpVar::new_witness(cs.clone(), || {
                self.bank_vec[i].ok_or(SynthesisError::AssignmentMissing)
            })?);
        }
        let threshold_var = FpVar::new_witness(cs.clone(), || {
            self.threshold.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Constraint 1: bank binding ===
        // bank_commitment == H(bank_vec[0..DIM] ‖ threshold)
        let mut bank_inputs: Vec<FpVar<Fr>> = bank_vars.clone();
        bank_inputs.push(threshold_var.clone());
        let computed_bank = hash_chain_gadget(cs.clone(), &bank_inputs)?;
        computed_bank.enforce_equal(&bank_commitment_var)?;

        // === Constraint 2: readout arithmetic ===
        // score_out == Σᵢ actᵢ · bankᵢ   (FpVar mul generates one R1CS
        // constraint per product term automatically)
        let mut score_var = FpVar::new_constant(cs.clone(), Fr::from(0u64))?;
        for i in 0..DIM {
            score_var = score_var + &act_vars[i] * &bank_vars[i];
        }
        score_var.enforce_equal(&score_out_var)?;

        // === Constraint 3: integrity ===
        // integrity_out == SCALE − score_out
        let scale_var = FpVar::new_constant(cs.clone(), Fr::from(SCALE))?;
        let expected_integrity = &scale_var - &score_out_var;
        expected_integrity.enforce_equal(&integrity_out_var)?;

        // === Constraint 4: leaf binding + Merkle membership ===
        // leaf = H(activations[0..DIM])
        let leaf = hash_chain_gadget(cs.clone(), &act_vars)?;

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
            current = mimc_gadget(cs.clone(), &left, &right)?;
        }
        current.enforce_equal(&merkle_root_var)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Bn254;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    fn fixture() -> (JLensReadoutCircuit, Vec<Fr>, Vec<Fr>) {
        // Deterministic small readout: a·d dot product over DIM=8.
        let acts: Vec<Fr> = (0..DIM).map(|i| Fr::from(10u64 + i as u64)).collect();
        let bank: Vec<Fr> = (0..DIM).map(|i| Fr::from(3u64 + i as u64)).collect();
        let threshold = Fr::from(500u64);

        let score: Fr = acts
            .iter()
            .zip(bank.iter())
            .map(|(a, b)| *a * *b)
            .sum();
        let integrity = Fr::from(SCALE) - score;

        // Leaf + Merkle tree (single real leaf, rest zero-padded).
        let leaf = readout_leaf_native(&acts);
        let zero_leaf = sensor_safety_circuit::mimc_hash(Fr::from(0u64), Fr::from(0u64));
        let height = 4;
        let mut leaves = vec![zero_leaf; (1 << height) - 1];
        leaves.push(leaf);
        let (root, paths, bits) = build_merkle_tree_for_fixture(&leaves, height);
        let leaf_idx = leaves.len() - 1;

        let circuit = JLensReadoutCircuit {
            merkle_root: Some(root),
            bank_commitment: Some(bank_commitment_native(&bank, threshold)),
            score_out: Some(score),
            integrity_out: Some(integrity),
            activations: acts.iter().map(|a| Some(*a)).collect(),
            bank_vec: bank.iter().map(|b| Some(*b)).collect(),
            threshold: Some(threshold),
            merkle_path: paths[leaf_idx].iter().map(|p| Some(*p)).collect(),
            path_bits: bits[leaf_idx].iter().map(|b| Some(*b)).collect(),
            tree_height: height,
        };
        (circuit, vec![score, integrity, root, bank_commitment_native(&bank, threshold)], acts)
    }

    #[test]
    fn proof_verifies() {
        let mut rng = StdRng::seed_from_u64(7);
        let (circuit, _public, _acts) = fixture();
        let (pk, vk) =
            Groth16::<Bn254>::circuit_specific_setup(JLensReadoutCircuit::empty(4), &mut rng).unwrap();
        let proof = Groth16::<Bn254>::prove(&pk, circuit.clone(), &mut rng).unwrap();

        // Public inputs must be in allocation order:
        // merkle_root, bank_commitment, score_out, integrity_out.
        let public_inputs = vec![
            circuit.merkle_root.unwrap(),
            circuit.bank_commitment.unwrap(),
            circuit.score_out.unwrap(),
            circuit.integrity_out.unwrap(),
        ];
        assert!(Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap());
    }

    #[test]
    fn wrong_score_rejected() {
        let mut rng = StdRng::seed_from_u64(8);
        let (circuit, _, _) = fixture();
        let (pk, vk) =
            Groth16::<Bn254>::circuit_specific_setup(JLensReadoutCircuit::empty(4), &mut rng).unwrap();
        let proof = Groth16::<Bn254>::prove(&pk, circuit.clone(), &mut rng).unwrap();

        // Tamper: claim a different score.
        let bad_inputs = vec![
            circuit.merkle_root.unwrap(),
            circuit.bank_commitment.unwrap(),
            circuit.score_out.unwrap() + Fr::from(1u64),
            circuit.integrity_out.unwrap(),
        ];
        assert!(!Groth16::<Bn254>::verify(&vk, &bad_inputs, &proof).unwrap_or(false));
    }
}

/// Re-export of the shared Merkle builder (kept under a jlens name so callers
/// don't reach into sensor-safety directly).
pub fn build_merkle_tree_for_fixture(
    leaves: &[Fr],
    tree_height: usize,
) -> (Fr, Vec<Vec<Fr>>, Vec<Vec<bool>>) {
    sensor_safety_circuit::build_merkle_tree(leaves, tree_height)
}
