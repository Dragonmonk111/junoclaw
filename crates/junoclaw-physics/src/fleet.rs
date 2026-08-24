//! Cross-fleet memory sync — sharing L1 memory records across robots that
//! do not share an owner, with a trust-gated import that defends against
//! a hostile fleet trying to poison another fleet's memory.
//!
//! What actually crosses the wire in gossip sync is a `MemoryRecord`: a
//! feature vector, a cycle hash, a batch Merkle root, and the batch's full
//! leaf list — never the raw `PhysicsState`. This keeps the payload small
//! (bandwidth-realistic) and self-verifying (the importer recomputes the
//! Merkle root from the leaves rather than trusting the sender's claim).
//!
//! Trust gating ("redmark/slashing"): every contributor has a trust score
//! derived from its own reported green/red ratio. A contributor whose
//! score falls below the importer's threshold has its records rejected —
//! this is the fleet-level equivalent of a validator being slashed out of
//! a consensus set. `slash()` provides an explicit override for when a
//! contributor is caught fabricating a Merkle root (integrity failure),
//! which is a stronger signal than an honestly-reported red batch and
//! should never be conflated with it.

use crate::memory::{MemoryIndex, MemoryRecord};
use crate::merkle::compute_merkle_root;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-contributor reputation, derived purely from locally observed
/// outcomes — never trusted from the contributor's own self-report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributorStats {
    pub robot_id: String,
    pub batches_contributed: u64,
    pub green_batches: u64,
    pub red_batches: u64,
    /// Set by `FleetRegistry::slash` — an integrity failure (bad Merkle
    /// root, malformed leaves), distinct from an honestly-reported red
    /// verdict. Slashing overrides trust_score to 0 regardless of history.
    pub slashed: bool,
}

impl ContributorStats {
    fn new(robot_id: &str) -> Self {
        Self {
            robot_id: robot_id.to_string(),
            batches_contributed: 0,
            green_batches: 0,
            red_batches: 0,
            slashed: false,
        }
    }

    /// Trust score in [0, 1]. Honestly reporting a red (failure) batch is
    /// NOT penalized the same as fabricating data — a contributor that
    /// always tells the truth about its own failures is exactly the
    /// behavior this system wants to reward, not punish. New contributors
    /// start at a neutral 0.5 prior (neither trusted nor distrusted).
    pub fn trust_score(&self) -> f64 {
        if self.slashed {
            return 0.0;
        }
        let total = self.green_batches + self.red_batches;
        if total == 0 {
            return 0.5;
        }
        self.green_batches as f64 / total as f64
    }
}

/// Reasons a foreign record can be rejected during import.
#[derive(Clone, Debug, PartialEq)]
pub enum FleetRejection {
    /// The batch's Merkle root did not recompute from its own leaf list —
    /// the record is internally inconsistent or fabricated.
    RootMismatch { claimed: String, computed: String },
    /// The record's cycle hash is not actually present at its claimed
    /// leaf index within its own batch leaves.
    LeafMismatch,
    /// The contributor's trust score is below the importer's threshold.
    NotTrusted { robot_id: String, trust_score: f64 },
    /// The contributor has been explicitly slashed for a prior integrity failure.
    Slashed { robot_id: String },
}

/// Summary of a gossip sync attempt.
#[derive(Clone, Debug, Default)]
pub struct SyncSummary {
    pub accepted: usize,
    pub rejected: usize,
    pub rejections: Vec<FleetRejection>,
}

/// Local registry of fleet-wide contributor trust, gating what gets
/// merged into this robot's own `MemoryIndex`.
pub struct FleetRegistry {
    contributors: HashMap<String, ContributorStats>,
    min_trust_to_accept: f64,
}

impl FleetRegistry {
    pub fn new(min_trust_to_accept: f64) -> Self {
        Self {
            contributors: HashMap::new(),
            min_trust_to_accept,
        }
    }

    /// Record a batch this robot produced and verified locally (not
    /// imported) — the ground truth that builds this robot's own
    /// reputation as seen by others.
    pub fn record_local_batch(&mut self, robot_id: &str, verdict: Option<&str>) {
        let stats = self
            .contributors
            .entry(robot_id.to_string())
            .or_insert_with(|| ContributorStats::new(robot_id));
        stats.batches_contributed += 1;
        match verdict {
            Some("green") => stats.green_batches += 1,
            Some("red") => stats.red_batches += 1,
            _ => {}
        }
    }

    /// Explicitly slash a contributor for an integrity failure (e.g. a
    /// fabricated Merkle root caught during import). Distinct from an
    /// honestly-reported red batch, which does not slash.
    pub fn slash(&mut self, robot_id: &str) {
        let stats = self
            .contributors
            .entry(robot_id.to_string())
            .or_insert_with(|| ContributorStats::new(robot_id));
        stats.slashed = true;
    }

    /// Reinstate a previously slashed contributor (e.g. after manual
    /// governance review clears them).
    pub fn reinstate(&mut self, robot_id: &str) {
        if let Some(stats) = self.contributors.get_mut(robot_id) {
            stats.slashed = false;
        }
    }

    pub fn trust_score(&self, robot_id: &str) -> f64 {
        self.contributors
            .get(robot_id)
            .map(|s| s.trust_score())
            .unwrap_or(0.5)
    }

    pub fn is_trusted(&self, robot_id: &str) -> bool {
        self.trust_score(robot_id) >= self.min_trust_to_accept
    }

    pub fn contributor_stats(&self, robot_id: &str) -> Option<&ContributorStats> {
        self.contributors.get(robot_id)
    }

    /// Attempt to import a single foreign memory record into `local_index`.
    ///
    /// Verification order: integrity first (root/leaf consistency), then
    /// trust (contributor reputation) — a fabricated record from a
    /// trusted-looking robot_id is still rejected, and a well-formed
    /// record from an untrusted robot is still rejected.
    pub fn import_record(
        &mut self,
        local_index: &mut MemoryIndex,
        record: MemoryRecord,
    ) -> Result<(), FleetRejection> {
        let computed_root = compute_merkle_root(&record.batch_leaves);
        if computed_root != record.batch_root {
            return Err(FleetRejection::RootMismatch {
                claimed: record.batch_root.clone(),
                computed: computed_root,
            });
        }

        let leaf_ok = record
            .batch_leaves
            .get(record.leaf_index)
            .map(|h| h == &record.cycle_hash)
            .unwrap_or(false);
        if !leaf_ok {
            return Err(FleetRejection::LeafMismatch);
        }

        let stats = self
            .contributors
            .entry(record.robot_id.clone())
            .or_insert_with(|| ContributorStats::new(&record.robot_id));

        if stats.slashed {
            return Err(FleetRejection::Slashed {
                robot_id: record.robot_id.clone(),
            });
        }

        let trust = stats.trust_score();
        if trust < self.min_trust_to_accept {
            return Err(FleetRejection::NotTrusted {
                robot_id: record.robot_id.clone(),
                trust_score: trust,
            });
        }

        local_index.add_record(record);
        Ok(())
    }

    /// Gossip sync: attempt to import a batch of foreign records (e.g.
    /// received from a peer robot over the network) into `local_index`,
    /// applying integrity and trust checks to each independently.
    pub fn gossip_sync(
        &mut self,
        local_index: &mut MemoryIndex,
        foreign_records: Vec<MemoryRecord>,
    ) -> SyncSummary {
        let mut summary = SyncSummary::default();

        for record in foreign_records {
            match self.import_record(local_index, record) {
                Ok(()) => summary.accepted += 1,
                Err(reason) => {
                    summary.rejected += 1;
                    summary.rejections.push(reason);
                }
            }
        }

        summary
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{PhysicsSimulator, QuadrupedBackend, QuadrupedConfig};
    use crate::memory::StateFeatures;

    fn make_record(robot_id: &str, batch_leaves: Vec<String>, leaf_index: usize, verdict: Option<&str>) -> MemoryRecord {
        let mut sim = QuadrupedBackend::new(robot_id.to_string(), QuadrupedConfig::default());
        let state = sim.step(1);
        MemoryRecord {
            features: StateFeatures::from_state(&state),
            cycle_hash: batch_leaves[leaf_index].clone(),
            batch_id: "foreign_batch".to_string(),
            batch_root: compute_merkle_root(&batch_leaves),
            leaf_index,
            batch_leaves,
            verdict: verdict.map(|s| s.to_string()),
            robot_id: robot_id.to_string(),
            timestamp_ms: state.timestamp_ms,
            violated_invariants: vec![],
        }
    }

    fn sample_leaves(n: usize) -> Vec<String> {
        let mut sim = QuadrupedBackend::new("leaf-source".to_string(), QuadrupedConfig::default());
        (0..n).map(|_| sim.step(1).hash()).collect()
    }

    #[test]
    fn test_new_contributor_neutral_trust() {
        let registry = FleetRegistry::new(0.5);
        assert_eq!(registry.trust_score("unknown-robot"), 0.5);
        assert!(registry.is_trusted("unknown-robot"), "0.5 threshold should accept neutral prior");
    }

    #[test]
    fn test_import_accepts_well_formed_trusted_record() {
        let mut registry = FleetRegistry::new(0.4);
        let mut local_index = MemoryIndex::new();
        let leaves = sample_leaves(5);
        let record = make_record("robot-b", leaves, 2, Some("green"));

        let result = registry.import_record(&mut local_index, record);
        assert!(result.is_ok());
        assert_eq!(local_index.len(), 1);
    }

    #[test]
    fn test_import_rejects_fabricated_root() {
        let mut registry = FleetRegistry::new(0.4);
        let mut local_index = MemoryIndex::new();
        let leaves = sample_leaves(5);
        let mut record = make_record("robot-c", leaves, 1, Some("green"));
        record.batch_root = "0".repeat(64); // fabricated

        let result = registry.import_record(&mut local_index, record);
        assert!(matches!(result, Err(FleetRejection::RootMismatch { .. })));
        assert_eq!(local_index.len(), 0, "fabricated record must not be indexed");
    }

    #[test]
    fn test_import_rejects_leaf_mismatch() {
        let mut registry = FleetRegistry::new(0.4);
        let mut local_index = MemoryIndex::new();
        let leaves = sample_leaves(5);
        let mut record = make_record("robot-d", leaves, 0, Some("green"));
        record.cycle_hash = "1".repeat(64); // doesn't match batch_leaves[0]

        let result = registry.import_record(&mut local_index, record);
        assert_eq!(result, Err(FleetRejection::LeafMismatch));
    }

    #[test]
    fn test_import_rejects_low_trust_contributor() {
        let mut registry = FleetRegistry::new(0.6);
        // Build a poor track record for robot-e: mostly red.
        for _ in 0..8 {
            registry.record_local_batch("robot-e", Some("red"));
        }
        registry.record_local_batch("robot-e", Some("green"));

        assert!(registry.trust_score("robot-e") < 0.6);

        let mut local_index = MemoryIndex::new();
        let leaves = sample_leaves(3);
        let record = make_record("robot-e", leaves, 0, Some("green"));

        let result = registry.import_record(&mut local_index, record);
        assert!(matches!(result, Err(FleetRejection::NotTrusted { .. })));
    }

    #[test]
    fn test_slash_overrides_trust_entirely() {
        let mut registry = FleetRegistry::new(0.1); // very permissive threshold
        for _ in 0..20 {
            registry.record_local_batch("robot-f", Some("green")); // perfect record
        }
        assert_eq!(registry.trust_score("robot-f"), 1.0);

        registry.slash("robot-f");
        assert_eq!(registry.trust_score("robot-f"), 0.0);

        let mut local_index = MemoryIndex::new();
        let leaves = sample_leaves(3);
        let record = make_record("robot-f", leaves, 0, Some("green"));
        let result = registry.import_record(&mut local_index, record);
        assert!(matches!(result, Err(FleetRejection::Slashed { .. })));

        registry.reinstate("robot-f");
        assert_eq!(registry.trust_score("robot-f"), 1.0);
    }

    #[test]
    fn test_gossip_sync_summary_mixed_batch() {
        let mut registry = FleetRegistry::new(0.4);
        let mut local_index = MemoryIndex::new();

        let good_leaves = sample_leaves(4);
        let good_record = make_record("robot-g", good_leaves, 0, Some("green"));

        let bad_leaves = sample_leaves(4);
        let mut bad_record = make_record("robot-h", bad_leaves, 0, Some("green"));
        bad_record.batch_root = "f".repeat(64);

        let summary = registry.gossip_sync(&mut local_index, vec![good_record, bad_record]);

        assert_eq!(summary.accepted, 1);
        assert_eq!(summary.rejected, 1);
        assert_eq!(local_index.len(), 1);
        assert!(matches!(summary.rejections[0], FleetRejection::RootMismatch { .. }));
    }

    #[test]
    fn test_honest_red_report_not_penalized_like_fabrication() {
        let mut registry = FleetRegistry::new(0.3);
        // robot-i honestly reports 3 red batches out of 5 — a real,
        // struggling robot, not a liar.
        registry.record_local_batch("robot-i", Some("green"));
        registry.record_local_batch("robot-i", Some("green"));
        registry.record_local_batch("robot-i", Some("red"));
        registry.record_local_batch("robot-i", Some("red"));
        registry.record_local_batch("robot-i", Some("red"));

        // 2/5 = 0.4 trust, still above the 0.3 threshold — honest
        // failure reporting keeps the contributor eligible.
        assert!(registry.trust_score("robot-i") >= 0.3);
        assert!(registry.is_trusted("robot-i"));
    }
}
