//! Deterministic replay — proves that a recorded batch of cycles can be
//! reproduced bit-for-bit from a fresh simulator instance.
//!
//! `QuadrupedBackend` has no RNG and no wall-clock dependency (its
//! `timestamp_ms` is simulated elapsed time, not `SystemTime::now()`), so
//! the same robot config + the same sequence of joint-torque commands
//! always produces the same sequence of cycle hashes. This module makes
//! that property mechanically checkable rather than just an assumption.
//!
//! This is the basis for third-party audit: given a `ReplayLog` (which is
//! small — a config plus a list of torque vectors) and a claimed batch
//! Merkle root, anyone can replay it locally and confirm the root without
//! trusting the original robot's reported states.

use crate::merkle::compute_merkle_root;
use crate::simulator::{PhysicsSimulator, QuadrupedBackend, QuadrupedConfig};
use serde::{Deserialize, Serialize};

/// One recorded cycle: the exact joint torques commanded and the cycle
/// duration, plus the hash that resulted from applying them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayCycle {
    pub torques: Vec<f64>,
    pub dt_ms: u64,
    pub recorded_hash: String,
}

/// A fully self-contained recording of a run: robot identity, its config,
/// and every command applied. Small enough to embed in an audit bundle or
/// ship over the wire for independent replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayLog {
    pub robot_id: String,
    pub config: QuadrupedConfig,
    pub cycles: Vec<ReplayCycle>,
}

impl ReplayLog {
    pub fn new(robot_id: impl Into<String>, config: QuadrupedConfig) -> Self {
        Self {
            robot_id: robot_id.into(),
            config,
            cycles: Vec::new(),
        }
    }

    /// The Merkle root implied by this log's recorded hashes.
    pub fn recorded_merkle_root(&self) -> String {
        let hashes: Vec<String> = self.cycles.iter().map(|c| c.recorded_hash.clone()).collect();
        compute_merkle_root(&hashes)
    }
}

/// A recording wrapper around `QuadrupedBackend`: every `apply_and_step`
/// call is logged, so the resulting `ReplayLog` can later reproduce the
/// exact same run from scratch.
pub struct Recorder {
    sim: QuadrupedBackend,
    log: ReplayLog,
}

impl Recorder {
    pub fn new(robot_id: impl Into<String>, config: QuadrupedConfig) -> Self {
        let robot_id = robot_id.into();
        let sim = QuadrupedBackend::new(robot_id.clone(), config.clone());
        Self {
            sim,
            log: ReplayLog::new(robot_id, config),
        }
    }

    /// Apply a torque command for one cycle and record it.
    pub fn apply_and_step(&mut self, torques: &[f64], dt_ms: u64) -> String {
        self.sim.set_joint_controls(torques);
        let state = self.sim.step(dt_ms);
        let hash = state.hash();
        self.log.cycles.push(ReplayCycle {
            torques: torques.to_vec(),
            dt_ms,
            recorded_hash: hash.clone(),
        });
        hash
    }

    /// Consume the recorder, returning the completed log.
    pub fn into_log(self) -> ReplayLog {
        self.log
    }
}

/// Result of replaying a log against a fresh simulator instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayVerification {
    /// True if every cycle's replayed hash matched the recorded hash.
    pub all_matched: bool,
    /// Indices where replay diverged from the recording (empty = perfect replay).
    pub mismatched_cycles: Vec<usize>,
    /// Merkle root recomputed from the replayed hashes.
    pub replayed_merkle_root: String,
    /// Merkle root recomputed from the originally recorded hashes.
    pub recorded_merkle_root: String,
}

impl ReplayVerification {
    /// True if the replayed root matches the recorded root — the
    /// strongest single-number determinism proof.
    pub fn roots_match(&self) -> bool {
        self.replayed_merkle_root == self.recorded_merkle_root
    }
}

/// Replay a `ReplayLog` from a fresh `QuadrupedBackend` and check that
/// every cycle hash reproduces exactly.
pub fn replay(log: &ReplayLog) -> ReplayVerification {
    let mut sim = QuadrupedBackend::new(log.robot_id.clone(), log.config.clone());
    let mut mismatched_cycles = Vec::new();
    let mut replayed_hashes = Vec::with_capacity(log.cycles.len());

    for (i, cycle) in log.cycles.iter().enumerate() {
        sim.set_joint_controls(&cycle.torques);
        let state = sim.step(cycle.dt_ms);
        let hash = state.hash();
        if hash != cycle.recorded_hash {
            mismatched_cycles.push(i);
        }
        replayed_hashes.push(hash);
    }

    ReplayVerification {
        all_matched: mismatched_cycles.is_empty(),
        mismatched_cycles,
        replayed_merkle_root: compute_merkle_root(&replayed_hashes),
        recorded_merkle_root: log.recorded_merkle_root(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn turn_torques(max_t: f64) -> Vec<f64> {
        let mut t = vec![0.0; 15];
        t[0] = max_t;
        t[9] = max_t;
        t[3] = max_t;
        t[6] = -max_t;
        t
    }

    #[test]
    fn test_recorder_produces_replayable_log() {
        let mut recorder = Recorder::new("dogzilla-replay-test", QuadrupedConfig::default());
        let torques = turn_torques(2.0);

        for _ in 0..50 {
            recorder.apply_and_step(&torques, 1);
        }

        let log = recorder.into_log();
        assert_eq!(log.cycles.len(), 50);

        let verification = replay(&log);
        assert!(verification.all_matched, "identical config + commands must replay exactly");
        assert!(verification.roots_match());
        assert!(verification.mismatched_cycles.is_empty());
    }

    #[test]
    fn test_replay_detects_tampered_hash() {
        let mut recorder = Recorder::new("dogzilla-replay-tamper", QuadrupedConfig::default());
        let torques = turn_torques(1.0);
        for _ in 0..10 {
            recorder.apply_and_step(&torques, 1);
        }
        let mut log = recorder.into_log();

        // Tamper with a recorded hash, simulating a dishonest reporter.
        log.cycles[5].recorded_hash = "0".repeat(64);

        let verification = replay(&log);
        assert!(!verification.all_matched);
        assert_eq!(verification.mismatched_cycles, vec![5]);
        assert!(!verification.roots_match(), "tampering must change the recorded root");
    }

    #[test]
    fn test_replay_detects_different_robot_id_affecting_nothing_but_config_matters() {
        // Sanity: replay uses the log's own robot_id/config, so a log is
        // self-describing and doesn't depend on any external state.
        let mut recorder = Recorder::new("dogzilla-A", QuadrupedConfig::default());
        let torques = turn_torques(1.5);
        for _ in 0..20 {
            recorder.apply_and_step(&torques, 1);
        }
        let log = recorder.into_log();

        let verification = replay(&log);
        assert!(verification.all_matched);
    }

    #[test]
    fn test_replay_different_config_diverges() {
        let mut recorder = Recorder::new("dogzilla-cfg-test", QuadrupedConfig::default());
        let torques = turn_torques(2.0);
        for _ in 0..30 {
            recorder.apply_and_step(&torques, 1);
        }
        let mut log = recorder.into_log();

        // Swap in a different mass — physically different robot, should
        // diverge from the recorded hashes despite identical commands.
        log.config.mass *= 3.0;

        let verification = replay(&log);
        assert!(!verification.all_matched, "different physical config must diverge");
    }
}
