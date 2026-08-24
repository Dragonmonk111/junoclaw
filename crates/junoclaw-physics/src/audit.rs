//! Audit bundle export — packages a committed batch into a single,
//! independently verifiable artifact.
//!
//! An `AuditBundle` contains everything a third party needs to confirm a
//! batch's integrity WITHOUT trusting the robot, the fleet operator, or
//! this codebase: the claimed Merkle root, the full leaf hash list (so the
//! root can be recomputed from scratch), and a set of sample inclusion
//! proofs. `AuditBundle::verify()` performs that recomputation and proof
//! check locally — it is the machine-checkable version of "trust but
//! verify."

use crate::merkle::{compute_merkle_proof, compute_merkle_root, verify_merkle_proof};
use serde::{Deserialize, Serialize};

/// A single Merkle inclusion proof bundled for independent verification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SampleProof {
    pub leaf_index: usize,
    pub leaf_hash: String,
    pub proof: Vec<String>,
}

/// A self-contained, exportable record of one committed batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditBundle {
    pub batch_id: String,
    pub robot_id: String,
    pub cycle_hashes: Vec<String>,
    pub claimed_merkle_root: String,
    pub verdict: Option<String>,
    pub violated_invariants: Vec<String>,
    pub sample_proofs: Vec<SampleProof>,
}

/// Result of independently verifying an `AuditBundle`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditVerification {
    /// True if recomputing the root from `cycle_hashes` matches `claimed_merkle_root`.
    pub root_matches: bool,
    /// True if every sample proof independently verifies against the claimed root.
    pub all_sample_proofs_valid: bool,
    /// Indices of sample proofs that failed verification (empty = all valid).
    pub failed_proof_indices: Vec<usize>,
    /// Overall pass/fail — true only if both checks above pass.
    pub passed: bool,
}

impl AuditBundle {
    /// Build a bundle from a committed batch. `sample_indices` selects
    /// which leaves to include full inclusion proofs for — typically the
    /// violating cycles plus a few random clean ones, not the whole batch,
    /// to keep the bundle small.
    pub fn build(
        batch_id: impl Into<String>,
        robot_id: impl Into<String>,
        cycle_hashes: Vec<String>,
        verdict: Option<String>,
        violated_invariants: Vec<String>,
        sample_indices: &[usize],
    ) -> Self {
        let claimed_merkle_root = compute_merkle_root(&cycle_hashes);

        let sample_proofs = sample_indices
            .iter()
            .filter(|&&i| i < cycle_hashes.len())
            .map(|&i| SampleProof {
                leaf_index: i,
                leaf_hash: cycle_hashes[i].clone(),
                proof: compute_merkle_proof(&cycle_hashes, i),
            })
            .collect();

        Self {
            batch_id: batch_id.into(),
            robot_id: robot_id.into(),
            cycle_hashes,
            claimed_merkle_root,
            verdict,
            violated_invariants,
            sample_proofs,
        }
    }

    /// Serialize to JSON — the exportable, shareable form of the bundle.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Independently verify this bundle. Recomputes the Merkle root from
    /// `cycle_hashes` (does not trust `claimed_merkle_root`) and checks
    /// every sample proof reconstructs to that recomputed root.
    pub fn verify(&self) -> AuditVerification {
        let recomputed_root = compute_merkle_root(&self.cycle_hashes);
        let root_matches = recomputed_root == self.claimed_merkle_root;

        let mut failed_proof_indices = Vec::new();
        for (i, sample) in self.sample_proofs.iter().enumerate() {
            let reconstructed =
                verify_merkle_proof(&sample.leaf_hash, sample.leaf_index, &sample.proof);
            let leaf_matches_bundle = self
                .cycle_hashes
                .get(sample.leaf_index)
                .map(|h| h == &sample.leaf_hash)
                .unwrap_or(false);

            if reconstructed != recomputed_root || !leaf_matches_bundle {
                failed_proof_indices.push(i);
            }
        }

        let all_sample_proofs_valid = failed_proof_indices.is_empty();

        AuditVerification {
            root_matches,
            all_sample_proofs_valid,
            passed: root_matches && all_sample_proofs_valid,
            failed_proof_indices,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{PhysicsSimulator, QuadrupedBackend, QuadrupedConfig};

    fn sample_hashes(n: usize) -> Vec<String> {
        let mut sim = QuadrupedBackend::new("dogzilla-audit-test".to_string(), QuadrupedConfig::default());
        (0..n).map(|_| sim.step(1).hash()).collect()
    }

    #[test]
    fn test_audit_bundle_verifies_clean() {
        let hashes = sample_hashes(20);
        let bundle = AuditBundle::build(
            "batch_1",
            "dogzilla-audit-test",
            hashes,
            Some("green".to_string()),
            vec![],
            &[0, 5, 19],
        );

        let result = bundle.verify();
        assert!(result.passed);
        assert!(result.root_matches);
        assert!(result.all_sample_proofs_valid);
        assert!(result.failed_proof_indices.is_empty());
    }

    #[test]
    fn test_audit_bundle_detects_tampered_root() {
        let hashes = sample_hashes(10);
        let mut bundle = AuditBundle::build(
            "batch_2",
            "dogzilla-audit-test",
            hashes,
            None,
            vec![],
            &[3],
        );

        bundle.claimed_merkle_root = "0".repeat(64);

        let result = bundle.verify();
        assert!(!result.passed);
        assert!(!result.root_matches);
    }

    #[test]
    fn test_audit_bundle_detects_tampered_leaf_hash() {
        let hashes = sample_hashes(10);
        let mut bundle = AuditBundle::build(
            "batch_3",
            "dogzilla-audit-test",
            hashes,
            None,
            vec![],
            &[2, 7],
        );

        // Tamper with a sample proof's leaf hash without updating the root —
        // simulates a dishonest party trying to swap in a different cycle.
        bundle.sample_proofs[0].leaf_hash = "1".repeat(64);

        let result = bundle.verify();
        assert!(!result.passed);
        assert!(!result.all_sample_proofs_valid);
        assert_eq!(result.failed_proof_indices, vec![0]);
    }

    #[test]
    fn test_audit_bundle_json_roundtrip() {
        let hashes = sample_hashes(5);
        let bundle = AuditBundle::build(
            "batch_4",
            "dogzilla-audit-test",
            hashes,
            Some("red".to_string()),
            vec!["max_tilt_degrees".to_string()],
            &[0, 4],
        );

        let json = bundle.to_json();
        let decoded = AuditBundle::from_json(&json).unwrap();

        assert_eq!(decoded.batch_id, "batch_4");
        assert_eq!(decoded.verdict.as_deref(), Some("red"));
        assert!(decoded.verify().passed);
    }
}
