//! Reflex batch attestation — runs a physics simulation for a batch of
//! reflex cycles, checks safety invariants, and produces a
//! `ReflexBatchAttestation` ready for on-chain anchoring.
//!
//! This is the integration point between the physics layer and the
//! coordination layer. The attestation produced here is what gets
//! submitted to the merkle-verifier contract and the circuit-breaker
//! contract on Juno.

use junoclaw_coordination::{ReflexBatchAttestation, SafetyEnvelope};
use tracing::{info, warn};

use crate::merkle::compute_merkle_root;
use crate::simulator::{now_ms, PhysicsSimulator};
use crate::state::PhysicsState;

/// Configuration for a reflex batch run.
#[derive(Clone, Debug)]
pub struct BatchConfig {
    /// Number of reflex cycles in this batch
    pub cycle_count: u32,
    /// Duration of each cycle in milliseconds (default 1ms = 1000Hz)
    pub cycle_dt_ms: u64,
    /// Safety envelope to check against
    pub envelope: SafetyEnvelope,
    /// Reference for the rosbag/log segment (for post-hoc verification)
    pub rosbag_ref: String,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            cycle_count: 1000,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "default".to_string(),
                max_speed: 2.0,
                max_force: 50.0,
                min_collision_distance: 0.3,
                max_tilt_degrees: 15.0,
                max_acceleration: 3.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "sim_batch".to_string(),
        }
    }
}

impl BatchConfig {
    /// Quadruped preset for DOGZILLA-Lite (15-DOF: 12 leg + 3 arm).
    /// Tuned for a 575g desktop robot with conservative safety margins.
    pub fn quadruped_preset(robot_id: &str) -> Self {
        Self {
            cycle_count: 1000,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: robot_id.to_string(),
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
            rosbag_ref: "quadruped_sim_batch".to_string(),
        }
    }
}

/// Result of running a reflex batch — the attestation plus diagnostic data.
#[derive(Clone, Debug)]
pub struct BatchResult {
    /// The `ReflexBatchAttestation` ready for on-chain submission
    pub attestation: ReflexBatchAttestation,
    /// All cycle hashes (for Merkle proof construction)
    pub cycle_hashes: Vec<String>,
    /// All physics states (for debugging / rosbag export)
    pub states: Vec<PhysicsState>,
    /// Which cycles had violations (cycle index, invariant name)
    pub violations: Vec<(u32, String)>,
}

/// Run a reflex batch: simulate `cycle_count` cycles, hash each, build
/// Merkle tree, check safety invariants, and produce a `ReflexBatchAttestation`.
///
/// The simulator should be pre-configured with control commands and obstacles
/// before calling this function.
pub fn run_reflex_batch(
    sim: &mut dyn PhysicsSimulator,
    config: &BatchConfig,
) -> BatchResult {
    let robot_id = sim.robot_id().to_string();
    let batch_start = now_ms();

    let mut states = Vec::with_capacity(config.cycle_count as usize);
    let mut cycle_hashes = Vec::with_capacity(config.cycle_count as usize);
    let mut violations = Vec::new();
    let mut all_maintained = true;

    for cycle in 0..config.cycle_count {
        let state = sim.step(config.cycle_dt_ms);

        // Check safety invariants
        let cycle_violations = check_invariants(&state, &config.envelope);
        if !cycle_violations.is_empty() {
            all_maintained = false;
            for v in &cycle_violations {
                violations.push((cycle, v.clone()));
                warn!(
                    "Safety violation at cycle {}: robot={}, violation={}",
                    cycle, robot_id, v
                );
            }
        }

        // Hash the physics state
        let hash = state.hash();
        cycle_hashes.push(hash);
        states.push(state);
    }

    let batch_end = now_ms();

    // Build Merkle root from cycle hashes
    let merkle_root = compute_merkle_root(&cycle_hashes);

    // Collect unique violated invariant names
    let violated_invariants: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        violations
            .iter()
            .filter(|(_, name)| seen.insert(name.clone()))
            .map(|(_, name)| name.clone())
            .collect()
    };

    let attestation = ReflexBatchAttestation {
        robot_id: robot_id.clone(),
        merkle_root,
        cycle_count: config.cycle_count,
        batch_start_timestamp: batch_start,
        batch_end_timestamp: batch_end,
        envelope_version: config.envelope.version,
        all_invariants_maintained: all_maintained,
        violated_invariants: violated_invariants.clone(),
        rosbag_ref: config.rosbag_ref.clone(),
    };

    info!(
        "Reflex batch complete: robot={}, cycles={}, maintained={}, violations={}, merkle_root={}",
        robot_id,
        config.cycle_count,
        all_maintained,
        violations.len(),
        &attestation.merkle_root[..16]
    );

    BatchResult {
        attestation,
        cycle_hashes,
        states,
        violations,
    }
}

/// Check physics state against safety envelope.
///
/// Returns a list of violated invariant names (empty if all maintained).
pub fn check_invariants(state: &PhysicsState, envelope: &SafetyEnvelope) -> Vec<String> {
    let mut violated = Vec::new();

    if state.sensors.speed > envelope.max_speed {
        violated.push("max_speed".to_string());
    }

    if state.sensors.max_force > envelope.max_force {
        violated.push("max_force".to_string());
    }

    if state.sensors.min_distance < envelope.min_collision_distance {
        violated.push("min_collision_distance".to_string());
    }

    if state.sensors.tilt_degrees > envelope.max_tilt_degrees {
        violated.push("max_tilt_degrees".to_string());
    }

    if state.sensors.acceleration > envelope.max_acceleration {
        violated.push("max_acceleration".to_string());
    }

    // Arm-specific invariants (only checked if envelope specifies limits)
    if envelope.max_arm_force > 0.0 && state.sensors.max_force > envelope.max_arm_force {
        violated.push("max_arm_force".to_string());
    }

    if envelope.max_joint_torque > 0.0 {
        for joint in &state.joints {
            if joint.torque.abs() > envelope.max_joint_torque {
                violated.push("max_joint_torque".to_string());
                break;
            }
        }
    }

    violated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{SimConfig, SimulatedBackend, QuadrupedBackend, QuadrupedConfig};

    #[test]
    fn test_clean_batch() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let config = BatchConfig {
            cycle_count: 100,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 10.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test_clean".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert!(result.attestation.all_invariants_maintained);
        assert!(result.attestation.violated_invariants.is_empty());
        assert_eq!(result.attestation.cycle_count, 100);
        assert_eq!(result.cycle_hashes.len(), 100);
        assert!(!result.attestation.merkle_root.is_empty());
        assert_eq!(result.attestation.merkle_root.len(), 64);
    }

    #[test]
    fn test_speed_violation() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Apply max torque to build up speed
        sim.set_control(2.5, 2.5);

        let config = BatchConfig {
            cycle_count: 500,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 0.5, // very low limit
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test_speed".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert!(!result.attestation.all_invariants_maintained);
        assert!(result.attestation.violated_invariants.contains(&"max_speed".to_string()));
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_collision_violation() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Place obstacle very close
        sim.add_obstacle(0.05, 0.0, 0.1);

        let config = BatchConfig {
            cycle_count: 10,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.5, // require 0.5m clearance
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test_collision".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert!(!result.attestation.all_invariants_maintained);
        assert!(result.attestation.violated_invariants.contains(&"min_collision_distance".to_string()));
    }

    #[test]
    fn test_merkle_root_deterministic() {
        let mut sim1 = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let mut sim2 = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        let config = BatchConfig {
            cycle_count: 50,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test".to_string(),
        };

        let result1 = run_reflex_batch(&mut sim1, &config);
        let result2 = run_reflex_batch(&mut sim2, &config);

        // Same initial conditions → same Merkle root
        assert_eq!(result1.attestation.merkle_root, result2.attestation.merkle_root);
    }

    #[test]
    fn test_merkle_root_changes_with_control() {
        let mut sim1 = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let mut sim2 = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        sim2.set_control(1.0, 1.0); // different control

        let config = BatchConfig {
            cycle_count: 100,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test".to_string(),
        };

        let result1 = run_reflex_batch(&mut sim1, &config);
        let result2 = run_reflex_batch(&mut sim2, &config);

        // Different control inputs → different Merkle roots
        assert_ne!(result1.attestation.merkle_root, result2.attestation.merkle_root);
    }

    #[test]
    fn test_attestation_has_violation_flag() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        sim.add_obstacle(0.01, 0.0, 0.1);

        let config = BatchConfig {
            cycle_count: 10,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 1.0,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert!(result.attestation.has_violation());
    }

    #[test]
    fn test_check_invariants_clean() {
        let state = PhysicsState {
            timestamp_ms: 0,
            joints: vec![],
            imu: crate::state::ImuReading { accel: [0.0; 3], gyro: [0.0; 3] },
            contacts: vec![],
            sensors: crate::state::SensorReadings {
                speed: 1.0,
                max_force: 10.0,
                min_distance: 1.0,
                tilt_degrees: 5.0,
                acceleration: 1.0,
            },
            com_position: [0.0; 3],
            com_velocity: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0],
        };

        let envelope = SafetyEnvelope {
            robot_id: "test".to_string(),
            max_speed: 2.0,
            max_force: 50.0,
            min_collision_distance: 0.3,
            max_tilt_degrees: 15.0,
            max_acceleration: 3.0,
            human_proximity_allowed: true,
            max_arm_force: 0.0,
            max_joint_torque: 0.0,
            version: 1,
        };

        let violated = check_invariants(&state, &envelope);
        assert!(violated.is_empty());
    }

    #[test]
    fn test_check_invariants_multiple_violations() {
        let state = PhysicsState {
            timestamp_ms: 0,
            joints: vec![],
            imu: crate::state::ImuReading { accel: [0.0; 3], gyro: [0.0; 3] },
            contacts: vec![],
            sensors: crate::state::SensorReadings {
                speed: 5.0,       // exceeds max_speed=2.0
                max_force: 100.0, // exceeds max_force=50.0
                min_distance: 0.1, // below min_collision_distance=0.3
                tilt_degrees: 30.0, // exceeds max_tilt=15.0
                acceleration: 10.0, // exceeds max_accel=3.0
            },
            com_position: [0.0; 3],
            com_velocity: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0],
        };

        let envelope = SafetyEnvelope {
            robot_id: "test".to_string(),
            max_speed: 2.0,
            max_force: 50.0,
            min_collision_distance: 0.3,
            max_tilt_degrees: 15.0,
            max_acceleration: 3.0,
            human_proximity_allowed: true,
            max_arm_force: 0.0,
            max_joint_torque: 0.0,
            version: 1,
        };

        let violated = check_invariants(&state, &envelope);
        assert_eq!(violated.len(), 5);
        assert!(violated.contains(&"max_speed".to_string()));
        assert!(violated.contains(&"max_force".to_string()));
        assert!(violated.contains(&"min_collision_distance".to_string()));
        assert!(violated.contains(&"max_tilt_degrees".to_string()));
        assert!(violated.contains(&"max_acceleration".to_string()));
    }

    #[test]
    fn test_batch_with_obstacle_course() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Set up obstacle course
        sim.add_obstacle(0.5, 0.0, 0.1);
        sim.add_obstacle(1.0, 0.3, 0.15);
        sim.add_obstacle(0.8, -0.2, 0.1);

        // Drive forward
        sim.set_control(1.5, 1.5);

        let config = BatchConfig {
            cycle_count: 1000,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 3.0,
                max_force: 80.0,
                min_collision_distance: 0.2,
                max_tilt_degrees: 20.0,
                max_acceleration: 5.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "obstacle_course_001".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert_eq!(result.attestation.cycle_count, 1000);
        assert_eq!(result.cycle_hashes.len(), 1000);
        assert_eq!(result.attestation.rosbag_ref, "obstacle_course_001");
        // Merkle root should be valid hex
        assert!(hex::decode(&result.attestation.merkle_root).is_ok());
    }

    #[test]
    fn test_batch_envelope_version_tracked() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        let config = BatchConfig {
            cycle_count: 10,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "robot-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 7, // specific version
            },
            rosbag_ref: "test".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);
        assert_eq!(result.attestation.envelope_version, 7);
    }

    // --- Quadruped-specific attestation tests ---

    #[test]
    fn test_quadruped_preset_values() {
        let config = BatchConfig::quadruped_preset("dogzilla-lite-001");
        assert_eq!(config.envelope.max_speed, 1.5);
        assert_eq!(config.envelope.max_tilt_degrees, 35.0);
        assert_eq!(config.envelope.max_arm_force, 10.0);
        assert_eq!(config.envelope.max_joint_torque, 5.0);
        assert_eq!(config.envelope.robot_id, "dogzilla-lite-001");
        assert_eq!(config.rosbag_ref, "quadruped_sim_batch");
    }

    #[test]
    fn test_quadruped_clean_batch() {
        let mut sim = QuadrupedBackend::new(
            "dogzilla-lite-001".to_string(),
            QuadrupedConfig::default(),
        );
        let config = BatchConfig::quadruped_preset("dogzilla-lite-001");

        let result = run_reflex_batch(&mut sim, &config);

        assert!(result.attestation.all_invariants_maintained,
            "clean quadruped batch should maintain all invariants, violations: {:?}",
            result.attestation.violated_invariants);
        assert!(result.attestation.violated_invariants.is_empty());
        assert_eq!(result.attestation.cycle_count, 1000);
        assert_eq!(result.cycle_hashes.len(), 1000);
        assert!(!result.attestation.merkle_root.is_empty());
        assert_eq!(result.attestation.merkle_root.len(), 64);
        // All states should have 15 joints
        assert!(result.states.iter().all(|s| s.joints.len() == 15),
            "all states should have 15 joints");
    }

    #[test]
    fn test_quadruped_joint_torque_violation() {
        let mut sim = QuadrupedBackend::new(
            "dogzilla-lite-001".to_string(),
            QuadrupedConfig::default(),
        );

        // Set torque on all joints above the 5.0 N·m limit
        let torques = [6.0; 15];
        // But arm joints can't exceed max_joint_torque in config (5.0)
        // The QuadrupedConfig clamps to 5.0, so we need to lower the envelope
        sim.set_joint_controls(&torques);

        let config = BatchConfig {
            cycle_count: 100,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "dogzilla-lite-001".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 3.0, // lower than the 5.0 clamp
                version: 1,
            },
            rosbag_ref: "test_quadruped_torque".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        // Torques are clamped to 5.0 by the simulator, but envelope limit is 3.0
        assert!(!result.attestation.all_invariants_maintained,
            "should detect torque violation");
        assert!(result.attestation.violated_invariants.contains(&"max_joint_torque".to_string()),
            "violations: {:?}", result.attestation.violated_invariants);
    }

    #[test]
    fn test_quadruped_arm_force_violation() {
        let mut sim = QuadrupedBackend::new(
            "dogzilla-lite-001".to_string(),
            QuadrupedConfig::default(),
        );

        // Set high torque on arm_shoulder joint to trigger arm force
        let mut torques = [0.0; 15];
        torques[13] = 5.0; // arm_shoulder at max
        sim.set_joint_controls(&torques);

        // Use a very low arm force limit
        let config = BatchConfig {
            cycle_count: 100,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "dogzilla-lite-001".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.1, // very low limit
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test_quadruped_arm".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        // arm_force = torque * 0.1 = 5.0 * 0.1 = 0.5, which exceeds 0.1
        assert!(!result.attestation.all_invariants_maintained,
            "should detect arm force violation");
        assert!(result.attestation.violated_invariants.contains(&"max_arm_force".to_string()),
            "violations: {:?}", result.attestation.violated_invariants);
    }

    #[test]
    fn test_quadruped_tilt_violation() {
        let mut sim = QuadrupedBackend::new(
            "dogzilla-lite-001".to_string(),
            QuadrupedConfig::default(),
        );

        // Forward + turn to induce body pitch and roll
        sim.set_control(2.0, 0.5);

        let config = BatchConfig {
            cycle_count: 500,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "dogzilla-lite-001".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 0.0, // zero limit: any nonzero tilt violates
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test_quadruped_tilt".to_string(),
        };

        let result = run_reflex_batch(&mut sim, &config);

        assert!(!result.attestation.all_invariants_maintained,
            "should detect tilt violation with 0° limit");
        assert!(result.attestation.violated_invariants.contains(&"max_tilt_degrees".to_string()),
            "violations: {:?}", result.attestation.violated_invariants);
    }

    #[test]
    fn test_quadruped_merkle_root_deterministic() {
        let config = BatchConfig::quadruped_preset("dog-1");

        let mut sim1 = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        let result1 = run_reflex_batch(&mut sim1, &config);

        let mut sim2 = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        let result2 = run_reflex_batch(&mut sim2, &config);

        assert_eq!(result1.attestation.merkle_root, result2.attestation.merkle_root,
            "same config should produce same merkle root");
    }

    #[test]
    fn test_quadruped_differs_from_wheeled() {
        let mut quad_sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        let mut wheeled_sim = SimulatedBackend::new(
            "dog-1".to_string(),
            SimConfig::default(),
        );

        let config = BatchConfig {
            cycle_count: 100,
            cycle_dt_ms: 1,
            envelope: SafetyEnvelope {
                robot_id: "dog-1".to_string(),
                max_speed: 10.0,
                max_force: 100.0,
                min_collision_distance: 0.01,
                max_tilt_degrees: 45.0,
                max_acceleration: 100.0,
                human_proximity_allowed: true,
                max_arm_force: 0.0,
                max_joint_torque: 0.0,
                version: 1,
            },
            rosbag_ref: "test".to_string(),
        };

        let quad_result = run_reflex_batch(&mut quad_sim, &config);
        let wheeled_result = run_reflex_batch(&mut wheeled_sim, &config);

        assert_ne!(quad_result.attestation.merkle_root, wheeled_result.attestation.merkle_root,
            "quadruped and wheeled should produce different merkle roots");
    }
}
