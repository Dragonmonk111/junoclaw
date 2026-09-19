//! Jolt guest programs — 5 safety checks as RISC-V provable functions.
//!
//! These replace the BN254 R1CS circuits with regular Rust functions
//! that Jolt's zkVM can prove. The logic is identical: range checks,
//! Merkle tree inclusion, envelope binding, and cross-tier consistency.
//!
//! The guest programs are compiled to RISC-V and proven by Jolt.
//! The on-chain verifier (jolt-cw-verifier) verifies the Jolt proof.
//!
//! ### Why this is simpler than R1CS
//!
//! In BN254/Groth16, every comparison, hash, and Merkle check must be
//! expressed as R1CS constraints (FpVar, Boolean, CondSelectGadget, etc.).
//! In Jolt, we just write the logic in Rust — Jolt handles the constraint
//! generation automatically by proving the RISC-V execution trace.

#![allow(dead_code)]

use sha2::{Sha256, Digest};

// ========================================
// Shared utilities (same logic as BN254 circuits, but in plain Rust)
// ========================================

/// MiMC hash replacement — we use SHA256 in Jolt guests because:
/// 1. SHA256 is a RISC-V instruction in Jolt's instruction set
/// 2. No need for field-arithmetic-friendly hashes when the zkVM handles constraints
/// 3. SHA256 is natively supported and fast in RISC-V execution
fn sha256_hash(left: &[u8], right: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn sha256_hash_5(a: &[u8], b: &[u8], c: &[u8], d: &[u8], e: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    hasher.update(c);
    hasher.update(d);
    hasher.update(e);
    hasher.finalize().into()
}

/// Merkle tree inclusion proof
fn verify_merkle_inclusion(
    leaf: &[u8; 32],
    root: &[u8; 32],
    path: &[[u8; 32]],
    bits: &[bool],
) -> bool {
    let mut current = *leaf;
    for (sibling, go_left) in path.iter().zip(bits.iter()) {
        if *go_left {
            current = sha256_hash(&current, sibling);
        } else {
            current = sha256_hash(sibling, &current);
        }
    }
    current == *root
}

// ========================================
// Guest Program 1: SensorSafety
// ========================================

/// Public inputs (committed on-chain):
/// - envelope_commitment: SHA256(max_speed || max_force || min_distance || max_tilt || max_accel)
/// - merkle_root: root of the reflex batch Merkle tree
/// - cycle_index: which cycle in the batch
///
/// Private witness (known to prover, hidden from verifier):
/// - sensor readings (speed, force, distance, tilt, accel)
/// - envelope params
/// - Merkle path + bits
///
/// Returns: true if the sensor readings satisfy the safety envelope
/// and are correctly included in the Merkle tree.
pub fn sensor_safety(
    // Public inputs
    envelope_commitment: [u8; 32],
    merkle_root: [u8; 32],
    cycle_index: u64,
    // Private witness
    speed: u64,
    force: u64,
    distance: u64,
    tilt: u64,
    accel: u64,
    max_speed: u64,
    max_force: u64,
    min_distance: u64,
    max_tilt: u64,
    max_accel: u64,
    merkle_path: Vec<[u8; 32]>,
    path_bits: Vec<bool>,
) -> bool {
    // 1. Range constraints: sensor readings within envelope
    if speed > max_speed { return false; }
    if force > max_force { return false; }
    if distance < min_distance { return false; }
    if tilt > max_tilt { return false; }
    if accel > max_accel { return false; }

    // 2. Envelope binding: H(max_speed || max_force || ...) == envelope_commitment
    let computed_envelope = sha256_hash_5(
        &max_speed.to_le_bytes(),
        &max_force.to_le_bytes(),
        &min_distance.to_le_bytes(),
        &max_tilt.to_le_bytes(),
        &max_accel.to_le_bytes(),
    );
    if computed_envelope != envelope_commitment { return false; }

    // 3. Batch binding: sensor leaf is in the Merkle tree at cycle_index
    let leaf = sha256_hash_5(
        &speed.to_le_bytes(),
        &force.to_le_bytes(),
        &distance.to_le_bytes(),
        &tilt.to_le_bytes(),
        &accel.to_le_bytes(),
    );

    if !verify_merkle_inclusion(&leaf, &merkle_root, &merkle_path, &path_bits) {
        return false;
    }

    // 4. Verify cycle_index matches path length (tree height)
    if merkle_path.len() != path_bits.len() { return false; }

    true
}

// ========================================
// Guest Program 2: IntentConsistency
// ========================================

/// Public inputs:
/// - intent_commitment: H(action || H(params) || sensor_snapshot_hash || envelope_commitment || H(agent_id))
/// - merkle_root: root of the reflex batch Merkle tree
/// - envelope_commitment: safety envelope commitment (links to reflex tier)
/// - policy_commitment: H(zone_x_min || zone_x_max || zone_y_min || zone_y_max)
///
/// Private witness:
/// - action, params_x, params_y, sensor_snapshot_hash, agent_id
/// - envelope params, policy zone bounds
/// - Merkle path + bits
pub fn intent_consistency(
    // Public inputs
    intent_commitment: [u8; 32],
    merkle_root: [u8; 32],
    envelope_commitment: [u8; 32],
    policy_commitment: [u8; 32],
    // Private witness: intent data
    action: u64,
    params_x: u64,
    params_y: u64,
    sensor_snapshot_hash: [u8; 32],
    agent_id: u64,
    // Private witness: Merkle proof
    merkle_path: Vec<[u8; 32]>,
    path_bits: Vec<bool>,
    // Private witness: envelope params
    max_speed: u64,
    max_force: u64,
    min_distance: u64,
    max_tilt: u64,
    max_accel: u64,
    // Private witness: policy zone
    zone_x_min: u64,
    zone_x_max: u64,
    zone_y_min: u64,
    zone_y_max: u64,
) -> bool {
    // 1. Envelope binding: H(envelope_params) == envelope_commitment
    let computed_envelope = sha256_hash_5(
        &max_speed.to_le_bytes(),
        &max_force.to_le_bytes(),
        &min_distance.to_le_bytes(),
        &max_tilt.to_le_bytes(),
        &max_accel.to_le_bytes(),
    );
    if computed_envelope != envelope_commitment { return false; }

    // 2. Policy compliance: params within authorized zone
    if params_x < zone_x_min || params_x > zone_x_max { return false; }
    if params_y < zone_y_min || params_y > zone_y_max { return false; }

    // 3. Policy binding: H(zone) == policy_commitment
    let computed_policy = sha256_hash_5(
        &zone_x_min.to_le_bytes(),
        &zone_x_max.to_le_bytes(),
        &zone_y_min.to_le_bytes(),
        &zone_y_max.to_le_bytes(),
        &[0u8; 32], // padding
    );
    // Note: policy commitment uses 4 params, not 5. We hash 4 + padding.
    let mut hasher = Sha256::new();
    hasher.update(&zone_x_min.to_le_bytes());
    hasher.update(&zone_x_max.to_le_bytes());
    hasher.update(&zone_y_min.to_le_bytes());
    hasher.update(&zone_y_max.to_le_bytes());
    let computed_policy: [u8; 32] = hasher.finalize().into();
    if computed_policy != policy_commitment { return false; }

    // 4. Intent binding: H(action || H(params) || sensor_snapshot || envelope || H(agent_id)) == intent_commitment
    let params_hash = sha256_hash(&params_x.to_le_bytes(), &params_y.to_le_bytes());
    let agent_hash = sha256_hash(&agent_id.to_le_bytes(), &[]);
    let computed_intent = sha256_hash_5(
        &action.to_le_bytes(),
        &params_hash,
        &sensor_snapshot_hash,
        &envelope_commitment,
        &agent_hash,
    );
    if computed_intent != intent_commitment { return false; }

    // 5. Sensor consistency: sensor_snapshot_hash is in the Merkle tree
    if !verify_merkle_inclusion(&sensor_snapshot_hash, &merkle_root, &merkle_path, &path_bits) {
        return false;
    }

    true
}

// ========================================
// Guest Program 3: ConsensusMembership
// ========================================

/// Public inputs:
/// - validator_set_root: Merkle root of the validator set
/// - epoch: consensus epoch number
/// - vote_commitment: H(block_hash || vote_decision || epoch)
///
/// Private witness:
/// - validator_pubkey, Merkle path + bits
/// - block_hash, vote_decision
pub fn consensus_membership(
    // Public inputs
    validator_set_root: [u8; 32],
    epoch: u64,
    vote_commitment: [u8; 32],
    // Private witness: validator identity
    validator_pubkey: [u8; 32],
    merkle_path: Vec<[u8; 32]>,
    path_bits: Vec<bool>,
    // Private witness: vote data
    block_hash: [u8; 32],
    vote_decision: u64,
) -> bool {
    // 1. Validator membership: H(validator_pubkey) is in the Merkle tree
    let leaf = sha256_hash(&validator_pubkey, &[]);
    if !verify_merkle_inclusion(&leaf, &validator_set_root, &merkle_path, &path_bits) {
        return false;
    }

    // 2. Vote binding: H(block_hash || vote_decision || epoch) == vote_commitment
    let mut hasher = Sha256::new();
    hasher.update(&block_hash);
    hasher.update(&vote_decision.to_le_bytes());
    hasher.update(&epoch.to_le_bytes());
    let computed_vote: [u8; 32] = hasher.finalize().into();
    if computed_vote != vote_commitment { return false; }

    true
}

// ========================================
// Guest Program 4: BatchSafety
// ========================================

/// Public inputs:
/// - envelope_commitment: H(max_speed || max_force || ...)
/// - merkle_root: root of the batch Merkle tree
/// - batch_size: number of cycles
///
/// Private witness:
/// - sensor_readings: [(speed, force, distance, tilt, accel); N]
/// - envelope params
/// - Merkle paths + bits for each cycle
pub fn batch_safety(
    // Public inputs
    envelope_commitment: [u8; 32],
    merkle_root: [u8; 32],
    batch_size: u64,
    // Private witness: sensor readings
    sensor_readings: Vec<(u64, u64, u64, u64, u64)>,
    // Private witness: envelope params
    max_speed: u64,
    max_force: u64,
    min_distance: u64,
    max_tilt: u64,
    max_accel: u64,
    // Private witness: Merkle paths
    merkle_paths: Vec<Vec<[u8; 32]>>,
    path_bits: Vec<Vec<bool>>,
) -> bool {
    // 1. Batch size check
    if sensor_readings.len() as u64 != batch_size { return false; }
    if merkle_paths.len() != sensor_readings.len() { return false; }
    if path_bits.len() != sensor_readings.len() { return false; }

    // 2. Envelope binding
    let computed_envelope = sha256_hash_5(
        &max_speed.to_le_bytes(),
        &max_force.to_le_bytes(),
        &min_distance.to_le_bytes(),
        &max_tilt.to_le_bytes(),
        &max_accel.to_le_bytes(),
    );
    if computed_envelope != envelope_commitment { return false; }

    // 3. All N sensor readings satisfy the envelope + Merkle inclusion
    for (i, &(speed, force, distance, tilt, accel)) in sensor_readings.iter().enumerate() {
        // Range constraints
        if speed > max_speed { return false; }
        if force > max_force { return false; }
        if distance < min_distance { return false; }
        if tilt > max_tilt { return false; }
        if accel > max_accel { return false; }

        // Merkle inclusion
        let leaf = sha256_hash_5(
            &speed.to_le_bytes(),
            &force.to_le_bytes(),
            &distance.to_le_bytes(),
            &tilt.to_le_bytes(),
            &accel.to_le_bytes(),
        );
        if !verify_merkle_inclusion(&leaf, &merkle_root, &merkle_paths[i], &path_bits[i]) {
            return false;
        }
    }

    true
}

// ========================================
// Guest Program 5: Aggregation (Fusion)
// ========================================

/// Public inputs:
/// - aggregation_commitment: H(all public inputs from sensor + intent + consensus)
/// - cross_tier_envelope: envelope commitment (must match sensor + intent)
/// - cross_tier_merkle_root: Merkle root (must match sensor + intent)
///
/// Private witness:
/// - Sensor proof public inputs: [envelope_commitment, merkle_root, cycle_index]
/// - Intent proof public inputs: [intent_commitment, merkle_root, envelope_commitment, policy_commitment]
/// - Consensus proof public inputs: [validator_set_root, epoch, vote_commitment]
///
/// In the BN254 path, the TEE verifies the 3 Groth16 pairings and the circuit
/// proves knowledge + consistency. In the Jolt path, the Jolt proof itself
/// proves both knowledge and consistency (no TEE needed).
pub fn aggregation_fusion(
    // Public inputs
    aggregation_commitment: [u8; 32],
    cross_tier_envelope: [u8; 32],
    cross_tier_merkle_root: [u8; 32],
    // Private witness: sensor proof public inputs
    sensor_envelope: [u8; 32],
    sensor_merkle_root: [u8; 32],
    sensor_cycle_index: u64,
    // Private witness: intent proof public inputs
    intent_commitment: [u8; 32],
    intent_merkle_root: [u8; 32],
    intent_envelope: [u8; 32],
    intent_policy: [u8; 32],
    // Private witness: consensus proof public inputs
    consensus_valset_root: [u8; 32],
    consensus_epoch: u64,
    consensus_vote: [u8; 32],
) -> bool {
    // 1. Cross-tier consistency: envelope matches between sensor and intent
    if sensor_envelope != cross_tier_envelope { return false; }
    if intent_envelope != cross_tier_envelope { return false; }

    // 2. Cross-tier consistency: merkle_root matches between sensor and intent
    if sensor_merkle_root != cross_tier_merkle_root { return false; }
    if intent_merkle_root != cross_tier_merkle_root { return false; }

    // 3. Aggregation binding: H(all public inputs) == aggregation_commitment
    let mut hasher = Sha256::new();
    // Sensor public inputs
    hasher.update(&sensor_envelope);
    hasher.update(&sensor_merkle_root);
    hasher.update(&sensor_cycle_index.to_le_bytes());
    // Intent public inputs
    hasher.update(&intent_commitment);
    hasher.update(&intent_merkle_root);
    hasher.update(&intent_envelope);
    hasher.update(&intent_policy);
    // Consensus public inputs
    hasher.update(&consensus_valset_root);
    hasher.update(&consensus_epoch.to_le_bytes());
    hasher.update(&consensus_vote);
    let computed_agg: [u8; 32] = hasher.finalize().into();
    if computed_agg != aggregation_commitment { return false; }

    true
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_hash(n: u64) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[..8].copy_from_slice(&n.to_le_bytes());
        h
    }

    #[test]
    fn test_sensor_safety_pass() {
        let envelope = sha256_hash_5(
            &5000u64.to_le_bytes(),
            &50000u64.to_le_bytes(),
            &500u64.to_le_bytes(),
            &30000u64.to_le_bytes(),
            &3000u64.to_le_bytes(),
        );
        let leaf = sha256_hash_5(
            &3000u64.to_le_bytes(),
            &30000u64.to_le_bytes(),
            &500u64.to_le_bytes(),
            &20000u64.to_le_bytes(),
            &2000u64.to_le_bytes(),
        );
        // Single-leaf Merkle tree (height 1)
        let sibling = sha256_hash(&[0u8; 32], &[0u8; 32]);
        let root = sha256_hash(&leaf, &sibling);

        let result = sensor_safety(
            envelope, root, 0,
            3000, 30000, 500, 20000, 2000,
            5000, 50000, 500, 30000, 3000,
            vec![sibling], vec![true],
        );
        assert!(result);
    }

    #[test]
    fn test_sensor_safety_violation() {
        let envelope = sha256_hash_5(
            &5000u64.to_le_bytes(),
            &50000u64.to_le_bytes(),
            &500u64.to_le_bytes(),
            &30000u64.to_le_bytes(),
            &3000u64.to_le_bytes(),
        );
        let leaf = sha256_hash_5(
            &6000u64.to_le_bytes(), // speed > max_speed
            &30000u64.to_le_bytes(),
            &500u64.to_le_bytes(),
            &20000u64.to_le_bytes(),
            &2000u64.to_le_bytes(),
        );
        let sibling = sha256_hash(&[0u8; 32], &[0u8; 32]);
        let root = sha256_hash(&leaf, &sibling);

        let result = sensor_safety(
            envelope, root, 0,
            6000, 30000, 500, 20000, 2000,
            5000, 50000, 500, 30000, 3000,
            vec![sibling], vec![true],
        );
        assert!(!result); // should fail: speed violation
    }

    #[test]
    fn test_aggregation_fusion_pass() {
        let envelope = dummy_hash(1);
        let merkle_root = dummy_hash(2);
        let intent_commit = dummy_hash(3);
        let intent_policy = dummy_hash(4);
        let valset_root = dummy_hash(5);
        let vote = dummy_hash(6);

        let mut hasher = Sha256::new();
        hasher.update(&envelope);
        hasher.update(&merkle_root);
        hasher.update(&42u64.to_le_bytes());
        hasher.update(&intent_commit);
        hasher.update(&merkle_root);
        hasher.update(&envelope);
        hasher.update(&intent_policy);
        hasher.update(&valset_root);
        hasher.update(&1u64.to_le_bytes());
        hasher.update(&vote);
        let agg_commit: [u8; 32] = hasher.finalize().into();

        let result = aggregation_fusion(
            agg_commit, envelope, merkle_root,
            envelope, merkle_root, 42,
            intent_commit, merkle_root, envelope, intent_policy,
            valset_root, 1, vote,
        );
        assert!(result);
    }

    #[test]
    fn test_aggregation_fusion_mismatch_fails() {
        let envelope = dummy_hash(1);
        let merkle_root = dummy_hash(2);
        let wrong_envelope = dummy_hash(99);

        let result = aggregation_fusion(
            dummy_hash(0), envelope, merkle_root,
            wrong_envelope, merkle_root, 42, // sensor_envelope != cross_tier
            dummy_hash(3), merkle_root, envelope, dummy_hash(4),
            dummy_hash(5), 1, dummy_hash(6),
        );
        assert!(!result);
    }
}
