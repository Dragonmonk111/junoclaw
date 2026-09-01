//! L2.5 Skills — named, taught, portable behaviors that transfer across
//! robots: sim-to-real, and real-to-real across *different* embodiments.
//!
//! A `Skill` is captured by recording a demonstration (a sequence of joint
//! keyframes) — in simulation, or on real hardware via the bridge viewer —
//! then exported as a small, self-describing JSON artifact. Any other
//! robot, same model or a different one, can import it and `retarget` it
//! onto its own joint schema by name, with an explicit coverage report of
//! which joints matched. This is the piece that makes "teach it once, run
//! it anywhere" concrete rather than aspirational: name-based retargeting
//! is honest about its limits (it will not invent a mapping between joints
//! that share no name), but it is exactly the mechanism that lets, say, a
//! `wave` skill taught on one quadruped's arm run unmodified on another
//! quadruped that reuses the same joint-naming convention.
//!
//! Paired with L1 Merkle-verified memory (`memory.rs`) and the L2 world
//! model (`worldmodel.rs`), a skill inherits the same provenance property
//! as everything else in this crate: `provenance_batch_root` ties a skill
//! back to the attested reflex batch it was captured within, so "this skill
//! was really demonstrated by robot X on date Y" is a Merkle proof, not a
//! claim.
//!
//! Skill playback is a position sequence — the same category of
//! teach-and-repeat most low-cost robots ship with — but `SkillGate` below
//! checks each frame against the L2 `WorldModel` and L1 `MemoryFetch`
//! before it's approved: predict the coarse consequence of the frame's
//! implied action, and reject it if the world model is unconfident or the
//! predicted state lands near a memory that once went red. This is the
//! same `evaluate_action` check every other candidate action in this crate
//! goes through — skill playback does not get a special exemption.
//!
//! Scope, honestly: `plugin-ros2`, the Rust adapter that actually talks to
//! ROS2 on real hardware, does not yet depend on this crate — today it only
//! calls the Python bridge over HTTP. `SkillGate` is real, tested, and
//! ready to wire in wherever a `WorldModel` and `MemoryFetch` are live
//! (in-process Rust, e.g. a future on-device agent, or an offline
//! pre-flight check on an imported skill before it ever reaches a robot).
//! Until that wiring lands, the live playback path in the ROS2 bridge uses
//! a simpler, explicitly-labeled kinematic safety clamp — see
//! `server.py::_play` — not this semantic check.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::memory::MemoryFetch;
use crate::state::PhysicsState;
use crate::worldmodel::{ActionVector, WorldModel};

/// Self-describing metadata for a taught skill. Small enough to gossip as a
/// Buzz/Nostr event on its own; the keyframe payload can travel alongside it
/// or be fetched separately (e.g. via Blossom) for longer skills.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Human name, e.g. "wave", "sit", "climb_step".
    pub name: String,
    pub description: String,
    /// Robot that taught this skill.
    pub author_robot_id: String,
    /// Joint schema the skill was captured against, in keyframe column order.
    pub joint_names: Vec<String>,
    pub frame_count: usize,
    pub cycle_dt_ms: u64,
    /// Open-source license marker, e.g. "CC0", "MIT". Empty = unspecified.
    pub license: String,
    /// Merkle batch root the demonstration was captured within, if any.
    /// Empty until patched — mirrors `DatasetExporter::patch_batch_root`.
    pub provenance_batch_root: String,
    pub created_at_ms: u64,
}

/// A taught skill: metadata plus a position-keyframe sequence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub manifest: SkillManifest,
    /// `keyframes[t][i]` = target position (radians) for
    /// `manifest.joint_names[i]` at frame `t`.
    keyframes: Vec<Vec<f64>>,
}

impl Skill {
    pub fn joint_names(&self) -> &[String] {
        &self.manifest.joint_names
    }

    pub fn frame_count(&self) -> usize {
        self.keyframes.len()
    }

    /// Target positions at frame `t`, keyed by joint name. Returns `None`
    /// past the end of the recording.
    pub fn frame(&self, t: usize) -> Option<HashMap<String, f64>> {
        let row = self.keyframes.get(t)?;
        Some(
            self.manifest
                .joint_names
                .iter()
                .cloned()
                .zip(row.iter().copied())
                .collect(),
        )
    }

    /// Backfill the Merkle batch root once the capturing batch is
    /// finalized — same pattern as `DatasetExporter::patch_batch_root`.
    pub fn patch_provenance(&mut self, batch_root: &str) {
        if self.manifest.provenance_batch_root.is_empty() {
            self.manifest.provenance_batch_root = batch_root.to_string();
        }
    }

    /// Portable JSON export — the shareable artifact. Small skills fit
    /// comfortably in a single Nostr event; larger ones are a natural fit
    /// for Blossom upload with the manifest carried in the event tags.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }

    /// Retarget this skill onto a different robot's joint schema by name.
    ///
    /// This is intentionally simple and intentionally honest: a joint only
    /// transfers if the target robot has a joint with the *same name*. No
    /// attempt is made to infer a mapping between differently-named joints
    /// (e.g. a 4-DOF arm onto a 6-DOF arm) — that is a harder, separate
    /// problem. What this buys today: any two robots that share a joint
    /// naming convention (as all `QUADRUPED_JOINT_NAMES` robots in this
    /// codebase do) can exchange skills immediately, and the coverage
    /// report makes partial matches (e.g. a hexapod importing a quadruped
    /// gait skill) legible rather than silently wrong.
    pub fn retarget(&self, target_joint_names: &[String]) -> (RetargetedSkill, RetargetReport) {
        let target_set: std::collections::HashSet<&str> =
            target_joint_names.iter().map(|s| s.as_str()).collect();

        let mut matched_idx = Vec::new();
        let mut matched_joints = Vec::new();
        let mut missing_in_target = Vec::new();

        for (i, name) in self.manifest.joint_names.iter().enumerate() {
            if target_set.contains(name.as_str()) {
                matched_idx.push(i);
                matched_joints.push(name.clone());
            } else {
                missing_in_target.push(name.clone());
            }
        }

        let matched_set: std::collections::HashSet<&str> =
            matched_joints.iter().map(|s| s.as_str()).collect();
        let unused_target_joints: Vec<String> = target_joint_names
            .iter()
            .filter(|n| !matched_set.contains(n.as_str()))
            .cloned()
            .collect();

        let keyframes: Vec<Vec<f64>> = self
            .keyframes
            .iter()
            .map(|row| matched_idx.iter().map(|&i| row[i]).collect())
            .collect();

        let coverage = if self.manifest.joint_names.is_empty() {
            0.0
        } else {
            matched_joints.len() as f64 / self.manifest.joint_names.len() as f64
        };

        let retargeted = RetargetedSkill {
            source_manifest: self.manifest.clone(),
            joint_names: matched_joints.clone(),
            keyframes,
        };

        let report = RetargetReport {
            matched_joints,
            missing_in_target,
            unused_target_joints,
            coverage,
        };

        (retargeted, report)
    }
}

/// Result of retargeting a skill onto a specific robot's joint schema —
/// only the joints that matched by name, in the same frame order as the
/// source skill.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetargetedSkill {
    /// Provenance of the original skill this was derived from.
    pub source_manifest: SkillManifest,
    pub joint_names: Vec<String>,
    keyframes: Vec<Vec<f64>>,
}

impl RetargetedSkill {
    pub fn frame_count(&self) -> usize {
        self.keyframes.len()
    }

    pub fn frame(&self, t: usize) -> Option<HashMap<String, f64>> {
        let row = self.keyframes.get(t)?;
        Some(
            self.joint_names
                .iter()
                .cloned()
                .zip(row.iter().copied())
                .collect(),
        )
    }
}

/// Transparency report for a retarget — which joints carried over, which
/// were dropped, and which target joints this skill simply does not drive.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RetargetReport {
    pub matched_joints: Vec<String>,
    pub missing_in_target: Vec<String>,
    pub unused_target_joints: Vec<String>,
    /// `matched_joints.len() / source joint count`, in `[0, 1]`.
    pub coverage: f64,
}

/// Captures a demonstration by sampling `PhysicsState` over time, holding
/// only the named joints of interest. Works identically whether the states
/// come from the in-crate simulator (teach in sim) or from real hardware
/// telemetry relayed through the ROS2 bridge (teach by physically posing
/// the robot, or driving it via the browser viewer) — both paths produce
/// the same `PhysicsState`, so the recorder does not need to know which one
/// it is watching. This is what makes the sim-to-real transfer trivial: the
/// captured `Skill` carries no notion of where it came from beyond
/// `author_robot_id` and `provenance_batch_root`.
#[derive(Clone, Debug, Default)]
pub struct SkillRecorder {
    joint_names: Vec<String>,
    keyframes: Vec<Vec<f64>>,
    cycle_dt_ms: u64,
    last_values: HashMap<String, f64>,
}

impl SkillRecorder {
    pub fn new(cycle_dt_ms: u64) -> Self {
        Self {
            cycle_dt_ms,
            ..Default::default()
        }
    }

    pub fn is_recording(&self) -> bool {
        !self.joint_names.is_empty()
    }

    pub fn frame_count(&self) -> usize {
        self.keyframes.len()
    }

    /// Begin recording, fixing the joint schema from the first observed
    /// state. Calling `start` again resets any in-progress recording.
    pub fn start(&mut self, state: &PhysicsState) {
        self.joint_names = state.joints.iter().map(|j| j.name.clone()).collect();
        self.keyframes.clear();
        self.last_values = state
            .joints
            .iter()
            .map(|j| (j.name.clone(), j.position))
            .collect();
    }

    /// Append one keyframe. Joints present in the recording schema but
    /// absent from this particular state hold their last known value
    /// (defends against a dropped sensor frame breaking the whole capture).
    pub fn capture(&mut self, state: &PhysicsState) {
        if !self.is_recording() {
            self.start(state);
        }

        for j in &state.joints {
            if self.last_values.contains_key(&j.name) {
                self.last_values.insert(j.name.clone(), j.position);
            }
        }

        let row: Vec<f64> = self
            .joint_names
            .iter()
            .map(|n| *self.last_values.get(n).unwrap_or(&0.0))
            .collect();
        self.keyframes.push(row);
    }

    /// Finish recording and produce the portable `Skill` artifact.
    pub fn finish(
        self,
        name: impl Into<String>,
        description: impl Into<String>,
        author_robot_id: impl Into<String>,
        license: impl Into<String>,
        created_at_ms: u64,
    ) -> Skill {
        let manifest = SkillManifest {
            name: name.into(),
            description: description.into(),
            author_robot_id: author_robot_id.into(),
            joint_names: self.joint_names.clone(),
            frame_count: self.keyframes.len(),
            cycle_dt_ms: self.cycle_dt_ms,
            license: license.into(),
            provenance_batch_root: String::new(),
            created_at_ms,
        };

        Skill {
            manifest,
            keyframes: self.keyframes,
        }
    }
}

// ---------------------------------------------------------------------------
// L2-gated playback
// ---------------------------------------------------------------------------

/// Outcome of checking a single skill frame against the L2/L1 gate.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GatedFrameDecision {
    pub frame_index: usize,
    pub approved: bool,
    pub uncertainty: f64,
    pub red_match: bool,
    /// Empty when rejected — caller should hold position / skip, not guess.
    pub targets: HashMap<String, f64>,
}

/// Best-effort translation of a skill frame's joint-position targets into
/// the world model's coarse `(speed, turn_rate, stride_scale, arm_position)`
/// action space, so a per-joint keyframe can be checked by the same gate
/// that already checks every other candidate action in this crate.
///
/// This is intentionally coarse — the world model was built and validated
/// for gait-level decisions, not full per-joint dynamics — and skill
/// playback inherits exactly that resolution rather than a bespoke,
/// unvalidated substitute:
/// - `speed` is approximated as mean absolute joint position change per
///   second across the joints this frame actually targets.
/// - `arm_position` is read directly off any joint named `arm_*`, since
///   `ActionVector::arm_position` is already normalized 0..1 for that.
/// - `turn_rate` and `stride_scale` are not derivable from a position
///   keyframe alone and are left at their conservative defaults (`0.0`,
///   `1.0`) rather than guessed.
fn implied_action(current: &PhysicsState, target: &HashMap<String, f64>, cycle_dt_ms: u64) -> ActionVector {
    let dt = (cycle_dt_ms as f64 / 1000.0).max(0.001);
    let mut total_delta = 0.0;
    let mut n = 0usize;
    let mut arm_position = 0.0;
    let mut arm_found = false;

    for j in &current.joints {
        if let Some(&target_pos) = target.get(&j.name) {
            total_delta += (target_pos - j.position).abs();
            n += 1;
            if j.name.starts_with("arm_") {
                arm_position = target_pos.clamp(0.0, 1.0);
                arm_found = true;
            }
        }
    }

    let mean_delta = if n > 0 { total_delta / n as f64 } else { 0.0 };

    ActionVector {
        speed: mean_delta / dt,
        turn_rate: 0.0,
        stride_scale: 1.0,
        arm_position: if arm_found { arm_position } else { 0.0 },
    }
}

/// Gates skill playback through the L2 world model and L1 memory — the
/// same `evaluate_action` check the rest of this crate uses for any other
/// candidate action. See module docs for where this is (and is not yet)
/// wired into a live robot.
pub struct SkillGate<'a> {
    world_model: &'a WorldModel,
    memory: &'a MemoryFetch,
    epsilon: f64,
}

impl<'a> SkillGate<'a> {
    pub fn new(world_model: &'a WorldModel, memory: &'a MemoryFetch, epsilon: f64) -> Self {
        Self {
            world_model,
            memory,
            epsilon,
        }
    }

    /// Check whether a single frame is safe to execute given the robot's
    /// current physics state. Does not execute anything — the caller
    /// decides what to do with a rejected frame (hold position, skip,
    /// abort playback).
    pub fn check_frame(
        &self,
        current_state: &PhysicsState,
        frame_index: usize,
        targets: &HashMap<String, f64>,
        cycle_dt_ms: u64,
    ) -> GatedFrameDecision {
        let action = implied_action(current_state, targets, cycle_dt_ms);
        let eval = self
            .world_model
            .evaluate_action(current_state, &action, self.memory, self.epsilon);

        GatedFrameDecision {
            frame_index,
            approved: eval.approved,
            uncertainty: eval.predicted.uncertainty,
            red_match: eval.red_match,
            targets: if eval.approved {
                targets.clone()
            } else {
                HashMap::new()
            },
        }
    }

    /// Pre-flight check of an entire retargeted skill against a single
    /// starting state — every frame is evaluated from the same
    /// `current_state` rather than a forward-simulated trajectory, since
    /// this gate has no simulator to advance state between frames. This is
    /// deliberately conservative and is meant for offline screening of an
    /// imported skill ("does anything in here look obviously unsafe from
    /// where the robot is right now"), not as a substitute for per-tick
    /// `check_frame` calls during real playback.
    pub fn check_skill(
        &self,
        skill: &RetargetedSkill,
        current_state: &PhysicsState,
        cycle_dt_ms: u64,
    ) -> Vec<GatedFrameDecision> {
        (0..skill.frame_count())
            .filter_map(|i| skill.frame(i).map(|targets| (i, targets)))
            .map(|(i, targets)| self.check_frame(current_state, i, &targets, cycle_dt_ms))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{PhysicsSimulator, QuadrupedBackend, QuadrupedConfig};

    fn demo_states(n: usize) -> Vec<PhysicsState> {
        let mut sim = QuadrupedBackend::new("dogzilla-teach".to_string(), QuadrupedConfig::default());
        (0..n).map(|_| sim.step(10)).collect()
    }

    #[test]
    fn test_recorder_captures_frames_and_schema() {
        let mut recorder = SkillRecorder::new(10);
        let states = demo_states(20);

        for s in &states {
            recorder.capture(s);
        }

        assert!(recorder.is_recording());
        assert_eq!(recorder.frame_count(), 20);

        let skill = recorder.finish("wave", "wave the arm", "dogzilla-teach", "CC0", 1_700_000_000_000);
        assert_eq!(skill.frame_count(), 20);
        assert_eq!(skill.manifest.name, "wave");
        assert_eq!(skill.manifest.joint_names.len(), states[0].joints.len());
        assert_eq!(skill.manifest.provenance_batch_root, "");
    }

    #[test]
    fn test_skill_frame_lookup_matches_source() {
        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(5) {
            recorder.capture(&s);
        }
        let skill = recorder.finish("gait_trot", "", "dogzilla-teach", "MIT", 0);

        let frame0 = skill.frame(0).expect("frame 0 exists");
        assert_eq!(frame0.len(), skill.joint_names().len());
        assert!(skill.frame(5).is_none(), "only 5 frames recorded (0..4)");
    }

    #[test]
    fn test_patch_provenance_only_fills_once() {
        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(3) {
            recorder.capture(&s);
        }
        let mut skill = recorder.finish("sit", "", "r1", "CC0", 0);

        skill.patch_provenance("root_abc");
        assert_eq!(skill.manifest.provenance_batch_root, "root_abc");

        skill.patch_provenance("root_should_not_overwrite");
        assert_eq!(skill.manifest.provenance_batch_root, "root_abc");
    }

    #[test]
    fn test_json_roundtrip_preserves_keyframes() {
        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(4) {
            recorder.capture(&s);
        }
        let skill = recorder.finish("fetch", "goes and gets it", "dogzilla-001", "Apache-2.0", 42);

        let json = skill.to_json().expect("serialize");
        let decoded = Skill::from_json(&json).expect("deserialize");

        assert_eq!(decoded.manifest.name, "fetch");
        assert_eq!(decoded.frame_count(), skill.frame_count());
        assert_eq!(decoded.frame(0), skill.frame(0));
    }

    #[test]
    fn test_retarget_full_match_when_same_schema() {
        let mut recorder = SkillRecorder::new(10);
        let states = demo_states(6);
        for s in &states {
            recorder.capture(s);
        }
        let skill = recorder.finish("trot", "", "robot-a", "CC0", 0);

        // Same robot type -> identical joint schema -> full coverage.
        let target_joints: Vec<String> = states[0].joints.iter().map(|j| j.name.clone()).collect();
        let (retargeted, report) = skill.retarget(&target_joints);

        assert_eq!(report.coverage, 1.0);
        assert!(report.missing_in_target.is_empty());
        assert!(report.unused_target_joints.is_empty());
        assert_eq!(retargeted.frame_count(), skill.frame_count());
        assert_eq!(retargeted.joint_names.len(), skill.joint_names().len());
    }

    #[test]
    fn test_retarget_partial_match_reports_gaps_honestly() {
        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(3) {
            recorder.capture(&s);
        }
        let skill = recorder.finish("wave", "", "robot-a", "CC0", 0);
        let source_joint_count = skill.joint_names().len();

        // A different robot that only shares the first joint name and adds
        // one of its own the skill never touches.
        let mut target_joints = vec![skill.joint_names()[0].clone()];
        target_joints.push("extra_joint_only_on_target".to_string());

        let (retargeted, report) = skill.retarget(&target_joints);

        assert_eq!(report.matched_joints, vec![skill.joint_names()[0].clone()]);
        assert_eq!(report.missing_in_target.len(), source_joint_count - 1);
        assert_eq!(report.unused_target_joints, vec!["extra_joint_only_on_target".to_string()]);
        assert!((report.coverage - (1.0 / source_joint_count as f64)).abs() < 1e-9);
        assert_eq!(retargeted.joint_names.len(), 1);

        let frame0 = retargeted.frame(0).expect("frame exists");
        assert_eq!(frame0.len(), 1);
    }

    #[test]
    fn test_retarget_no_overlap_yields_zero_coverage_not_panic() {
        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(2) {
            recorder.capture(&s);
        }
        let skill = recorder.finish("gripper_close", "", "robot-a", "CC0", 0);

        let alien_joints = vec!["wheel_left".to_string(), "wheel_right".to_string()];
        let (retargeted, report) = skill.retarget(&alien_joints);

        assert_eq!(report.coverage, 0.0);
        assert!(report.matched_joints.is_empty());
        assert_eq!(retargeted.joint_names.len(), 0);
        // Frames still exist, just empty per-frame maps — caller can detect
        // via joint_names().is_empty() that this skill does not apply here.
        assert_eq!(retargeted.frame(0).unwrap().len(), 0);
    }

    #[test]
    fn test_capture_before_start_auto_starts() {
        let mut recorder = SkillRecorder::new(10);
        let states = demo_states(1);
        // No explicit start() call — first capture() should bootstrap schema.
        recorder.capture(&states[0]);

        assert!(recorder.is_recording());
        assert_eq!(recorder.frame_count(), 1);
    }

    // -----------------------------------------------------------------------
    // SkillGate — L2-gated playback
    // -----------------------------------------------------------------------

    use crate::memory::{MemoryFetch, MemoryIndex, RootCache};
    use crate::merkle::compute_merkle_root;
    use sha2::{Digest, Sha256};

    fn hash_of(data: &[u8]) -> String {
        hex::encode(Sha256::digest(data))
    }

    fn base_state() -> PhysicsState {
        demo_states(1).into_iter().next().unwrap()
    }

    fn small_move_frame(state: &PhysicsState) -> HashMap<String, f64> {
        // Tiny nudge on every joint — should imply a small, low-uncertainty
        // action once the model is trained on small, safe transitions.
        state.joints.iter().map(|j| (j.name.clone(), j.position + 0.01)).collect()
    }

    fn large_move_frame(state: &PhysicsState) -> HashMap<String, f64> {
        // Large jump on every joint — implies a much bigger action.
        state.joints.iter().map(|j| (j.name.clone(), j.position + 5.0)).collect()
    }

    #[test]
    fn test_skill_gate_untrained_model_rejects_everything() {
        let model = WorldModel::new();
        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);
        let gate = SkillGate::new(&model, &memory, 5.0);

        let state = base_state();
        let frame = small_move_frame(&state);
        let decision = gate.check_frame(&state, 0, &frame, 10);

        assert!(!decision.approved, "untrained model should not approve any frame");
        assert!(decision.targets.is_empty(), "rejected frame must not hand back targets");
    }

    #[test]
    fn test_skill_gate_trained_model_approves_small_safe_move() {
        let mut model = WorldModel::new();
        for _ in 0..200 {
            model.train_step(&crate::worldmodel::TransitionSample {
                state_t: crate::memory::StateFeatures::from_state(&base_state()),
                action: ActionVector { speed: 0.1, turn_rate: 0.0, stride_scale: 1.0, arm_position: 0.0 },
                state_t1: crate::memory::StateFeatures::from_state(&base_state()),
                verdict: Some("green".into()),
                batch_root: "root_test".into(),
                robot_id: "dogzilla-teach".into(),
            });
        }

        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);
        let gate = SkillGate::new(&model, &memory, 5.0);

        let state = base_state();
        let frame = small_move_frame(&state);
        let decision = gate.check_frame(&state, 0, &frame, 10);

        assert!(decision.approved, "trained model with no red memory should approve a small safe move");
        assert!(!decision.red_match);
        assert_eq!(decision.targets.len(), frame.len());
    }

    #[test]
    fn test_skill_gate_rejects_frame_near_red_memory() {
        let mut model = WorldModel::new();
        // Train the model to strongly associate large speed with large tilt,
        // so a big joint jump predicts a state near the red memory below.
        for i in 0..100 {
            let s = 0.5 + 0.05 * i as f64;
            model.train_step(&crate::worldmodel::TransitionSample {
                state_t: crate::memory::StateFeatures::from_state(&base_state()),
                action: ActionVector { speed: s, turn_rate: 0.0, stride_scale: 1.0, arm_position: 0.0 },
                state_t1: {
                    let mut st = base_state();
                    st.sensors.tilt_degrees = 30.0 + s;
                    crate::memory::StateFeatures::from_state(&st)
                },
                verdict: Some("green".into()),
                batch_root: "root_test".into(),
                robot_id: "dogzilla-teach".into(),
            });
        }

        // Red memory at high tilt.
        let mut index = MemoryIndex::new();
        let mut red_state = base_state();
        red_state.sensors.tilt_degrees = 35.0;
        let red_hashes = vec![hash_of(b"red_cycle_0")];
        index.add_batch(
            "batch_red",
            "dogzilla-teach",
            &[red_state],
            &red_hashes,
            Some("red".into()),
            vec!["max_tilt".into()],
        );
        let mut cache = RootCache::new(8);
        cache.push(compute_merkle_root(&red_hashes), 100);
        let memory = MemoryFetch::new(index, cache);

        let gate = SkillGate::new(&model, &memory, 30.0);
        let state = base_state();

        let large_frame = large_move_frame(&state);
        let decision = gate.check_frame(&state, 0, &large_frame, 10);
        assert!(!decision.approved, "large move predicted near a red memory should be rejected");
    }

    #[test]
    fn test_skill_gate_check_skill_returns_one_decision_per_frame() {
        let model = WorldModel::new();
        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);
        let gate = SkillGate::new(&model, &memory, 5.0);

        let mut recorder = SkillRecorder::new(10);
        for s in demo_states(4) {
            recorder.capture(&s);
        }
        let skill = recorder.finish("trot", "", "dogzilla-teach", "CC0", 0);
        let joint_names: Vec<String> = skill.joint_names().to_vec();
        let (retargeted, _) = skill.retarget(&joint_names);

        let state = base_state();
        let decisions = gate.check_skill(&retargeted, &state, 10);

        assert_eq!(decisions.len(), retargeted.frame_count());
    }
}
