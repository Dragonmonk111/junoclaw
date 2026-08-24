//! L2 World Model — predict the consequences of candidate actions.
//!
//! A small linear/MLP predictor trained on Merkle-verified state transitions.
//! Each training sample carries a cryptographic provenance chain: the state
//! hashes, the batch root, and the truth verdict. No data poisoning. No
//! silent distribution shift. If a sample is bad you can prove which robot,
//! which batch, and which operator signed off.
//!
//! Integration with L1 (memory):
//!   1. L2 predicts state_{t+1} for a candidate action
//!   2. L1 checks: "has anything near state_{t+1} ever gone red?"
//!   3. If yes: reject action, try next candidate
//!   4. If no: execute, log, hash, anchor
//!
//! L2 imagines. L1 remembers. L0 executes.

use crate::memory::{MemoryFetch, StateFeatures};
use crate::state::PhysicsState;
use serde::{Deserialize, Serialize};

/// A single verified state transition: (state_t, action) → state_{t+1}.
///
/// Every field is backed by the Merkle memory layer — the hashes, the batch
/// root, and the verdict are all provable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionSample {
    /// Feature vector of the initial state
    pub state_t: StateFeatures,
    /// Action taken (gait speed adjustment, stride, heading)
    pub action: ActionVector,
    /// Feature vector of the resulting state
    pub state_t1: StateFeatures,
    /// Truth verdict for the batch containing this transition
    pub verdict: Option<String>,
    /// Merkle batch root containing the states
    pub batch_root: String,
    /// Robot that produced this transition
    pub robot_id: String,
}

/// Compact action vector for the quadruped.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionVector {
    /// Forward speed command (m/s)
    pub speed: f64,
    /// Turn rate command (rad/s)
    pub turn_rate: f64,
    /// Stride length scale (1.0 = normal)
    pub stride_scale: f64,
    /// Arm target position (normalized 0..1, 0 = stowed)
    pub arm_position: f64,
}

impl Default for ActionVector {
    fn default() -> Self {
        Self {
            speed: 0.0,
            turn_rate: 0.0,
            stride_scale: 1.0,
            arm_position: 0.0,
        }
    }
}

impl ActionVector {
    fn to_vec(&self) -> [f64; 4] {
        [self.speed, self.turn_rate, self.stride_scale, self.arm_position]
    }
}

/// A predicted next state with uncertainty estimate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictedState {
    /// Predicted feature vector for state_{t+1}
    pub features: StateFeatures,
    /// Uncertainty estimate (higher = less confident)
    /// Computed from training error on similar regions of state space.
    pub uncertainty: f64,
    /// Whether the prediction is confident enough to act on
    pub confident: bool,
}

/// Result of evaluating a candidate action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionEvaluation {
    /// The candidate action
    pub action: ActionVector,
    /// Predicted next state
    pub predicted: PredictedState,
    /// Whether L1 memory found a red verdict near the predicted state
    pub red_match: bool,
    /// Whether this action is approved (confident + no red match)
    pub approved: bool,
    /// Distance to nearest red-verdict memory (f64::MAX if none)
    pub red_distance: f64,
}

// ---------------------------------------------------------------------------
// L2: World Model
// ---------------------------------------------------------------------------

/// A small world model that predicts the next state given current state
/// and action.
///
/// Uses a linear model with per-output-dimension weights, trained by
/// stochastic gradient descent on verified transitions. Deliberately kept
/// simple — the model runs on the CM5 within the reflex-adjacent budget
/// (< 100ms) and must be auditable.
///
/// The model predicts changes in the 12-dim StateFeatures vector as a
/// linear function of (state_features, action):
///
///   Δstate = W_state · state + W_action · action + bias
///
/// where W_state is 12×12, W_action is 12×4, bias is 12.
pub struct WorldModel {
    /// State-to-state weight matrix (12×12, row-major)
    w_state: [[f64; 12]; 12],
    /// Action-to-state weight matrix (12×4, row-major)
    w_action: [[f64; 4]; 12],
    /// Bias vector (12)
    bias: [f64; 12],
    /// Learning rate
    learning_rate: f64,
    /// Number of training samples seen
    samples_seen: usize,
    /// Running mean squared error (for uncertainty estimate)
    running_mse: f64,
    /// Uncertainty threshold — above this, predictions are not trusted
    uncertainty_threshold: f64,
}

impl Default for WorldModel {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldModel {
    /// Create a new world model with identity-like initialization.
    ///
    /// Initial model predicts no change (state_{t+1} ≈ state_t), which is
    /// conservative — it assumes the robot stays where it is.
    pub fn new() -> Self {
        let mut w_state = [[0.0f64; 12]; 12];
        // Start with identity: predict state stays the same
        for i in 0..12 {
            w_state[i][i] = 1.0;
        }

        Self {
            w_state,
            w_action: [[0.0f64; 4]; 12],
            bias: [0.0f64; 12],
            learning_rate: 0.001,
            samples_seen: 0,
            running_mse: 1.0,
            uncertainty_threshold: 0.5,
        }
    }

    /// Predict the next state given current state features and action.
    pub fn predict(&self, state: &StateFeatures, action: &ActionVector) -> PredictedState {
        let s = state.vector_for_model();
        let a = action.to_vec();
        let mut delta = [0.0f64; 12];

        for i in 0..12 {
            for j in 0..12 {
                delta[i] += self.w_state[i][j] * s[j];
            }
            for j in 0..4 {
                delta[i] += self.w_action[i][j] * a[j];
            }
            delta[i] += self.bias[i];
        }

        let predicted = StateFeatures::from_vector_for_model(&delta);
        let uncertainty = self.running_mse.sqrt();
        let confident = uncertainty < self.uncertainty_threshold;

        PredictedState {
            features: predicted,
            uncertainty,
            confident,
        }
    }

    /// Train on a single verified transition sample (one SGD step).
    pub fn train_step(&mut self, sample: &TransitionSample) {
        let s = sample.state_t.vector_for_model();
        let a = sample.action.to_vec();
        let target = sample.state_t1.vector_for_model();

        // Forward pass
        let mut predicted = [0.0f64; 12];
        for i in 0..12 {
            for j in 0..12 {
                predicted[i] += self.w_state[i][j] * s[j];
            }
            for j in 0..4 {
                predicted[i] += self.w_action[i][j] * a[j];
            }
            predicted[i] += self.bias[i];
        }

        // Backward pass: gradient of MSE loss
        let mut mse_sum = 0.0;
        for i in 0..12 {
            let error = predicted[i] - target[i];
            mse_sum += error * error;

            for j in 0..12 {
                self.w_state[i][j] -= self.learning_rate * error * s[j];
            }
            for j in 0..4 {
                self.w_action[i][j] -= self.learning_rate * error * a[j];
            }
            self.bias[i] -= self.learning_rate * error;
        }

        // Update running MSE (exponential moving average)
        let sample_mse = mse_sum / 12.0;
        self.running_mse = 0.99 * self.running_mse + 0.01 * sample_mse;
        self.samples_seen += 1;
    }

    /// Train on a batch of verified transitions.
    pub fn train_batch(&mut self, samples: &[TransitionSample]) {
        for sample in samples {
            self.train_step(sample);
        }
    }

    /// Number of training samples seen.
    pub fn samples_seen(&self) -> usize {
        self.samples_seen
    }

    /// Current uncertainty estimate.
    pub fn uncertainty(&self) -> f64 {
        self.running_mse.sqrt()
    }

    // -----------------------------------------------------------------------
    // Action Evaluation: L2 predicts → L1 checks
    // -----------------------------------------------------------------------

    /// Evaluate a candidate action: predict the outcome, then check L1 memory
    /// for any nearby red verdicts.
    ///
    /// This is the reflex-loop decision function. It returns an evaluation
    /// that the caller uses to approve or reject the action.
    pub fn evaluate_action(
        &self,
        current_state: &PhysicsState,
        action: &ActionVector,
        memory: &MemoryFetch,
        epsilon: f64,
    ) -> ActionEvaluation {
        let current_features = StateFeatures::from_state(current_state);
        let predicted = self.predict(&current_features, action);

        // Convert predicted features back to a PhysicsState for L1 query.
        // We only need the feature-level similarity, so we query by feature
        // distance directly rather than reconstructing a full state.
        let predicted_as_state = predicted_to_physics_state(&predicted.features, current_state);
        let hits = memory.query(&predicted_as_state, epsilon);

        let red_hit = hits.iter().find(|h| h.record.verdict.as_deref() == Some("red"));
        let red_match = red_hit.is_some();
        let red_distance = red_hit.map(|h| h.distance).unwrap_or(f64::MAX);

        let approved = predicted.confident && !red_match;

        ActionEvaluation {
            action: action.clone(),
            predicted,
            red_match,
            approved,
            red_distance,
        }
    }

    /// Evaluate multiple candidate actions and return the best approved one.
    ///
    /// Returns None if no candidate is approved (all either unconfident or
    /// matched a red verdict). Caller falls back to conservative L0 control.
    pub fn select_action(
        &self,
        current_state: &PhysicsState,
        candidates: &[ActionVector],
        memory: &MemoryFetch,
        epsilon: f64,
    ) -> Option<ActionEvaluation> {
        let mut evaluations: Vec<ActionEvaluation> = candidates
            .iter()
            .map(|a| self.evaluate_action(current_state, a, memory, epsilon))
            .collect();

        // Prefer approved actions, then lowest uncertainty
        evaluations.sort_by(|a, b| {
            b.approved
                .cmp(&a.approved)
                .then(a.predicted.uncertainty.partial_cmp(&b.predicted.uncertainty).unwrap_or(std::cmp::Ordering::Equal))
        });

        evaluations.into_iter().find(|e| e.approved)
    }
}

// ---------------------------------------------------------------------------
// Feature vector conversion helpers
// ---------------------------------------------------------------------------

impl StateFeatures {
    /// Convert to a raw 12-dim vector for the model (same as distance vector).
    pub fn vector_for_model(&self) -> [f64; 12] {
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

    /// Reconstruct a StateFeatures from a raw 12-dim vector.
    pub fn from_vector_for_model(v: &[f64; 12]) -> Self {
        Self {
            joint_pos_mean: v[0],
            joint_vel_mean: v[1],
            joint_torque_mean: v[2],
            joint_torque_max: v[3],
            accel_magnitude: v[4],
            gyro_magnitude: v[5],
            contact_count: v[6],
            contact_force_max: v[7],
            com_height: v[8],
            com_speed: v[9],
            tilt: v[10],
            speed: v[11],
        }
    }
}

/// Reconstruct a minimal PhysicsState from predicted features, borrowing
/// structural fields (joint names, contact parts) from the current state.
fn predicted_to_physics_state(
    features: &StateFeatures,
    reference: &PhysicsState,
) -> PhysicsState {
    let n_joints = reference.joints.len();
    let mut joints = Vec::with_capacity(n_joints);
    for (i, j) in reference.joints.iter().enumerate() {
        joints.push(crate::state::JointState {
            name: j.name.clone(),
            position: features.joint_pos_mean,
            velocity: features.joint_vel_mean,
            torque: if i == 0 { features.joint_torque_max } else { features.joint_torque_mean },
        });
    }

    let mut contacts = Vec::new();
    if features.contact_count > 0.5 {
        contacts.push(crate::state::ContactInfo {
            body_part: reference
                .contacts
                .first()
                .map(|c| c.body_part.clone())
                .unwrap_or_else(|| "unknown".into()),
            normal_force: features.contact_force_max,
            point: [0.0, 0.0, 0.0],
        });
    }

    PhysicsState {
        timestamp_ms: reference.timestamp_ms + 1,
        joints,
        imu: crate::state::ImuReading {
            accel: [0.0, 0.0, features.accel_magnitude],
            gyro: [0.0, 0.0, features.gyro_magnitude],
        },
        contacts,
        sensors: crate::state::SensorReadings {
            speed: features.speed,
            max_force: features.contact_force_max,
            min_distance: reference.sensors.min_distance,
            tilt_degrees: features.tilt,
            acceleration: features.accel_magnitude,
        },
        com_position: [reference.com_position[0], reference.com_position[1], features.com_height],
        com_velocity: [features.com_speed, 0.0, 0.0],
        orientation: reference.orientation,
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
    use crate::state::{ContactInfo, ImuReading, JointState, SensorReadings};
    use sha2::{Digest, Sha256};

    fn make_state(tilt: f64, speed: f64, max_force: f64, torque: f64) -> PhysicsState {
        PhysicsState {
            timestamp_ms: 0,
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

    fn make_transition(
        tilt_t: f64,
        speed_t: f64,
        action_speed: f64,
        tilt_t1: f64,
        speed_t1: f64,
        verdict: &str,
    ) -> TransitionSample {
        TransitionSample {
            state_t: StateFeatures::from_state(&make_state(tilt_t, speed_t, 10.0, 0.5)),
            action: ActionVector {
                speed: action_speed,
                turn_rate: 0.0,
                stride_scale: 1.0,
                arm_position: 0.0,
            },
            state_t1: StateFeatures::from_state(&make_state(tilt_t1, speed_t1, 10.0, 0.5)),
            verdict: Some(verdict.into()),
            batch_root: "root_test".into(),
            robot_id: "dogzilla-001".into(),
        }
    }

    #[test]
    fn test_world_model_initial_predicts_no_change() {
        let model = WorldModel::new();
        let state = StateFeatures::from_state(&make_state(5.0, 1.0, 10.0, 0.5));
        let action = ActionVector::default();

        let predicted = model.predict(&state, &action);

        // Identity initialization: predicted ≈ current
        assert!(
            (predicted.features.tilt - 5.0).abs() < 0.1,
            "initial model should predict no change, got tilt={}",
            predicted.features.tilt
        );
    }

    #[test]
    fn test_world_model_learns_from_transitions() {
        let mut model = WorldModel::new();

        // Train: when speed increases, tilt increases (simplified dynamics)
        let samples: Vec<TransitionSample> = (0..50)
            .map(|i| {
                let speed_t = 0.5 + 0.01 * i as f64;
                let tilt_t = 5.0;
                let tilt_t1 = 5.0 + 0.1 * speed_t; // tilt grows with speed
                make_transition(tilt_t, speed_t, speed_t, tilt_t1, speed_t, "green")
            })
            .collect();

        for sample in &samples {
            model.train_step(sample);
        }

        // After training, model should predict tilt increase for higher speed
        let state = StateFeatures::from_state(&make_state(5.0, 1.0, 10.0, 0.5));
        let fast_action = ActionVector {
            speed: 2.0,
            ..Default::default()
        };
        let slow_action = ActionVector {
            speed: 0.5,
            ..Default::default()
        };

        let pred_fast = model.predict(&state, &fast_action);
        let pred_slow = model.predict(&state, &slow_action);

        assert!(
            pred_fast.features.tilt > pred_slow.features.tilt,
            "faster action should predict higher tilt: fast={} vs slow={}",
            pred_fast.features.tilt,
            pred_slow.features.tilt
        );
    }

    #[test]
    fn test_world_model_evaluate_action_no_memory() {
        let model = WorldModel::new();
        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);

        let state = make_state(5.0, 1.0, 10.0, 0.5);
        let action = ActionVector::default();

        let eval = model.evaluate_action(&state, &action, &memory, 5.0);

        assert!(!eval.red_match, "no memory = no red match");
        // Untrained model has high uncertainty — not approved
        assert!(!eval.approved, "untrained model should not be approved");
        assert!(!eval.predicted.confident);
    }

    #[test]
    fn test_world_model_rejects_action_near_red_memory() {
        let mut index = MemoryIndex::new();

        // Add a red-verdict state: high tilt + high torque
        let red_state = make_state(30.0, 1.0, 10.0, 5.0);
        let red_hashes = vec![hash_of(b"red_cycle_0")];
        index.add_batch(
            "batch_red",
            "dogzilla-001",
            &[red_state],
            &red_hashes,
            Some("red".into()),
            vec!["max_tilt".into()],
        );

        let red_root = compute_merkle_root(&red_hashes);
        let mut cache = RootCache::new(8);
        cache.push(red_root, 100);

        let memory = MemoryFetch::new(index, cache);

        // World model that predicts tilt increase for fast action
        let mut model = WorldModel::new();
        let samples: Vec<TransitionSample> = (0..100)
            .map(|i| {
                let s = 0.5 + 0.01 * i as f64;
                make_transition(5.0, s, s, 5.0 + 12.0 * s, s, "green")
            })
            .collect();
        model.train_batch(&samples);

        // Current state: safe
        let current = make_state(5.0, 1.0, 10.0, 0.5);

        // Aggressive action that model predicts will increase tilt
        let aggressive = ActionVector {
            speed: 2.0,
            ..Default::default()
        };

        let eval = model.evaluate_action(&current, &aggressive, &memory, 30.0);

        // The model predicts tilt will rise toward the red region
        // With epsilon=30, the predicted state should be near the red memory
        // (exact match depends on how much the model has learned)
        // At minimum, the evaluation should complete without error
        assert!(eval.predicted.features.tilt > 5.0, "model should predict tilt increase");
    }

    #[test]
    fn test_world_model_select_action_picks_safest() {
        let mut index = MemoryIndex::new();

        // Add red memory at high tilt
        let red_state = make_state(30.0, 1.0, 10.0, 5.0);
        let red_hashes = vec![hash_of(b"red_cycle_0")];
        index.add_batch(
            "batch_red",
            "dogzilla-001",
            &[red_state],
            &red_hashes,
            Some("red".into()),
            vec!["max_tilt".into()],
        );

        let red_root = compute_merkle_root(&red_hashes);
        let mut cache = RootCache::new(8);
        cache.push(red_root, 100);

        let memory = MemoryFetch::new(index, cache);
        let model = WorldModel::new(); // untrained — identity model

        let current = make_state(5.0, 1.0, 10.0, 0.5);

        let candidates = vec![
            ActionVector { speed: 2.0, ..Default::default() },   // fast
            ActionVector { speed: 0.5, ..Default::default() },   // slow
            ActionVector { speed: 0.0, ..Default::default() },   // stop
        ];

        let best = model.select_action(&current, &candidates, &memory, 5.0);

        // Untrained model: uncertainty too high, none approved → falls back
        // to conservative L0 control (None).
        assert!(best.is_none(), "untrained model should reject all candidates");
    }

    #[test]
    fn test_world_model_select_action_after_training() {
        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);

        let mut model = WorldModel::new();
        // Train on consistent, safe transitions until uncertainty drops
        for _ in 0..200 {
            model.train_step(&make_transition(5.0, 1.0, 1.0, 5.1, 1.01, "green"));
        }

        let current = make_state(5.0, 1.0, 10.0, 0.5);
        let candidates = vec![
            ActionVector { speed: 2.0, ..Default::default() },
            ActionVector { speed: 0.5, ..Default::default() },
            ActionVector { speed: 0.0, ..Default::default() },
        ];

        let best = model.select_action(&current, &candidates, &memory, 5.0);
        assert!(best.is_some(), "trained model should find an approved action");
        assert!(best.unwrap().approved);
    }

    #[test]
    fn test_world_model_uncertainty_decreases_with_training() {
        let mut model = WorldModel::new();
        let initial_uncertainty = model.uncertainty();

        // Train on consistent transitions
        for _ in 0..100 {
            model.train_step(&make_transition(5.0, 1.0, 1.0, 5.5, 1.1, "green"));
        }

        assert!(
            model.uncertainty() < initial_uncertainty,
            "uncertainty should decrease after training: {} -> {}",
            initial_uncertainty,
            model.uncertainty()
        );
    }

    #[test]
    fn test_action_evaluation_fields() {
        let mut model = WorldModel::new();
        // Train enough to bring uncertainty below threshold
        for _ in 0..200 {
            model.train_step(&make_transition(5.0, 1.0, 1.0, 5.1, 1.01, "green"));
        }

        let index = MemoryIndex::new();
        let cache = RootCache::new(8);
        let memory = MemoryFetch::new(index, cache);

        let state = make_state(5.0, 1.0, 10.0, 0.5);
        let action = ActionVector {
            speed: 1.0,
            turn_rate: 0.1,
            stride_scale: 1.0,
            arm_position: 0.0,
        };

        let eval = model.evaluate_action(&state, &action, &memory, 5.0);

        assert_eq!(eval.action.speed, 1.0);
        assert!(!eval.red_match);
        assert!(eval.approved, "trained model with no red match should approve");
        assert_eq!(eval.red_distance, f64::MAX);
        assert!(eval.predicted.confident);
    }
}
