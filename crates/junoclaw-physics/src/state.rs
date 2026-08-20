//! Physics state — the complete snapshot of a robot's physical state at
//! a single reflex cycle (typically 1ms intervals).
//!
//! This struct is what gets hashed to produce a cycle hash. The hash
//! is then included in a Merkle tree whose root is anchored on-chain.

use serde::{Deserialize, Serialize};

/// Complete physics state snapshot at a single reflex cycle.
///
/// All fields are in SI units (meters, radians, m/s, rad/s, Newtons, etc.)
/// unless otherwise noted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsState {
    /// Simulation timestamp (ms since batch start)
    pub timestamp_ms: u64,
    /// Joint states (position, velocity, torque for each actuated joint)
    pub joints: Vec<JointState>,
    /// IMU readings (accelerometer + gyroscope)
    pub imu: ImuReading,
    /// Contact events detected this cycle
    pub contacts: Vec<ContactInfo>,
    /// Derived sensor readings (speed, force, distance, tilt, acceleration)
    pub sensors: SensorReadings,
    /// Center of mass position (x, y, z) in world frame
    pub com_position: [f64; 3],
    /// Center of mass velocity (vx, vy, vz) in world frame
    pub com_velocity: [f64; 3],
    /// Orientation as quaternion (w, x, y, z)
    pub orientation: [f64; 4],
}

/// State of a single joint at a reflex cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointState {
    /// Joint name (e.g. "left_wheel", "shoulder_pitch")
    pub name: String,
    /// Position (radians for revolute, meters for prismatic)
    pub position: f64,
    /// Velocity (rad/s or m/s)
    pub velocity: f64,
    /// Applied torque/force (N·m or N)
    pub torque: f64,
}

/// IMU reading at a reflex cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImuReading {
    /// Linear acceleration (m/s²) in body frame [x, y, z]
    pub accel: [f64; 3],
    /// Angular velocity (rad/s) in body frame [roll, pitch, yaw]
    pub gyro: [f64; 3],
}

/// Contact event detected during a reflex cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContactInfo {
    /// Body part that made contact
    pub body_part: String,
    /// Contact normal force (N)
    pub normal_force: f64,
    /// Contact point in world frame [x, y, z]
    pub point: [f64; 3],
}

/// Derived sensor readings — the values checked against the safety envelope.
///
/// These are computed from the raw physics state (joints, IMU, contacts)
/// to produce the scalar values that the safety envelope checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorReadings {
    /// Linear speed (m/s) — magnitude of COM velocity
    pub speed: f64,
    /// Maximum contact force (N) across all contacts this cycle
    pub max_force: f64,
    /// Minimum distance to any obstacle (m)
    pub min_distance: f64,
    /// Tilt angle (degrees) — angle between up vector and body z-axis
    pub tilt_degrees: f64,
    /// Linear acceleration magnitude (m/s²)
    pub acceleration: f64,
}

impl PhysicsState {
    /// Serialize to canonical JSON for hashing.
    ///
    /// Uses sorted keys to ensure deterministic hashing across platforms.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        // Serialize with sorted keys for determinism
        let mut value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        sort_json_keys(&mut value);
        serde_json::to_vec(&value).unwrap_or_default()
    }

    /// Compute SHA-256 hash of this physics state.
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Recursively sort JSON object keys for deterministic serialization.
fn sort_json_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // serde_json::Map preserves insertion order with "preserve_order" feature,
            // but without it, keys are already sorted. To be safe, we sort explicitly.
            let mut sorted: Vec<(String, serde_json::Value)> =
                map.iter_mut().map(|(k, v)| {
                    sort_json_keys(v);
                    (k.clone(), v.clone())
                }).collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            map.clear();
            for (k, v) in sorted {
                map.insert(k, v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                sort_json_keys(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_state_hash_deterministic() {
        let state = PhysicsState {
            timestamp_ms: 100,
            joints: vec![JointState {
                name: "left_wheel".to_string(),
                position: 1.5,
                velocity: 0.8,
                torque: 2.3,
            }],
            imu: ImuReading {
                accel: [0.1, 0.2, 9.8],
                gyro: [0.0, 0.0, 0.1],
            },
            contacts: vec![],
            sensors: SensorReadings {
                speed: 0.8,
                max_force: 0.0,
                min_distance: 5.0,
                tilt_degrees: 2.1,
                acceleration: 0.3,
            },
            com_position: [0.0, 0.0, 0.5],
            com_velocity: [0.8, 0.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0],
        };

        let h1 = state.hash();
        let h2 = state.hash();
        assert_eq!(h1, h2, "hash should be deterministic");
        assert_eq!(h1.len(), 64, "SHA-256 hex should be 64 chars");
    }

    #[test]
    fn test_physics_state_hash_differs_on_change() {
        let mut state = PhysicsState {
            timestamp_ms: 100,
            joints: vec![JointState {
                name: "left_wheel".to_string(),
                position: 1.5,
                velocity: 0.8,
                torque: 2.3,
            }],
            imu: ImuReading {
                accel: [0.1, 0.2, 9.8],
                gyro: [0.0, 0.0, 0.1],
            },
            contacts: vec![],
            sensors: SensorReadings {
                speed: 0.8,
                max_force: 0.0,
                min_distance: 5.0,
                tilt_degrees: 2.1,
                acceleration: 0.3,
            },
            com_position: [0.0, 0.0, 0.5],
            com_velocity: [0.8, 0.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0],
        };

        let h1 = state.hash();

        // Change speed
        state.sensors.speed = 1.2;
        let h2 = state.hash();

        assert_ne!(h1, h2, "hash should change when state changes");
    }

    #[test]
    fn test_physics_state_serialization() {
        let state = PhysicsState {
            timestamp_ms: 0,
            joints: vec![],
            imu: ImuReading {
                accel: [0.0, 0.0, 9.81],
                gyro: [0.0; 3],
            },
            contacts: vec![ContactInfo {
                body_part: "foot".to_string(),
                normal_force: 45.0,
                point: [0.1, 0.2, 0.0],
            }],
            sensors: SensorReadings {
                speed: 0.0,
                max_force: 45.0,
                min_distance: 0.1,
                tilt_degrees: 0.0,
                acceleration: 0.0,
            },
            com_position: [0.0, 0.0, 0.8],
            com_velocity: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0],
        };

        let json = serde_json::to_string(&state).unwrap();
        let decoded: PhysicsState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.sensors.max_force, 45.0);
        assert_eq!(decoded.contacts.len(), 1);
        assert_eq!(decoded.contacts[0].body_part, "foot");
    }
}
