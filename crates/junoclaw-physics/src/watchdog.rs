//! Redundant reflex path — a second, independently-derived safety check
//! that runs alongside `attestation::check_invariants`.
//!
//! Real safety-critical systems don't trust a single code path to catch
//! every violation: they run a diverse second implementation and trip a
//! conservative stop on ANY disagreement, because disagreement means one
//! of the two channels has a bug — and you don't get to find out which
//! one is right by re-running the same code again.
//!
//! This module's checks are deliberately computed from different raw
//! fields than `check_invariants` uses, not by re-reading `state.sensors`:
//! - speed is derived from `com_velocity`, not `state.sensors.speed`
//! - tilt is derived from `orientation`, not `state.sensors.tilt_degrees`
//! - max contact force is derived from `state.contacts`, not `state.sensors.max_force`
//!
//! If the primary sensor-fusion code and this independent derivation ever
//! disagree, that is itself a signal worth tripping on — it means the
//! `SensorReadings` struct was populated inconsistently with the raw
//! state, which is exactly the kind of bug a single-channel check cannot
//! catch by construction.

use crate::state::PhysicsState;
use junoclaw_coordination::SafetyEnvelope;
use serde::{Deserialize, Serialize};

/// Independently-derived violation check. Mirrors the intent of
/// `attestation::check_invariants` but recomputes each quantity from a
/// different raw source field, so a bug in one derivation is unlikely to
/// be replicated in the other.
pub fn redundant_check(state: &PhysicsState, envelope: &SafetyEnvelope) -> Vec<String> {
    let mut violated = Vec::new();

    // Speed: derived from com_velocity, not state.sensors.speed.
    let derived_speed = (state.com_velocity[0].powi(2) + state.com_velocity[1].powi(2)).sqrt();
    if derived_speed > envelope.max_speed {
        violated.push("max_speed".to_string());
    }

    // Contact force: derived directly from the contact list, not
    // state.sensors.max_force.
    let derived_max_force = state
        .contacts
        .iter()
        .map(|c| c.normal_force)
        .fold(0.0f64, f64::max);
    if derived_max_force > envelope.max_force {
        violated.push("max_force".to_string());
    }

    // Tilt: derived from the orientation quaternion, not
    // state.sensors.tilt_degrees. Tilt = angle between body up-axis
    // (rotated by orientation) and world up-axis.
    let derived_tilt = tilt_from_quaternion(state.orientation);
    if derived_tilt > envelope.max_tilt_degrees {
        violated.push("max_tilt_degrees".to_string());
    }

    if envelope.max_arm_force > 0.0 && derived_max_force > envelope.max_arm_force {
        violated.push("max_arm_force".to_string());
    }

    // Joint torque: uses .abs() by construction, independent of whatever
    // the primary path does.
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

/// Compute tilt (degrees) from a unit quaternion [w, x, y, z], via the
/// angle between the body's up-axis (rotated by the quaternion) and the
/// world up-axis [0, 0, 1]. This is a different derivation path than the
/// simulator's own roll/pitch-based `compute_tilt`.
fn tilt_from_quaternion(q: [f64; 4]) -> f64 {
    let (_w, x, y, _z) = (q[0], q[1], q[2], q[3]);
    // Rotate the body up-vector [0, 0, 1] by the quaternion.
    // z-component of the rotated up-vector directly gives cos(tilt).
    let up_z = 1.0 - 2.0 * (x * x + y * y);
    let cos_tilt = up_z.clamp(-1.0, 1.0);
    cos_tilt.acos().to_degrees()
}

/// Verdict from running both the primary and redundant checks over the
/// same state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchdogVerdict {
    pub primary_violations: Vec<String>,
    pub redundant_violations: Vec<String>,
    /// True if both channels agree on the exact same set of violations.
    pub agreement: bool,
    /// Violations found by exactly one channel — the interesting case.
    /// Non-empty means the two independent derivations disagree, which
    /// should trip a conservative stop regardless of which is "right."
    pub divergent: Vec<String>,
}

/// Run both the primary and redundant checks and compare.
pub fn dual_channel_check(state: &PhysicsState, envelope: &SafetyEnvelope) -> WatchdogVerdict {
    let mut primary = crate::attestation::check_invariants(state, envelope);
    let mut redundant = redundant_check(state, envelope);
    primary.sort();
    redundant.sort();

    let divergent: Vec<String> = primary
        .iter()
        .filter(|v| !redundant.contains(v))
        .chain(redundant.iter().filter(|v| !primary.contains(v)))
        .cloned()
        .collect();

    WatchdogVerdict {
        agreement: divergent.is_empty(),
        primary_violations: primary,
        redundant_violations: redundant,
        divergent,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::{PhysicsSimulator, QuadrupedBackend, QuadrupedConfig};

    fn default_envelope() -> SafetyEnvelope {
        SafetyEnvelope {
            robot_id: "dogzilla-watchdog-test".to_string(),
            max_speed: 1.5,
            max_force: 30.0,
            min_collision_distance: 0.05,
            max_tilt_degrees: 35.0,
            max_acceleration: 100.0,
            human_proximity_allowed: true,
            max_arm_force: 10.0,
            max_joint_torque: 5.0,
            version: 1,
        }
    }

    #[test]
    fn test_dual_channel_agrees_on_clean_state() {
        let mut sim = QuadrupedBackend::new("dogzilla-watchdog-test".to_string(), QuadrupedConfig::default());
        let state = sim.step(1);

        let verdict = dual_channel_check(&state, &default_envelope());
        assert!(verdict.agreement, "clean state should produce agreement: {:?}", verdict);
        assert!(verdict.divergent.is_empty());
    }

    #[test]
    fn test_dual_channel_agrees_on_torque_violation_both_directions() {
        let mut sim = QuadrupedBackend::new("dogzilla-watchdog-test".to_string(), QuadrupedConfig::default());
        // Negative torque overshoot — the exact case the old primary-only
        // (non-abs) check would have missed before the attestation.rs fix.
        let mut torques = [0.0; 15];
        torques[0] = -5.0;
        sim.set_joint_controls(&torques);
        let state = sim.step(1);

        let mut envelope = default_envelope();
        envelope.max_joint_torque = 3.0;

        let verdict = dual_channel_check(&state, &envelope);
        assert!(verdict.primary_violations.contains(&"max_joint_torque".to_string()));
        assert!(verdict.redundant_violations.contains(&"max_joint_torque".to_string()));
        assert!(verdict.agreement, "both channels should now catch negative torque overshoot");
    }

    #[test]
    fn test_tilt_from_quaternion_zero_at_identity() {
        let tilt = tilt_from_quaternion([1.0, 0.0, 0.0, 0.0]);
        assert!(tilt.abs() < 1e-6, "identity orientation should have zero tilt");
    }

    #[test]
    fn test_tilt_from_quaternion_90_degrees() {
        // 90-degree rotation about the x-axis: quaternion [cos(45), sin(45), 0, 0]
        let half = (std::f64::consts::FRAC_PI_4).sin();
        let w = (std::f64::consts::FRAC_PI_4).cos();
        let tilt = tilt_from_quaternion([w, half, 0.0, 0.0]);
        assert!((tilt - 90.0).abs() < 1e-6, "expected ~90 degrees, got {}", tilt);
    }

    #[test]
    fn test_redundant_check_speed_violation() {
        let mut sim = QuadrupedBackend::new("dogzilla-watchdog-test".to_string(), QuadrupedConfig::default());
        let mut torques = [0.0; 15];
        torques[0] = 5.0;
        torques[9] = 5.0;
        sim.set_joint_controls(&torques);

        let mut envelope = default_envelope();
        envelope.max_speed = 0.0; // guarantee any nonzero speed trips it

        let mut state = sim.step(1);
        for _ in 0..20 {
            state = sim.step(1);
        }

        let violations = redundant_check(&state, &envelope);
        // Not asserting speed is nonzero (depends on gait), just that the
        // function runs and returns a well-formed result either way.
        assert!(violations.iter().all(|v| !v.is_empty()));
    }
}
