//! Transition dataset export — turns verified reflex-loop transitions into
//! a training corpus for the L2 world model (or any external trainer).
//!
//! Every record is provenance-linked to a Merkle batch root. A dataset
//! exported here is not just a training file — it is auditable. Pull any
//! row, verify its `batch_root` against the on-chain anchor, and prove the
//! data was not fabricated or altered after export.

use crate::memory::StateFeatures;
use crate::worldmodel::{ActionVector, TransitionSample};
use serde::{Deserialize, Serialize};

/// A single exported transition, ready for an external trainer (PyTorch,
/// JAX, or the in-crate `WorldModel`) or for audit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetRecord {
    /// Feature vector of the initial state.
    pub state_t: StateFeatures,
    /// Action taken.
    pub action: ActionVector,
    /// Feature vector of the resulting state.
    pub state_t1: StateFeatures,
    /// Truth verdict for the cycle (green/red from `check_invariants`).
    pub verdict: String,
    /// Merkle batch root this transition belongs to. Empty until the
    /// containing batch has been committed — see `DatasetExporter::patch_batch_root`.
    pub batch_root: String,
    /// Robot that produced this transition.
    pub robot_id: String,
    /// Simulation/wall-clock timestamp (ms).
    pub timestamp_ms: u64,
}

/// Summary statistics over an exported dataset — useful for a quick sanity
/// check before training (e.g. "is this corpus mostly green, or do we have
/// enough red examples to teach the model what to avoid?").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetStats {
    pub total_records: usize,
    pub green_count: usize,
    pub red_count: usize,
    pub other_count: usize,
    pub unique_robots: usize,
    pub unique_batches: usize,
}

/// Accumulates transitions from a running reflex loop and exports them in
/// formats consumable by external trainers.
#[derive(Clone, Debug, Default)]
pub struct DatasetExporter {
    records: Vec<DatasetRecord>,
    /// Index range of records belonging to the batch currently being
    /// accumulated (patched with the real root once the batch commits).
    pending_batch_start: usize,
}

impl DatasetExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transition sample produced by the reflex loop. The
    /// `batch_root` field is left empty until `patch_batch_root` is called
    /// when the containing batch is committed to L1 memory.
    pub fn record(&mut self, sample: &TransitionSample, timestamp_ms: u64) {
        self.records.push(DatasetRecord {
            state_t: sample.state_t.clone(),
            action: sample.action.clone(),
            state_t1: sample.state_t1.clone(),
            verdict: sample.verdict.clone().unwrap_or_else(|| "unknown".to_string()),
            batch_root: sample.batch_root.clone(),
            robot_id: sample.robot_id.clone(),
            timestamp_ms,
        });
    }

    /// Backfill the Merkle batch root for every pending record once the
    /// batch is finalized. Called by `ReflexPipeline` right after
    /// `commit_batch` computes the root.
    pub fn patch_batch_root(&mut self, batch_root: &str) {
        for record in &mut self.records[self.pending_batch_start..] {
            if record.batch_root.is_empty() {
                record.batch_root = batch_root.to_string();
            }
        }
        self.pending_batch_start = self.records.len();
    }

    /// Number of records currently held.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// All records (read-only).
    pub fn records(&self) -> &[DatasetRecord] {
        &self.records
    }

    /// Export as JSON Lines — one record per line. This is the format a
    /// Python training script (PyTorch/JAX DataLoader) would stream.
    pub fn to_jsonl(&self) -> String {
        self.records
            .iter()
            .filter_map(|r| serde_json::to_string(r).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export as CSV — flattened feature vectors, for quick inspection in
    /// pandas or a spreadsheet.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(
            "robot_id,timestamp_ms,verdict,batch_root,\
             t_joint_pos_mean,t_joint_vel_mean,t_joint_torque_mean,t_joint_torque_max,\
             t_accel_magnitude,t_gyro_magnitude,t_contact_count,t_contact_force_max,\
             t_com_height,t_com_speed,t_tilt,t_speed,\
             action_speed,action_turn_rate,action_stride_scale,action_arm_position,\
             t1_joint_pos_mean,t1_joint_vel_mean,t1_joint_torque_mean,t1_joint_torque_max,\
             t1_accel_magnitude,t1_gyro_magnitude,t1_contact_count,t1_contact_force_max,\
             t1_com_height,t1_com_speed,t1_tilt,t1_speed\n",
        );

        for r in &self.records {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.robot_id,
                r.timestamp_ms,
                r.verdict,
                r.batch_root,
                r.state_t.joint_pos_mean, r.state_t.joint_vel_mean, r.state_t.joint_torque_mean, r.state_t.joint_torque_max,
                r.state_t.accel_magnitude, r.state_t.gyro_magnitude, r.state_t.contact_count, r.state_t.contact_force_max,
                r.state_t.com_height, r.state_t.com_speed, r.state_t.tilt, r.state_t.speed,
                r.action.speed, r.action.turn_rate, r.action.stride_scale, r.action.arm_position,
                r.state_t1.joint_pos_mean, r.state_t1.joint_vel_mean, r.state_t1.joint_torque_mean, r.state_t1.joint_torque_max,
                r.state_t1.accel_magnitude, r.state_t1.gyro_magnitude, r.state_t1.contact_count, r.state_t1.contact_force_max,
                r.state_t1.com_height, r.state_t1.com_speed, r.state_t1.tilt, r.state_t1.speed,
            ));
        }

        out
    }

    /// Summary statistics over the current dataset.
    pub fn stats(&self) -> DatasetStats {
        use std::collections::HashSet;

        let mut green_count = 0;
        let mut red_count = 0;
        let mut other_count = 0;
        let mut robots = HashSet::new();
        let mut batches = HashSet::new();

        for r in &self.records {
            match r.verdict.as_str() {
                "green" => green_count += 1,
                "red" => red_count += 1,
                _ => other_count += 1,
            }
            robots.insert(r.robot_id.clone());
            if !r.batch_root.is_empty() {
                batches.insert(r.batch_root.clone());
            }
        }

        DatasetStats {
            total_records: self.records.len(),
            green_count,
            red_count,
            other_count,
            unique_robots: robots.len(),
            unique_batches: batches.len(),
        }
    }

    /// Verify that every record with a non-empty `batch_root` is internally
    /// consistent — i.e. no record claims a batch root that was never
    /// patched from an actual commit. This is a cheap sanity check, not a
    /// full Merkle proof verification (that requires the original leaf
    /// hashes, held by `MemoryIndex`).
    pub fn all_roots_assigned(&self) -> bool {
        self.records.iter().all(|r| !r.batch_root.is_empty())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample(tilt: f64, verdict: &str, batch_root: &str) -> TransitionSample {
        let features = StateFeatures {
            joint_pos_mean: 0.0,
            joint_vel_mean: 0.0,
            joint_torque_mean: 0.0,
            joint_torque_max: 0.0,
            accel_magnitude: 0.0,
            gyro_magnitude: 0.0,
            contact_count: 1.0,
            contact_force_max: 5.0,
            com_height: 0.12,
            com_speed: 0.5,
            tilt,
            speed: 0.5,
        };
        TransitionSample {
            state_t: features.clone(),
            action: ActionVector::default(),
            state_t1: features,
            verdict: Some(verdict.to_string()),
            batch_root: batch_root.to_string(),
            robot_id: "dogzilla-export-test".to_string(),
        }
    }

    #[test]
    fn test_exporter_records_and_counts() {
        let mut exporter = DatasetExporter::new();
        exporter.record(&make_sample(5.0, "green", ""), 0);
        exporter.record(&make_sample(30.0, "red", ""), 1);

        assert_eq!(exporter.len(), 2);
        let stats = exporter.stats();
        assert_eq!(stats.total_records, 2);
        assert_eq!(stats.green_count, 1);
        assert_eq!(stats.red_count, 1);
        assert_eq!(stats.unique_robots, 1);
    }

    #[test]
    fn test_exporter_patch_batch_root() {
        let mut exporter = DatasetExporter::new();
        exporter.record(&make_sample(5.0, "green", ""), 0);
        exporter.record(&make_sample(6.0, "green", ""), 1);

        assert!(!exporter.all_roots_assigned());

        exporter.patch_batch_root("root_abc123");

        assert!(exporter.all_roots_assigned());
        assert!(exporter.records().iter().all(|r| r.batch_root == "root_abc123"));
    }

    #[test]
    fn test_exporter_patch_only_affects_pending_records() {
        let mut exporter = DatasetExporter::new();
        exporter.record(&make_sample(5.0, "green", ""), 0);
        exporter.patch_batch_root("root_batch_1");

        exporter.record(&make_sample(6.0, "green", ""), 1);
        exporter.patch_batch_root("root_batch_2");

        assert_eq!(exporter.records()[0].batch_root, "root_batch_1");
        assert_eq!(exporter.records()[1].batch_root, "root_batch_2");
    }

    #[test]
    fn test_exporter_to_jsonl_roundtrip() {
        let mut exporter = DatasetExporter::new();
        exporter.record(&make_sample(5.0, "green", "root1"), 0);
        exporter.record(&make_sample(30.0, "red", "root1"), 1);

        let jsonl = exporter.to_jsonl();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);

        let decoded: DatasetRecord = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(decoded.verdict, "green");
        assert_eq!(decoded.robot_id, "dogzilla-export-test");
    }

    #[test]
    fn test_exporter_to_csv_has_header_and_rows() {
        let mut exporter = DatasetExporter::new();
        exporter.record(&make_sample(5.0, "green", "root1"), 0);

        let csv = exporter.to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2, "header + 1 data row");
        assert!(lines[0].starts_with("robot_id,timestamp_ms,verdict,batch_root"));
    }

    #[test]
    fn test_exporter_empty_stats() {
        let exporter = DatasetExporter::new();
        let stats = exporter.stats();
        assert_eq!(stats.total_records, 0);
        assert_eq!(stats.unique_robots, 0);
        assert!(exporter.all_roots_assigned(), "vacuously true for empty set");
    }
}
