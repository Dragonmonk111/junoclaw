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
    fields::fp::FpVar,
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
    /// 1 if score >= threshold (matches J-Lens `mx >= threshold` semantics)
    pub detection_out: Option<Fr>,

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
            detection_out: None,
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

/// Canonical int8 encoding: a signed element maps to its byte value
/// (field element `v` with -128<=v<=127 maps to `v+128`).
pub fn to_byte(v: Fr) -> Fr {
    v + Fr::from(128u64)
}
pub fn from_byte(v: Fr) -> Fr {
    v - Fr::from(128u64)
}

/// Native bank commitment: H(bank_bytes ‖ threshold) over the canonical
/// int8 domain (each direction element is its byte value).
pub fn bank_commitment_native(bank_vec: &[Fr], threshold: Fr) -> Fr {
    let mut inputs: Vec<Fr> = bank_vec.iter().map(|b| to_byte(*b)).collect();
    inputs.push(threshold);
    hash_chain_native(&inputs)
}

/// Native readout leaf: H(activation_bytes) over the canonical int8
/// domain — byte-faithful to `jlens.integrity.v1` (one field element per
/// int8 byte).
pub fn readout_leaf_native(activations: &[Fr]) -> Fr {
    let bytes: Vec<Fr> = activations.iter().map(|a| to_byte(*a)).collect();
    hash_chain_native(&bytes)
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
        let detection_out_var = Boolean::new_input(cs.clone(), || {
            self.detection_out
                .map(|f| f == Fr::from(1u64))
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
        // bank_commitment == H(bank_bytes[0..DIM] ‖ threshold)
        let c128_bank = FpVar::new_constant(cs.clone(), Fr::from(128u64))?;
        let mut bank_inputs: Vec<FpVar<Fr>> = bank_vars
            .iter()
            .map(|b| b.clone() + &c128_bank)
            .collect();
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

        // === Constraint 3b: detection flag ===
        // detection=1  =>  score >= threshold
        // detection=0  =>  score < threshold   (i.e. score+1 <= threshold)
        let one = FpVar::new_constant(cs.clone(), Fr::from(1u64))?;
        let score_plus_one = &score_out_var + &one;
        let bound_a = detection_out_var.select(&threshold_var, &score_plus_one)?;
        let bound_b = detection_out_var.select(&score_out_var, &threshold_var)?;
        // native values for the witness decomposition
        let detection_native = self
            .detection_out
            .map(|f| f == Fr::from(1u64))
            .unwrap_or(false);
        let (a_val, b_val) = if detection_native {
            (self.threshold, self.score_out)
        } else {
            (
                self.score_out.map(|s| s + Fr::from(1u64)),
                self.threshold,
            )
        };
        enforce_leq(cs.clone(), &bound_a, &bound_b, a_val, b_val)?;

        // === Constraint 4: canonical int8 domain ===
        // Each activation/bank element must be a signed int8: we range-
        // check (v + 128) fits in 8 bits. This is the jlens.integrity.v1
        // canonicalization — the circuit cannot be fed out-of-domain
        // field values.
        for i in 0..DIM {
            let av = self.activations[i];
            let bv = self.bank_vec[i];
            // (a+128) in [0,255]  <=>  enforce_leq(0, a+128) with 8 bits
            enforce_u8(cs.clone(), &act_vars[i], av)?;
            enforce_u8(cs.clone(), &bank_vars[i], bv)?;
        }

        // === Constraint 5: leaf binding + Merkle membership ===
        // leaf = H(activation_bytes[0..DIM]) — bytes = a_i + 128
        let mut byte_vars: Vec<FpVar<Fr>> = Vec::with_capacity(DIM);
        let c128 = FpVar::new_constant(cs.clone(), Fr::from(128u64))?;
        for a in &act_vars {
            byte_vars.push(a.clone() + &c128);
        }
        let leaf = hash_chain_gadget(cs.clone(), &byte_vars)?;

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

/// Byte-range gadget: proves the witness `v` is a signed int8 by
/// decomposing `v + 128` into 8 bits.
fn enforce_u8(
    cs: ConstraintSystemRef<Fr>,
    v: &FpVar<Fr>,
    v_val: Option<Fr>,
) -> Result<(), SynthesisError> {
    let c128 = FpVar::new_constant(cs.clone(), Fr::from(128u64))?;
    let shifted = v.clone() + &c128;
    let shifted_val = v_val.map(|x| x + Fr::from(128u64));

    let bytes = shifted_val.unwrap_or_default().into_bigint().to_bytes_le();
    let byte0 = *bytes.first().unwrap_or(&0);

    let bits: Vec<Boolean<Fr>> = (0..8)
        .map(|i| {
            let bit = (byte0 >> i) & 1 == 1;
            Boolean::new_witness(cs.clone(), || Ok(bit))
        })
        .collect::<Result<_, _>>()?;

    let mut reconstructed = FpVar::new_constant(cs.clone(), Fr::from(0u64))?;
    let mut power = Fr::from(1u64);
    for bit in &bits {
        let bit_fp = FpVar::from(bit.clone());
        let term = bit_fp * FpVar::new_constant(cs.clone(), power)?;
        reconstructed = reconstructed + term;
        power.double_in_place();
    }
    reconstructed.enforce_equal(&shifted)?;
    Ok(())
}

/// Range gadget: proves `b - a` fits in `RANGE_BITS` unsigned bits,
/// i.e. `a <= b`. Same construction as sensor-safety.
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

/// Native detection flag — must match J-Lens `mx >= threshold`.
pub fn detection_native(score: Fr, threshold: Fr) -> Fr {
    Fr::from(if score >= threshold { 1u64 } else { 0u64 })
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
        let detection = detection_native(score, threshold);

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
            detection_out: Some(detection),
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
        // merkle_root, bank_commitment, score_out, integrity_out, detection_out.
        let public_inputs = vec![
            circuit.merkle_root.unwrap(),
            circuit.bank_commitment.unwrap(),
            circuit.score_out.unwrap(),
            circuit.integrity_out.unwrap(),
            circuit.detection_out.unwrap(),
        ];
        assert!(Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap());
    }

    #[test]
    fn detection_zero_case_verifies() {
        let mut rng = StdRng::seed_from_u64(9);
        // Threshold above the score => detection = 0.
        let acts: Vec<Fr> = (0..DIM).map(|i| Fr::from(10u64 + i as u64)).collect();
        let bank: Vec<Fr> = (0..DIM).map(|i| Fr::from(3u64 + i as u64)).collect();
        let score: Fr = acts.iter().zip(bank.iter()).map(|(a, b)| *a * *b).sum();
        let threshold = score + Fr::from(1u64); // score < threshold
        let integrity = Fr::from(SCALE) - score;
        let detection = detection_native(score, threshold);

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
            detection_out: Some(detection),
            activations: acts.iter().map(|a| Some(*a)).collect(),
            bank_vec: bank.iter().map(|b| Some(*b)).collect(),
            threshold: Some(threshold),
            merkle_path: paths[leaf_idx].iter().map(|p| Some(*p)).collect(),
            path_bits: bits[leaf_idx].iter().map(|b| Some(*b)).collect(),
            tree_height: height,
        };

        let (pk, vk) =
            Groth16::<Bn254>::circuit_specific_setup(JLensReadoutCircuit::empty(height), &mut rng)
                .unwrap();
        let proof = Groth16::<Bn254>::prove(&pk, circuit.clone(), &mut rng).unwrap();
        let public_inputs = vec![
            root,
            circuit.bank_commitment.unwrap(),
            score,
            integrity,
            detection,
        ];
        assert_eq!(detection, Fr::from(0u64));
        assert!(Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap());
    }

    /// Phase-0 freeze artifact: hash the exact R1CS the ceremony binds
    /// to. Any circuit edit changes this digest — that is the point.
    /// Run: cargo test -p jlens-readout -- --nocapture dump_r1cs_hash
    #[test]
    fn dump_r1cs_hash() {
        use ark_relations::r1cs::ConstraintSystem;
        use ark_serialize::CanonicalSerialize;
        use sha2::{Digest, Sha256};

        let cs = ConstraintSystem::<Fr>::new_ref();
        cs.set_mode(ark_relations::r1cs::SynthesisMode::Setup);
        JLensReadoutCircuit::empty(4)
            .generate_constraints(cs.clone())
            .unwrap();
        cs.finalize();
        let mut h = Sha256::new();
        h.update((cs.num_instance_variables() as u64).to_be_bytes());
        h.update((cs.num_witness_variables() as u64).to_be_bytes());
        h.update((cs.num_constraints() as u64).to_be_bytes());
        let mats = cs.to_matrices().unwrap();
        for m in [&mats.a, &mats.b, &mats.c] {
            for row in m.iter() {
                for (coeff, idx) in row.iter() {
                    let mut buf = Vec::new();
                    coeff.serialize_uncompressed(&mut buf).unwrap();
                    h.update(&buf);
                    h.update((*idx as u64).to_be_bytes());
                }
            }
        }
        eprintln!(
            "circuit_r1cs: constraints={} instance={} witness={} sha256={}",
            cs.num_constraints(),
            cs.num_instance_variables(),
            cs.num_witness_variables(),
            hex::encode(h.finalize())
        );
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
            circuit.detection_out.unwrap(),
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
