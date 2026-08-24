//! L1 Perpetual Memory — Merkle-verified recall of past reflex cycles.
//!
//! Every reflex cycle is hashed and anchored on-chain via a Merkle root.
//! This module turns that audit trail into a live memory: a robot can ask
//! "has any robot ever been in a state like this one, and what happened next?"
//! and get a cryptographically proven answer in under 12ms.
//!
//! The key property: reading a memory requires NO consensus round-trip.
//! Roots are pre-fetched at the coordination layer (L3, ~300ms) and cached
//! locally. A Merkle inclusion proof verifies against the cached root in
//! microseconds (~20 SHA-256 ops). Consensus is never in the critical path.
//!
//! Cold misses degrade gracefully — if the state is unseen locally, the
//! caller falls back to conservative L0 control and queues an async fetch.

use crate::merkle::{compute_merkle_root, verify_merkle_proof};
use crate::state::PhysicsState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Maximum number of roots held in the rolling cache.
const ROOT_CACHE_CAPACITY: usize = 64;

// ---------------------------------------------------------------------------
// L1: State Feature Extraction
// ---------------------------------------------------------------------------

/// Compact, normalized feature vector extracted from a PhysicsState.
///
/// This is what similarity is measured over. Dimensionality is fixed so that
/// distance is well-defined regardless of joint count.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateFeatures {
    /// Normalized joint positions (mean)
    pub joint_pos_mean: f64,
    /// Normalized joint velocities (mean)
    pub joint_vel_mean: f64,
    /// Normalized joint torques (mean)
    pub joint_torque_mean: f64,
    /// Maximum joint torque (absolute)
    pub joint_torque_max: f64,
    /// IMU linear acceleration magnitude
    pub accel_magnitude: f64,
    /// IMU angular velocity magnitude
    pub gyro_magnitude: f64,
    /// Number of active contacts (normalized)
    pub contact_count: f64,
    /// Maximum contact force (normalized)
    pub contact_force_max: f64,
    /// COM height (z)
    pub com_height: f64,
    /// COM speed
    pub com_speed: f64,
    /// Tilt (degrees)
    pub tilt: f64,
    /// Forward speed
    pub speed: f64,
}

impl StateFeatures {
    /// Extract a feature vector from a physics state.
    pub fn from_state(state: &PhysicsState) -> Self {
        let n_joints = state.joints.len().max(1) as f64;

        let joint_pos_mean =
            state.joints.iter().map(|j| j.position).sum::<f64>() / n_joints;
        let joint_vel_mean =
            state.joints.iter().map(|j| j.velocity).sum::<f64>() / n_joints;
        let joint_torque_mean =
            state.joints.iter().map(|j| j.torque).sum::<f64>() / n_joints;
        let joint_torque_max = state
            .joints
            .iter()
            .map(|j| j.torque.abs())
            .fold(0.0f64, f64::max);

        let accel = state.imu.accel;
        let accel_magnitude =
            (accel[0] * accel[0] + accel[1] * accel[1] + accel[2] * accel[2]).sqrt();

        let gyro = state.imu.gyro;
        let gyro_magnitude =
            (gyro[0] * gyro[0] + gyro[1] * gyro[1] + gyro[2] * gyro[2]).sqrt();

        let contact_count = state.contacts.len() as f64;
        let contact_force_max = state
            .contacts
            .iter()
            .map(|c| c.normal_force)
            .fold(0.0f64, f64::max);

        Self {
            joint_pos_mean,
            joint_vel_mean,
            joint_torque_mean,
            joint_torque_max,
            accel_magnitude,
            gyro_magnitude,
            contact_count,
            contact_force_max,
            com_height: state.com_position[2],
            com_speed: (state.com_velocity[0].powi(2)
                + state.com_velocity[1].powi(2)
                + state.com_velocity[2].powi(2))
            .sqrt(),
            tilt: state.sensors.tilt_degrees,
            speed: state.sensors.speed,
        }
    }

    /// Compute weighted Euclidean distance to another feature vector.
    ///
    /// Weights reflect which dimensions are most predictive of safety:
    /// tilt, torque, and contact force dominate.
    pub fn distance(&self, other: &StateFeatures) -> f64 {
        let d = self.vector();
        let o = other.vector();
        let w = Self::weights();

        d.iter()
            .zip(o.iter())
            .zip(w.iter())
            .map(|((a, b), weight)| weight * (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// The raw 12-dimensional vector.
    fn vector(&self) -> [f64; 12] {
        [
            self.joint_pos_mean,
            self.joint_vel_mean,
            self.joint_torque_mean,
            self.joint_torque_max,
            self.accel_magnitude,
            self.gyro_magnitude,
            self.contact_count,
            self.contact_force_max,
            self.com_height,
            self.com_speed,
            self.tilt,
            self.speed,
        ]
    }

    /// Per-dimension weights. Tilt, torque, and contact force are the
    /// highest-weighted because they most strongly predict safety violations.
    fn weights() -> [f64; 12] {
        [
            0.5, // joint_pos_mean
            0.5, // joint_vel_mean
            1.0, // joint_torque_mean
            2.0, // joint_torque_max
            0.5, // accel_magnitude
            0.5, // gyro_magnitude
            0.3, // contact_count
            1.5, // contact_force_max
            1.0, // com_height
            0.5, // com_speed
            3.0, // tilt — highest weight, most predictive of falls
            0.5, // speed
        ]
    }
}

// ---------------------------------------------------------------------------
// L1: Memory Record and Hit
// ---------------------------------------------------------------------------

/// A single remembered reflex cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// The feature vector (for similarity search)
    pub features: StateFeatures,
    /// The SHA-256 cycle hash (leaf in the Merkle tree)
    pub cycle_hash: String,
    /// Which batch this cycle belongs to
    pub batch_id: String,
    /// The Merkle root of that batch (anchored on-chain)
    pub batch_root: String,
    /// Index of this leaf within the batch
    pub leaf_index: usize,
    /// All leaf hashes in the batch (needed to reconstruct proof)
    pub batch_leaves: Vec<String>,
    /// The truth verdict this cycle was part of (if adjudicated)
    pub verdict: Option<String>,
    /// Which robot produced this cycle
    pub robot_id: String,
    /// Timestamp (ms)
    pub timestamp_ms: u64,
    /// Which invariants were violated (empty = clean)
    pub violated_invariants: Vec<String>,
}

/// A query result: one similar past state with its proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryHit {
    /// The matched record
    pub record: MemoryRecord,
    /// Feature-space distance to the query
    pub distance: f64,
    /// Merkle inclusion proof for the cycle hash
    pub merkle_proof: Vec<String>,
    /// Whether the proof verified against a cached root
    pub proof_verified: bool,
}

// ---------------------------------------------------------------------------
// L1: Memory Index
// ---------------------------------------------------------------------------

/// Local index over remembered physics states.
///
/// Uses brute-force kNN over feature vectors. For the memory sizes expected
/// on a single robot (thousands to tens of thousands of batches), brute force
/// with a feature-vector prefilter is sufficient and exact. A future HNSW or
/// LSH index can replace this without changing the API.
#[derive(Clone, Debug, Default)]
pub struct MemoryIndex {
    records: Vec<MemoryRecord>,
}

impl MemoryIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a batch of cycles to the index.
    ///
    /// Each state in the batch becomes a separate MemoryRecord, all sharing
    /// the same batch_id and batch_root.
    pub fn add_batch(
        &mut self,
        batch_id: &str,
        robot_id: &str,
        states: &[PhysicsState],
        cycle_hashes: &[String],
        verdict: Option<String>,
        violated_invariants: Vec<String>,
    ) {
        if states.len() != cycle_hashes.len() {
            return; // length mismatch — do not index partial batches
        }

        let batch_root = compute_merkle_root(cycle_hashes);

        for (i, (state, hash)) in states.iter().zip(cycle_hashes.iter()).enumerate() {
            self.records.push(MemoryRecord {
                features: StateFeatures::from_state(state),
                cycle_hash: hash.clone(),
                batch_id: batch_id.to_string(),
                batch_root: batch_root.clone(),
                leaf_index: i,
                batch_leaves: cycle_hashes.to_vec(),
                verdict: verdict.clone(),
                robot_id: robot_id.to_string(),
                timestamp_ms: state.timestamp_ms,
                violated_invariants: if violated_invariants.is_empty() {
                    Vec::new()
                } else {
                    violated_invariants.clone()
                },
            });
        }
    }

    /// Add a single record directly (for cross-fleet memory import).
    pub fn add_record(&mut self, record: MemoryRecord) {
        self.records.push(record);
    }

    /// Find all records within `epsilon` feature distance of the query state.
    ///
    /// Returns hits sorted by distance (nearest first).
    pub fn query(&self, state: &PhysicsState, epsilon: f64) -> Vec<MemoryHit> {
        let query_features = StateFeatures::from_state(state);

        let mut hits: Vec<MemoryHit> = self
            .records
            .iter()
            .filter_map(|record| {
                let distance = query_features.distance(&record.features);
                if distance <= epsilon {
                    let merkle_proof = crate::merkle::compute_merkle_proof(
                        &record.batch_leaves,
                        record.leaf_index,
                    );
                    Some(MemoryHit {
                        record: record.clone(),
                        distance,
                        merkle_proof,
                        proof_verified: false, // set by MemoryFetch after root check
                    })
                } else {
                    None
                }
            })
            .collect();

        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }

    /// Find the k nearest neighbors regardless of distance.
    pub fn query_knn(&self, state: &PhysicsState, k: usize) -> Vec<MemoryHit> {
        let query_features = StateFeatures::from_state(state);

        let mut hits: Vec<MemoryHit> = self
            .records
            .iter()
            .map(|record| {
                let distance = query_features.distance(&record.features);
                let merkle_proof = crate::merkle::compute_merkle_proof(
                    &record.batch_leaves,
                    record.leaf_index,
                );
                MemoryHit {
                    record: record.clone(),
                    distance,
                    merkle_proof,
                    proof_verified: false,
                }
            })
            .collect();

        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
        hits
    }

    /// Number of records in the index.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// All records (for export / cross-fleet sync).
    pub fn records(&self) -> &[MemoryRecord] {
        &self.records
    }
}

// ---------------------------------------------------------------------------
// L1: Root Cache
// ---------------------------------------------------------------------------

/// Rolling cache of consensus-finalized Merkle roots.
///
/// Roots arrive from the coordination layer (L3) as batches are finalized.
/// The cache holds the most recent roots so that memory fetches can verify
/// proofs locally without any network round-trip.
///
/// When the cache is empty or the root is not found, the memory fetch
/// degrades gracefully: it still returns the hit but marks it unverified.
#[derive(Clone, Debug)]
pub struct RootCache {
    /// Rolling window of (root, height) pairs, newest last.
    roots: VecDeque<(String, u64)>,
    capacity: usize,
}

impl Default for RootCache {
    fn default() -> Self {
        Self::new(ROOT_CACHE_CAPACITY)
    }
}

impl RootCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            roots: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a finalized root. If the cache is full, the oldest root is evicted.
    pub fn push(&mut self, root: String, height: u64) {
        if self.roots.len() >= self.capacity {
            self.roots.pop_front();
        }
        self.roots.push_back((root, height));
    }

    /// Check whether a root is in the cache.
    pub fn contains(&self, root: &str) -> bool {
        self.roots.iter().any(|(r, _)| r == root)
    }

    /// Get the latest cached root, if any.
    pub fn latest(&self) -> Option<&(String, u64)> {
        self.roots.back()
    }

    /// Number of cached roots.
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

// ---------------------------------------------------------------------------
// L1: Memory Fetch — the 12ms query API
// ---------------------------------------------------------------------------

/// The L1 memory fetch: the 12-millisecond query.
///
/// Combines the local MemoryIndex with the RootCache to return similar past
/// states, each with a Merkle proof verified against a cached root.
///
/// This is the hot path called from the reflex loop. It must never block,
/// never hit the network, and never fail — it degrades to unverified hits
/// when the root is not cached.
pub struct MemoryFetch {
    index: MemoryIndex,
    root_cache: RootCache,
}

impl MemoryFetch {
    pub fn new(index: MemoryIndex, root_cache: RootCache) -> Self {
        Self { index, root_cache }
    }

    /// Access the underlying index (for adding batches).
    pub fn index_mut(&mut self) -> &mut MemoryIndex {
        &mut self.index
    }

    /// Access the underlying root cache (for pushing finalized roots).
    pub fn root_cache_mut(&mut self) -> &mut RootCache {
        &mut self.root_cache
    }

    /// Query: "has any robot been in a state within epsilon of this one?"
    ///
    /// Returns hits sorted by distance, each with a verified (or unverified)
    /// Merkle proof. Target latency: p99 < 12ms on CM5-class hardware.
    pub fn query(&self, state: &PhysicsState, epsilon: f64) -> Vec<MemoryHit> {
        let mut hits = self.index.query(state, epsilon);

        for hit in &mut hits {
            hit.proof_verified = self.verify_hit(hit);
        }

        hits
    }

    /// Query k nearest neighbors.
    pub fn query_knn(&self, state: &PhysicsState, k: usize) -> Vec<MemoryHit> {
        let mut hits = self.index.query_knn(state, k);

        for hit in &mut hits {
            hit.proof_verified = self.verify_hit(hit);
        }

        hits
    }

    /// Check whether the current state matches any past red-verdict states.
    ///
    /// This is the reflex-loop guard: "am I about to repeat a known mistake?"
    pub fn has_red_match(&self, state: &PhysicsState, epsilon: f64) -> Option<MemoryHit> {
        self.query(state, epsilon)
            .into_iter()
            .find(|hit| hit.record.verdict.as_deref() == Some("red"))
    }

    /// Verify a hit's Merkle proof against the cached root.
    fn verify_hit(&self, hit: &MemoryHit) -> bool {
        // The batch root must be in the cache for verification to succeed.
        if !self.root_cache.contains(&hit.record.batch_root) {
            return false;
        }

        let computed_root = verify_merkle_proof(
            &hit.record.cycle_hash,
            hit.record.leaf_index,
            &hit.merkle_proof,
        );

        computed_root == hit.record.batch_root
    }

    /// Total records in the index.
    pub fn record_count(&self) -> usize {
        self.index.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::QuadrupedBackend;
    use crate::simulator::PhysicsSimulator;
    use crate::state::{ContactInfo, ImuReading, JointState, SensorReadings};
    use sha2::{Digest, Sha256};

    /// Helper: create a minimal physics state for testing.
    fn make_state(
        timestamp: u64,
        tilt: f64,
        speed: f64,
        max_force: f64,
        torque: f64,
    ) -> PhysicsState {
        PhysicsState {
            timestamp_ms: timestamp,
            joints: (0..15)
                .map(|i| JointState {
                    name: format!("joint_{}", i),
                    position: 0.1 * i as f64,
                    velocity: 0.01 * i as f64,
                    torque,
                })
                .collect(),
            imu: ImuReading {
                accel: [0.0, 0.0, -9.81],
                gyro: [0.0, 0.0, 0.0],
            },
            contacts: vec![ContactInfo {
                body_part: "fl_foot".into(),
                normal_force: max_force,
                point: [0.0, 0.0, 0.0],
            }],
            sensors: SensorReadings {
                speed,
                max_force,
                min_distance: 1.0,
                tilt_degrees: tilt,
                acceleration: 0.0,
            },
            com_position: [0.0, 0.0, 0.35],
            com_velocity: [speed, 0.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0],
        }
    }

    fn hash_of(data: &[u8]) -> String {
        hex::encode(Sha256::digest(data))
    }

    // --- StateFeatures ---

    #[test]
    fn test_features_extraction() {
        let state = make_state(0, 5.0, 1.0, 10.0, 0.5);
        let f = StateFeatures::from_state(&state);

        assert_eq!(f.tilt, 5.0);
        assert_eq!(f.speed, 1.0);
        assert_eq!(f.contact_force_max, 10.0);
        assert_eq!(f.joint_torque_max, 0.5);
        assert_eq!(f.contact_count, 1.0);
    }

    #[test]
    fn test_features_distance_identical() {
        let state = make_state(0, 5.0, 1.0, 10.0, 0.5);
        let f1 = StateFeatures::from_state(&state);
        let f2 = StateFeatures::from_state(&state);
        assert!(f1.distance(&f2) < 1e-10, "identical states should have ~0 distance");
    }

    #[test]
    fn test_features_distance_tilt_weighted() {
        let state_a = make_state(0, 5.0, 1.0, 10.0, 0.5);
        let state_b = make_state(0, 35.0, 1.0, 10.0, 0.5); // only tilt differs
        let state_c = make_state(0, 5.0, 2.0, 10.0, 0.5);  // only speed differs

        let f_a = StateFeatures::from_state(&state_a);
        let f_b = StateFeatures::from_state(&state_b);
        let f_c = StateFeatures::from_state(&state_c);

        let d_tilt = f_a.distance(&f_b);
        let d_speed = f_a.distance(&f_c);

        assert!(
            d_tilt > d_speed,
            "tilt difference should dominate due to weight: tilt={} vs speed={}",
            d_tilt,
            d_speed
        );
    }

    // --- MemoryIndex ---

    #[test]
    fn test_index_add_batch() {
        let mut index = MemoryIndex::new();
        let states = vec![
            make_state(0, 5.0, 1.0, 10.0, 0.5),
            make_state(1, 6.0, 1.1, 11.0, 0.6),
        ];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, Some("green".into()), vec![]);
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_index_query_finds_similar() {
        let mut index = MemoryIndex::new();
        let states = vec![
            make_state(0, 5.0, 1.0, 10.0, 0.5),
            make_state(1, 30.0, 2.0, 50.0, 5.0), // very different
        ];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, Some("green".into()), vec![]);

        let query_state = make_state(100, 5.5, 1.05, 10.5, 0.55);
        let hits = index.query(&query_state, 5.0);

        assert!(!hits.is_empty(), "should find the similar state");
        assert_eq!(hits[0].record.batch_id, "batch_001");
        assert!(hits[0].distance < 5.0);
    }

    #[test]
    fn test_index_query_miss_returns_empty() {
        let mut index = MemoryIndex::new();
        let states = vec![make_state(0, 5.0, 1.0, 10.0, 0.5)];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, None, vec![]);

        let far_state = make_state(100, 60.0, 5.0, 200.0, 50.0);
        let hits = index.query(&far_state, 1.0);
        assert!(hits.is_empty(), "distant state should not match with tight epsilon");
    }

    #[test]
    fn test_index_knn() {
        let mut index = MemoryIndex::new();
        let states: Vec<PhysicsState> = (0..10)
            .map(|i| make_state(i, 5.0 + i as f64, 1.0, 10.0, 0.5))
            .collect();
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, None, vec![]);

        let query_state = make_state(100, 7.0, 1.0, 10.0, 0.5);
        let hits = index.query_knn(&query_state, 3);

        assert_eq!(hits.len(), 3);
        // Nearest should be the one with tilt=7 (index 2)
        assert!(hits[0].distance <= hits[1].distance);
        assert!(hits[1].distance <= hits[2].distance);
    }

    // --- RootCache ---

    #[test]
    fn test_root_cache_push_and_contains() {
        let mut cache = RootCache::new(4);
        cache.push("root_a".into(), 100);
        cache.push("root_b".into(), 101);

        assert!(cache.contains("root_a"));
        assert!(cache.contains("root_b"));
        assert!(!cache.contains("root_c"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_root_cache_eviction() {
        let mut cache = RootCache::new(2);
        cache.push("root_a".into(), 100);
        cache.push("root_b".into(), 101);
        cache.push("root_c".into(), 102); // evicts root_a

        assert!(!cache.contains("root_a"), "oldest root should be evicted");
        assert!(cache.contains("root_b"));
        assert!(cache.contains("root_c"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_root_cache_latest() {
        let mut cache = RootCache::new(4);
        assert!(cache.latest().is_none());

        cache.push("root_a".into(), 100);
        cache.push("root_b".into(), 101);

        let latest = cache.latest().unwrap();
        assert_eq!(latest.0, "root_b");
        assert_eq!(latest.1, 101);
    }

    // --- MemoryFetch ---

    #[test]
    fn test_fetch_query_with_verified_proof() {
        let mut index = MemoryIndex::new();
        let states = vec![make_state(0, 5.0, 1.0, 10.0, 0.5)];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, Some("green".into()), vec![]);

        let batch_root = compute_merkle_root(&hashes);
        let mut cache = RootCache::new(8);
        cache.push(batch_root.clone(), 100);

        let fetch = MemoryFetch::new(index, cache);

        let query_state = make_state(100, 5.1, 1.01, 10.1, 0.51);
        let hits = fetch.query(&query_state, 5.0);

        assert_eq!(hits.len(), 1);
        assert!(hits[0].proof_verified, "proof should verify against cached root");
    }

    #[test]
    fn test_fetch_unverified_when_root_not_cached() {
        let mut index = MemoryIndex::new();
        let states = vec![make_state(0, 5.0, 1.0, 10.0, 0.5)];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        index.add_batch("batch_001", "dogzilla-001", &states, &hashes, None, vec![]);

        let cache = RootCache::new(8); // empty cache — root not present

        let fetch = MemoryFetch::new(index, cache);

        let query_state = make_state(100, 5.1, 1.01, 10.1, 0.51);
        let hits = fetch.query(&query_state, 5.0);

        assert_eq!(hits.len(), 1);
        assert!(
            !hits[0].proof_verified,
            "proof should be unverified when root is not cached"
        );
    }

    #[test]
    fn test_fetch_has_red_match() {
        let mut index = MemoryIndex::new();
        let states = vec![
            make_state(0, 5.0, 1.0, 10.0, 0.5),
            make_state(1, 30.0, 1.0, 10.0, 5.0),
        ];
        let hashes: Vec<String> = states
            .iter()
            .map(|s| hash_of(&s.timestamp_ms.to_le_bytes()))
            .collect();

        // First state green, second state red
        index.add_batch("batch_001", "dogzilla-001", &states[..1], &hashes[..1], Some("green".into()), vec![]);
        index.add_batch("batch_002", "dogzilla-001", &states[1..], &hashes[1..], Some("red".into()), vec!["max_tilt".into()]);

        let root1 = compute_merkle_root(&hashes[..1]);
        let root2 = compute_merkle_root(&hashes[1..]);

        let mut cache = RootCache::new(8);
        cache.push(root1, 100);
        cache.push(root2, 101);

        let fetch = MemoryFetch::new(index, cache);

        // Query near the green state — no red match
        let safe_state = make_state(100, 5.1, 1.01, 10.1, 0.51);
        assert!(fetch.has_red_match(&safe_state, 3.0).is_none());

        // Query near the red state — red match
        let danger_state = make_state(100, 29.0, 1.0, 10.0, 5.0);
        let red_hit = fetch.has_red_match(&danger_state, 15.0);
        assert!(red_hit.is_some(), "should find red match near dangerous state");

        let hit = red_hit.unwrap();
        assert_eq!(hit.record.verdict.as_deref(), Some("red"));
        assert!(hit.record.violated_invariants.contains(&"max_tilt".to_string()));
    }

    #[test]
    fn test_fetch_multiple_robots() {
        let mut index = MemoryIndex::new();

        // Robot A memory
        let states_a = vec![make_state(0, 30.0, 1.0, 10.0, 5.0)];
        let hashes_a: Vec<String> = states_a.iter().map(|s| hash_of(&s.timestamp_ms.to_le_bytes())).collect();
        index.add_batch("batch_a", "dogzilla-001", &states_a, &hashes_a, Some("red".into()), vec!["max_tilt".into()]);

        // Robot B memory
        let states_b = vec![make_state(0, 5.0, 1.0, 10.0, 0.5)];
        let hashes_b: Vec<String> = states_b.iter().map(|s| hash_of(&s.timestamp_ms.to_le_bytes())).collect();
        index.add_batch("batch_b", "dogzilla-002", &states_b, &hashes_b, Some("green".into()), vec![]);

        let root_a = compute_merkle_root(&hashes_a);
        let root_b = compute_merkle_root(&hashes_b);
        let mut cache = RootCache::new(8);
        cache.push(root_a, 100);
        cache.push(root_b, 101);

        let fetch = MemoryFetch::new(index, cache);

        // Robot B queries near Robot A's red state
        let danger = make_state(100, 29.0, 1.0, 10.0, 5.0);
        let hit = fetch.has_red_match(&danger, 15.0);

        assert!(hit.is_some(), "robot B should find robot A's red memory");
        assert_eq!(hit.unwrap().record.robot_id, "dogzilla-001");
    }

    #[test]
    fn test_fetch_proof_chain_integrity() {
        // Build a real batch with the quadruped backend
        let mut backend = QuadrupedBackend::new(
            "dogzilla-001".into(),
            Default::default(),
        );
        backend.reset();

        let mut states = Vec::new();
        let mut hashes = Vec::new();

        for _ in 0..8 {
            let state = backend.step(1);
            hashes.push(state.hash());
            states.push(state);
        }

        let mut index = MemoryIndex::new();
        index.add_batch("batch_real", "dogzilla-001", &states, &hashes, Some("green".into()), vec![]);

        let batch_root = compute_merkle_root(&hashes);
        let mut cache = RootCache::new(8);
        cache.push(batch_root, 100);

        let fetch = MemoryFetch::new(index, cache);

        // Query with the exact state from cycle 3
        let query_state = &states[3];
        let hits = fetch.query(query_state, 0.1);

        assert!(!hits.is_empty(), "exact state should match");
        let hit = &hits[0];
        assert!(hit.proof_verified, "proof should verify for exact match");
        assert_eq!(hit.record.cycle_hash, hashes[3]);
    }
}
