use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// A single robot cycle — the atomic unit of memory.
/// One cycle = one perception → decision → action → outcome at L0 (1ms).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique ID (timestamp_nanos + robot_id hash)
    pub id: String,
    /// Robot identifier
    pub robot_id: String,
    /// Block height when this cycle was recorded
    pub block_height: u64,
    /// Timestamp in milliseconds since UNIX epoch
    pub timestamp_ms: u64,
    /// Cycle state — what the robot perceived
    pub state: CycleState,
    /// Action taken
    pub action: CycleAction,
    /// Outcome observed
    pub outcome: CycleOutcome,
    /// Optional ZK proof hash (if safety-gated)
    pub proof_hash: Option<String>,
    /// Optional parent cycle ID (for chaining)
    pub parent_id: Option<String>,
}

impl MemoryEntry {
    pub fn new(
        robot_id: &str,
        state: &str,
        action: &str,
        outcome: &str,
    ) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let id = format!("{}-{}", timestamp_ms, robot_id);
        Self {
            id,
            robot_id: robot_id.to_string(),
            block_height: 0,
            timestamp_ms,
            state: CycleState::from_str(state),
            action: CycleAction::from_str(action),
            outcome: CycleOutcome::from_str(outcome),
            proof_hash: None,
            parent_id: None,
        }
    }

    /// Serialize to bytes for Merkle hashing.
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// SHA-256 hash of the entry.
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.to_bytes());
        hasher.finalize().into()
    }
}

/// Robot's perceived state at cycle start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleState {
    /// Raw description (e.g., "standing", "walking", "fallen")
    pub description: String,
    /// Sensor snapshot (joint angles, IMU, etc.)
    pub sensors: SensorSnapshot,
    /// Environment context (terrain, obstacles, etc.)
    pub environment: Option<String>,
}

impl CycleState {
    pub fn from_str(s: &str) -> Self {
        Self {
            description: s.to_string(),
            sensors: SensorSnapshot::default(),
            environment: None,
        }
    }
}

/// Action the robot decided to take.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleAction {
    /// Action description (e.g., "walk-forward", "turn-left", "halt")
    pub command: String,
    /// Motor commands (joint torques or velocities)
    pub motor_commands: Vec<f32>,
    /// Duration of the action in milliseconds
    pub duration_ms: u32,
}

impl CycleAction {
    pub fn from_str(s: &str) -> Self {
        Self {
            command: s.to_string(),
            motor_commands: Vec::new(),
            duration_ms: 1000,
        }
    }
}

/// Outcome observed after the action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleOutcome {
    /// Outcome description (e.g., "moved 0.3m", "fell", "stayed still")
    pub description: String,
    /// Whether the outcome was safe (verified by ZK proof)
    pub safe: bool,
    /// Error code if the action failed
    pub error: Option<String>,
}

impl CycleOutcome {
    pub fn from_str(s: &str) -> Self {
        Self {
            description: s.to_string(),
            safe: true,
            error: None,
        }
    }
}

/// Sensor snapshot at a single cycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SensorSnapshot {
    /// Joint angles in radians (one per joint)
    pub joint_angles: Vec<f32>,
    /// IMU reading: [accel_x, accel_y, accel_z, gyro_x, gyro_y, gyro_z]
    pub imu: [f32; 6],
    /// Battery voltage
    pub battery_voltage: f32,
    /// CPU temperature in Celsius
    pub cpu_temp_c: f32,
}
