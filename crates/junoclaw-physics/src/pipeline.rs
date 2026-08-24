//! Reflex pipeline — wires L2 (world model) into L1 (memory) into L0
//! (physics execution) as a single running loop.
//!
//! This is the concrete implementation of the article's claim:
//!
//!   "L2 imagines. L1 remembers. L0 executes."
//!
//! Every step:
//!   1. L2 predicts the outcome of each candidate action
//!   2. L1 checks whether any predicted outcome resembles a past red verdict
//!   3. The safest approved action is executed on the simulator (L0)
//!   4. If no candidate is approved, the pipeline falls back to a
//!      conservative zero action — it never blocks, never guesses
//!   5. Every cycle is hashed; every `batch_size` cycles, the batch is
//!      Merkle-rooted, indexed into L1 memory, and the root is cached
//!   6. The world model is trained online on the resulting verified
//!      transition, closing the loop

use crate::attestation::check_invariants;
use crate::dataset::DatasetExporter;
use crate::memory::{MemoryFetch, StateFeatures};
use crate::merkle::compute_merkle_root;
use crate::simulator::PhysicsSimulator;
use crate::state::PhysicsState;
use crate::worldmodel::{ActionEvaluation, ActionVector, TransitionSample, WorldModel};
use junoclaw_coordination::SafetyEnvelope;

/// Configuration for the reflex pipeline.
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    /// Feature-distance epsilon used for L1 memory queries.
    pub epsilon: f64,
    /// Number of cycles per batch before committing to L1 memory.
    pub batch_size: usize,
    /// Safety envelope checked every cycle.
    pub envelope: SafetyEnvelope,
    /// Cycle duration in ms (default 1ms = 1000Hz reflex rate).
    pub cycle_dt_ms: u64,
    /// Torque clamp applied when converting an ActionVector to sim controls.
    pub max_torque: f64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            epsilon: 5.0,
            batch_size: 100,
            envelope: SafetyEnvelope {
                robot_id: "default".to_string(),
                max_speed: 1.5,
                max_force: 30.0,
                min_collision_distance: 0.15,
                max_tilt_degrees: 35.0,
                max_acceleration: 2.0,
                human_proximity_allowed: true,
                max_arm_force: 10.0,
                max_joint_torque: 5.0,
                version: 1,
            },
            cycle_dt_ms: 1,
            max_torque: 2.0,
        }
    }
}

/// Result of a single pipeline step — full diagnostics for testing and
/// for the future trust dashboard.
#[derive(Clone, Debug)]
pub struct PipelineStepResult {
    /// The resulting physics state after executing the chosen action.
    pub state: PhysicsState,
    /// SHA-256 hash of the resulting state.
    pub cycle_hash: String,
    /// The action actually executed on the simulator.
    pub chosen_action: ActionVector,
    /// The L2+L1 evaluation that produced the chosen action, if any
    /// candidate was approved.
    pub evaluation: Option<ActionEvaluation>,
    /// True if no candidate was approved and the pipeline fell back to
    /// the conservative zero action.
    pub fallback_used: bool,
    /// Safety invariants violated this cycle (empty = clean).
    pub violated_invariants: Vec<String>,
    /// Set when this step completed a batch and committed it to L1 memory.
    pub committed_batch_root: Option<String>,
}

/// The reflex pipeline: L2 world model + L1 memory + L0 simulator, wired
/// into a single running loop.
pub struct ReflexPipeline {
    sim: Box<dyn PhysicsSimulator>,
    world_model: WorldModel,
    memory: MemoryFetch,
    config: PipelineConfig,
    current_state: PhysicsState,
    batch_states: Vec<PhysicsState>,
    batch_hashes: Vec<String>,
    batch_counter: u64,
    dataset: Option<DatasetExporter>,
}

impl ReflexPipeline {
    /// Construct a new pipeline. Steps the simulator once with dt=0 to
    /// obtain an initial state snapshot without advancing simulated time.
    pub fn new(
        mut sim: Box<dyn PhysicsSimulator>,
        world_model: WorldModel,
        memory: MemoryFetch,
        config: PipelineConfig,
    ) -> Self {
        let initial_state = sim.step(0);
        Self {
            sim,
            world_model,
            memory,
            config,
            current_state: initial_state,
            batch_states: Vec::new(),
            batch_hashes: Vec::new(),
            batch_counter: 0,
            dataset: None,
        }
    }

    /// Enable transition dataset export. Every subsequent `step()` records
    /// its (state_t, action, state_t1) transition; batch roots are backfilled
    /// automatically when a batch commits.
    pub fn enable_dataset_export(&mut self) {
        self.dataset = Some(DatasetExporter::new());
    }

    /// Access the dataset exporter, if enabled.
    pub fn dataset(&self) -> Option<&DatasetExporter> {
        self.dataset.as_ref()
    }

    /// Run one reflex cycle: L2 imagines each candidate, L1 checks memory,
    /// the safest approved action executes on L0.
    pub fn step(&mut self, candidates: &[ActionVector]) -> PipelineStepResult {
        let current = self.current_state.clone();

        let best = self
            .world_model
            .select_action(&current, candidates, &self.memory, self.config.epsilon);

        let (chosen_action, fallback_used) = match &best {
            Some(eval) => (eval.action.clone(), false),
            // No candidate approved (either too uncertain or matched a
            // red memory) — fall back to the conservative zero action.
            // This is L0's independence: it never waits on L1 or L2.
            None => (ActionVector::default(), true),
        };

        self.apply_action(&chosen_action);
        let new_state = self.sim.step(self.config.cycle_dt_ms);
        let cycle_hash = new_state.hash();

        let violated_invariants = check_invariants(&new_state, &self.config.envelope);
        let verdict = if violated_invariants.is_empty() { "green" } else { "red" };

        // Online training: every executed transition is a verified sample.
        let sample = TransitionSample {
            state_t: StateFeatures::from_state(&current),
            action: chosen_action.clone(),
            state_t1: StateFeatures::from_state(&new_state),
            verdict: Some(verdict.to_string()),
            batch_root: String::new(), // filled in retroactively at batch commit
            robot_id: self.sim.robot_id().to_string(),
        };
        self.world_model.train_step(&sample);

        if let Some(dataset) = &mut self.dataset {
            dataset.record(&sample, new_state.timestamp_ms);
        }

        self.batch_states.push(new_state.clone());
        self.batch_hashes.push(cycle_hash.clone());

        let committed_batch_root = if self.batch_states.len() >= self.config.batch_size {
            Some(self.commit_batch(verdict, &violated_invariants))
        } else {
            None
        };

        self.current_state = new_state.clone();

        PipelineStepResult {
            state: new_state,
            cycle_hash,
            chosen_action,
            evaluation: best,
            fallback_used,
            violated_invariants,
            committed_batch_root,
        }
    }

    /// Finalize the current batch: compute its Merkle root, index it into
    /// L1 memory, and push the root into the root cache — simulating what
    /// L3 consensus would do after finalizing the batch.
    fn commit_batch(&mut self, verdict: &str, violated_invariants: &[String]) -> String {
        let batch_id = format!("batch_{}", self.batch_counter);
        self.batch_counter += 1;

        let batch_root = compute_merkle_root(&self.batch_hashes);

        self.memory.index_mut().add_batch(
            &batch_id,
            self.sim.robot_id(),
            &self.batch_states,
            &self.batch_hashes,
            Some(verdict.to_string()),
            violated_invariants.to_vec(),
        );
        self.memory
            .root_cache_mut()
            .push(batch_root.clone(), self.batch_counter);

        self.batch_states.clear();
        self.batch_hashes.clear();

        if let Some(dataset) = &mut self.dataset {
            dataset.patch_batch_root(&batch_root);
        }

        batch_root
    }

    /// Convert an ActionVector into simulator torque commands.
    ///
    /// This is a simplified mapping (speed → forward torque, turn_rate →
    /// differential torque) sufficient to exercise the full L2→L1→L0 loop.
    /// `stride_scale` and `arm_position` are reserved for gait-specific
    /// backends and are not yet wired into the generic trait.
    fn apply_action(&mut self, action: &ActionVector) {
        let max_t = self.config.max_torque;
        let base = action.speed.clamp(-max_t, max_t);
        let turn = action.turn_rate.clamp(-max_t, max_t);
        let left = (base - turn).clamp(-max_t, max_t);
        let right = (base + turn).clamp(-max_t, max_t);
        self.sim.set_control(left, right);
    }

    /// Access the world model (e.g. to inspect uncertainty or pre-train).
    pub fn world_model(&self) -> &WorldModel {
        &self.world_model
    }

    /// Mutable access to the world model.
    pub fn world_model_mut(&mut self) -> &mut WorldModel {
        &mut self.world_model
    }

    /// Access the memory fetch layer (e.g. to import cross-fleet records).
    pub fn memory(&self) -> &MemoryFetch {
        &self.memory
    }

    /// Mutable access to the memory fetch layer.
    pub fn memory_mut(&mut self) -> &mut MemoryFetch {
        &mut self.memory
    }

    /// Current physics state (most recent executed cycle).
    pub fn current_state(&self) -> &PhysicsState {
        &self.current_state
    }

    /// Number of committed batches so far.
    pub fn batches_committed(&self) -> u64 {
        self.batch_counter
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryIndex, RootCache};
    use crate::merkle::compute_merkle_root;
    use crate::simulator::{QuadrupedBackend, QuadrupedConfig};
    use crate::worldmodel::TransitionSample;

    fn make_candidates() -> Vec<ActionVector> {
        vec![
            ActionVector { speed: 0.0, turn_rate: 0.0, stride_scale: 1.0, arm_position: 0.0 },
            ActionVector { speed: 0.5, turn_rate: 0.0, stride_scale: 1.0, arm_position: 0.0 },
            ActionVector { speed: 1.0, turn_rate: 0.0, stride_scale: 1.0, arm_position: 0.0 },
        ]
    }

    fn make_pipeline(batch_size: usize) -> ReflexPipeline {
        let sim = Box::new(QuadrupedBackend::new(
            "dogzilla-pipeline-test".to_string(),
            QuadrupedConfig::default(),
        ));
        let world_model = WorldModel::new();
        let memory = MemoryFetch::new(MemoryIndex::new(), RootCache::new(16));
        let config = PipelineConfig {
            batch_size,
            ..Default::default()
        };
        ReflexPipeline::new(sim, world_model, memory, config)
    }

    #[test]
    fn test_pipeline_steps_and_advances_state() {
        let mut pipeline = make_pipeline(1000);
        let candidates = make_candidates();

        let r1 = pipeline.step(&candidates);
        let r2 = pipeline.step(&candidates);

        assert!(r2.state.timestamp_ms > r1.state.timestamp_ms);
        assert_ne!(r1.cycle_hash, r2.cycle_hash);
    }

    #[test]
    fn test_pipeline_untrained_model_falls_back() {
        // Untrained world model has high uncertainty on every prediction,
        // so every candidate is rejected and the pipeline uses the
        // conservative fallback action. This mirrors the "L2 imagines but
        // isn't trusted yet" safety property.
        let mut pipeline = make_pipeline(1000);
        let candidates = make_candidates();

        let result = pipeline.step(&candidates);

        assert!(result.fallback_used, "untrained model should fall back");
        assert!(result.evaluation.is_none());
        assert_eq!(result.chosen_action.speed, 0.0, "fallback action should be zero");
    }

    #[test]
    fn test_pipeline_commits_batch_to_memory() {
        let mut pipeline = make_pipeline(10);
        let candidates = make_candidates();

        let mut committed = None;
        for _ in 0..10 {
            let r = pipeline.step(&candidates);
            if r.committed_batch_root.is_some() {
                committed = r.committed_batch_root;
            }
        }

        assert!(committed.is_some(), "batch should commit after batch_size cycles");
        assert_eq!(pipeline.batches_committed(), 1);
        assert_eq!(pipeline.memory().record_count(), 10);
    }

    #[test]
    fn test_pipeline_online_training_reduces_uncertainty() {
        let mut pipeline = make_pipeline(1000);
        let candidates = make_candidates();
        let initial_uncertainty = pipeline.world_model().uncertainty();

        for _ in 0..300 {
            pipeline.step(&candidates);
        }

        assert!(
            pipeline.world_model().uncertainty() < initial_uncertainty,
            "online training across steps should reduce uncertainty: {} -> {}",
            initial_uncertainty,
            pipeline.world_model().uncertainty()
        );
    }

    #[test]
    fn test_pipeline_rejects_candidate_near_seeded_red_memory() {
        // Seed L1 memory with a red-verdict state at high tilt/torque,
        // and pre-train L2 to be confident, so we can observe L1 actually
        // vetoing the world model's imagined outcome.
        let mut pipeline = make_pipeline(1000);

        // Pre-train the world model to be confident (low uncertainty) on
        // a consistent, safe transition pattern.
        for _ in 0..300 {
            let sample = TransitionSample {
                state_t: StateFeatures::from_state(pipeline.current_state()),
                action: ActionVector { speed: 0.1, ..Default::default() },
                state_t1: StateFeatures::from_state(pipeline.current_state()),
                verdict: Some("green".into()),
                batch_root: "seed".into(),
                robot_id: "dogzilla-pipeline-test".into(),
            };
            pipeline.world_model_mut().train_step(&sample);
        }

        // Seed a red memory that matches the "do nothing" state closely
        // (epsilon default = 5.0), so any candidate whose predicted
        // outcome lands near it gets vetoed.
        let red_state = pipeline.current_state().clone();
        let red_hash = red_state.hash();
        let red_hashes = vec![red_hash];
        pipeline.memory_mut().index_mut().add_batch(
            "seed_red_batch",
            "dogzilla-pipeline-test",
            &[red_state],
            &red_hashes,
            Some("red".into()),
            vec!["max_tilt".into()],
        );
        let red_root = compute_merkle_root(&red_hashes);
        pipeline.memory_mut().root_cache_mut().push(red_root, 1);

        let candidates = vec![ActionVector { speed: 0.0, ..Default::default() }];
        let result = pipeline.step(&candidates);

        // Either the candidate is rejected (fallback used) or, if approved,
        // it must not be a red match — the pipeline never executes an
        // action L1 flagged as matching a known-bad outcome.
        if let Some(eval) = &result.evaluation {
            assert!(!eval.red_match, "approved action must not match a red memory");
        } else {
            assert!(result.fallback_used);
        }
    }

    #[test]
    fn test_pipeline_dataset_export_end_to_end() {
        let mut pipeline = make_pipeline(20);
        pipeline.enable_dataset_export();
        let candidates = make_candidates();

        for _ in 0..40 {
            pipeline.step(&candidates);
        }

        let dataset = pipeline.dataset().expect("dataset export should be enabled");
        assert_eq!(dataset.len(), 40, "one record per step");
        assert!(dataset.all_roots_assigned(), "all batches should have committed and patched roots");

        let stats = dataset.stats();
        assert_eq!(stats.total_records, 40);
        assert_eq!(stats.unique_batches, 2, "40 steps / batch_size 20 = 2 batches");
        assert_eq!(stats.unique_robots, 1);

        let jsonl = dataset.to_jsonl();
        assert_eq!(jsonl.lines().count(), 40);

        let csv = dataset.to_csv();
        assert_eq!(csv.lines().count(), 41, "header + 40 rows");
    }
}
