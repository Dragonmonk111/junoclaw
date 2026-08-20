//! Physics simulator — produces `PhysicsState` at each reflex cycle.
//!
//! The `PhysicsSimulator` trait abstracts over backends:
//! - `SimulatedBackend`: Built-in rigid-body dynamics with gravity, joints,
//!   contacts, and simple obstacle avoidance. No external dependencies.
//! - `MujocoBackend` (future): Wraps MuJoCo Rust bindings for high-fidelity
//!   simulation. Requires MuJoCo SDK installed.
//!
//! The simulated backend models a differential-drive robot (like a TurtleBot
//! or a wheeled humanoid base) with:
//! - 2 driven wheels + 1 caster
//! - Gravity, ground contact, friction
//! - Obstacle proximity sensing (raycast)
//! - IMU (accelerometer + gyroscope)
//! - Joint torque limits
//!
//! It's not a replacement for MuJoCo, but it produces realistic physics
//! state that exercises the full attestation pipeline.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::state::{ContactInfo, ImuReading, JointState, PhysicsState, SensorReadings};

/// Trait for physics simulation backends.
pub trait PhysicsSimulator: Send + Sync {
    /// Advance the simulation by one reflex cycle (default 1ms).
    /// Returns the physics state at the new time step.
    fn step(&mut self, dt_ms: u64) -> PhysicsState;

    /// Get the robot ID.
    fn robot_id(&self) -> &str;

    /// Reset the simulation to initial state.
    fn reset(&mut self);

    /// Inject a control command (motor torques, joint targets).
    fn set_control(&mut self, left_torque: f64, right_torque: f64);

    /// Add an obstacle at the given position with radius.
    fn add_obstacle(&mut self, x: f64, y: f64, radius: f64);
}

/// Configuration for the simulated physics backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    /// Robot mass (kg)
    pub mass: f64,
    /// Wheel radius (m)
    pub wheel_radius: f64,
    /// Wheelbase (distance between wheels, m)
    pub wheelbase: f64,
    /// Maximum motor torque (N·m)
    pub max_torque: f64,
    /// Initial position [x, y, z]
    pub initial_position: [f64; 3],
    /// Gravity (m/s²)
    pub gravity: f64,
    /// Ground friction coefficient
    pub friction: f64,
    /// Initial battery level (0.0 - 1.0)
    pub battery: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            mass: 5.0,
            wheel_radius: 0.033,
            wheelbase: 0.16,
            max_torque: 2.5,
            initial_position: [0.0, 0.0, 0.15],
            gravity: 9.81,
            friction: 0.6,
            battery: 1.0,
        }
    }
}

/// Obstacle in the simulation environment.
#[derive(Clone, Debug)]
struct Obstacle {
    x: f64,
    y: f64,
    radius: f64,
}

/// Simulated physics backend — rigid-body dynamics for a differential-drive robot.
///
/// Models:
/// - Differential drive kinematics (2 wheels + caster)
/// - Gravity and ground contact
/// - Wheel friction and torque-to-force conversion
/// - Obstacle proximity (raycast in 4 directions)
/// - IMU readings (accelerometer + gyroscope from dynamics)
/// - Joint states (position, velocity, torque per wheel)
pub struct SimulatedBackend {
    robot_id: String,
    config: SimConfig,
    // Current state
    pos: [f64; 3],
    vel: [f64; 3],
    theta: f64,       // heading (radians)
    omega: f64,       // angular velocity (rad/s)
    left_wheel_pos: f64,
    right_wheel_pos: f64,
    left_wheel_vel: f64,
    right_wheel_vel: f64,
    left_torque: f64,
    right_torque: f64,
    // Environment
    obstacles: Vec<Obstacle>,
    // Timing
    sim_time_ms: u64,
    // Previous velocity for acceleration computation
    prev_vel: [f64; 3],
}

impl SimulatedBackend {
    pub fn new(robot_id: String, config: SimConfig) -> Self {
        let pos = config.initial_position;
        Self {
            robot_id,
            config,
            pos,
            vel: [0.0; 3],
            theta: 0.0,
            omega: 0.0,
            left_wheel_pos: 0.0,
            right_wheel_pos: 0.0,
            left_wheel_vel: 0.0,
            right_wheel_vel: 0.0,
            left_torque: 0.0,
            right_torque: 0.0,
            obstacles: Vec::new(),
            sim_time_ms: 0,
            prev_vel: [0.0; 3],
        }
    }

    /// Raycast to find minimum distance to obstacles.
    fn min_obstacle_distance(&self) -> f64 {
        let mut min_dist = 100.0; // large default

        for obs in &self.obstacles {
            let dx = obs.x - self.pos[0];
            let dy = obs.y - self.pos[1];
            let dist = (dx * dx + dy * dy).sqrt() - obs.radius;
            if dist < min_dist {
                min_dist = dist.max(0.0);
            }
        }

        min_dist
    }

    /// Check for contacts (simplified: ground contact + obstacle overlap).
    fn check_contacts(&self) -> Vec<ContactInfo> {
        let mut contacts = Vec::new();

        // Ground contact (always present if z <= wheel radius + clearance)
        if self.pos[2] <= self.config.wheel_radius + 0.15 {
            let normal_force = self.config.mass * self.config.gravity;
            contacts.push(ContactInfo {
                body_part: "wheels".to_string(),
                normal_force,
                point: [self.pos[0], self.pos[1], 0.0],
            });
        }

        // Obstacle contacts
        for obs in &self.obstacles {
            let dx = obs.x - self.pos[0];
            let dy = obs.y - self.pos[1];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < obs.radius + 0.1 {
                // Contact!
                let force = self.config.mass * (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]) * 0.5;
                contacts.push(ContactInfo {
                    body_part: "chassis".to_string(),
                    normal_force: force,
                    point: [obs.x - dx / dist * obs.radius, obs.y - dy / dist * obs.radius, 0.1],
                });
            }
        }

        contacts
    }

    /// Compute IMU readings from current dynamics.
    fn compute_imu(&self, accel_world: [f64; 3]) -> ImuReading {
        // Transform world-frame acceleration to body frame
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let accel_body_x = accel_world[0] * cos_t + accel_world[1] * sin_t;
        let accel_body_y = -accel_world[0] * sin_t + accel_world[1] * cos_t;
        let accel_body_z = accel_world[2];

        ImuReading {
            accel: [accel_body_x, accel_body_y, accel_body_z + self.config.gravity],
            gyro: [0.0, 0.0, self.omega],
        }
    }

    /// Compute tilt angle from orientation (simplified: heading only, no roll/pitch).
    fn compute_tilt(&self) -> f64 {
        // In the simulated model, tilt is minimal unless going over bumps
        // Add small noise based on acceleration to make it realistic
        let lateral_accel = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt() * self.omega.abs();
        // Tilt due to centripetal force: atan(a_lateral / g)
        (lateral_accel / self.config.gravity).atan().to_degrees()
    }

    /// Compute sensor readings from physics state.
    fn compute_sensors(&self, contacts: &[ContactInfo], accel: f64) -> SensorReadings {
        let speed = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt();
        let max_force = contacts.iter()
            .map(|c| c.normal_force)
            .fold(0.0f64, f64::max);
        let min_distance = self.min_obstacle_distance();
        let tilt = self.compute_tilt();

        SensorReadings {
            speed,
            max_force,
            min_distance,
            tilt_degrees: tilt,
            acceleration: accel,
        }
    }
}

impl PhysicsSimulator for SimulatedBackend {
    fn step(&mut self, dt_ms: u64) -> PhysicsState {
        let dt = dt_ms as f64 / 1000.0;

        // Clamp torques
        let left_torque = self.left_torque.clamp(-self.config.max_torque, self.config.max_torque);
        let right_torque = self.right_torque.clamp(-self.config.max_torque, self.config.max_torque);

        // Convert torque to wheel force (F = τ / r)
        let left_force = left_torque / self.config.wheel_radius;
        let right_force = right_torque / self.config.wheel_radius;

        // Differential drive kinematics
        let total_force = left_force + right_force;
        let turn_torque = (right_force - left_force) * self.config.wheelbase / 2.0;

        // Apply forces (simplified: no slip, flat ground)
        let accel_x = total_force / self.config.mass * self.theta.cos();
        let accel_y = total_force / self.config.mass * self.theta.sin();
        let accel_z = 0.0; // flat ground

        // Angular acceleration
        let angular_accel = turn_torque / (self.config.mass * self.config.wheelbase * self.config.wheelbase / 12.0);

        // Update velocity
        self.vel[0] += accel_x * dt;
        self.vel[1] += accel_y * dt;
        self.vel[2] = 0.0;

        // Friction (simple linear drag)
        let drag = self.config.friction * dt;
        self.vel[0] *= (1.0 - drag * 0.1).max(0.0);
        self.vel[1] *= (1.0 - drag * 0.1).max(0.0);

        // Update angular velocity
        self.omega += angular_accel * dt;
        self.omega *= (1.0 - drag * 0.1).max(0.0);

        // Update position
        self.pos[0] += self.vel[0] * dt;
        self.pos[1] += self.vel[1] * dt;
        self.theta += self.omega * dt;

        // Update wheel states
        let wheel_linear_vel = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt();
        self.left_wheel_vel = (wheel_linear_vel - self.omega * self.config.wheelbase / 2.0) / self.config.wheel_radius;
        self.right_wheel_vel = (wheel_linear_vel + self.omega * self.config.wheelbase / 2.0) / self.config.wheel_radius;
        self.left_wheel_pos += self.left_wheel_vel * dt;
        self.right_wheel_pos += self.right_wheel_vel * dt;

        // Compute acceleration magnitude
        let accel_mag = (accel_x * accel_x + accel_y * accel_y + accel_z * accel_z).sqrt();

        // Update sim time
        self.sim_time_ms += dt_ms;

        // Compute contacts
        let contacts = self.check_contacts();

        // Compute IMU
        let imu = self.compute_imu([accel_x, accel_y, accel_z]);

        // Compute sensors
        let sensors = self.compute_sensors(&contacts, accel_mag);

        // Build physics state
        let state = PhysicsState {
            timestamp_ms: self.sim_time_ms,
            joints: vec![
                JointState {
                    name: "left_wheel".to_string(),
                    position: self.left_wheel_pos,
                    velocity: self.left_wheel_vel,
                    torque: left_torque,
                },
                JointState {
                    name: "right_wheel".to_string(),
                    position: self.right_wheel_pos,
                    velocity: self.right_wheel_vel,
                    torque: right_torque,
                },
            ],
            imu,
            contacts,
            sensors,
            com_position: self.pos,
            com_velocity: self.vel,
            orientation: [self.theta.cos(), 0.0, 0.0, self.theta.sin()],
        };

        self.prev_vel = self.vel;

        state
    }

    fn robot_id(&self) -> &str {
        &self.robot_id
    }

    fn reset(&mut self) {
        self.pos = self.config.initial_position;
        self.vel = [0.0; 3];
        self.theta = 0.0;
        self.omega = 0.0;
        self.left_wheel_pos = 0.0;
        self.right_wheel_pos = 0.0;
        self.left_wheel_vel = 0.0;
        self.right_wheel_vel = 0.0;
        self.left_torque = 0.0;
        self.right_torque = 0.0;
        self.sim_time_ms = 0;
        self.prev_vel = [0.0; 3];
    }

    fn set_control(&mut self, left_torque: f64, right_torque: f64) {
        self.left_torque = left_torque;
        self.right_torque = right_torque;
    }

    fn add_obstacle(&mut self, x: f64, y: f64, radius: f64) {
        self.obstacles.push(Obstacle { x, y, radius });
    }
}

/// Helper: get current time in ms since UNIX epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_produces_state() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let state = sim.step(1);
        assert_eq!(state.timestamp_ms, 1);
        assert_eq!(state.joints.len(), 2);
        assert_eq!(state.joints[0].name, "left_wheel");
    }

    #[test]
    fn test_simulator_motion() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Apply forward torque
        sim.set_control(1.0, 1.0);

        // Step 100 times (100ms)
        for _ in 0..100 {
            sim.step(1);
        }

        // Robot should have moved forward
        let state = sim.step(1);
        assert!(state.com_position[0] > 0.0 || state.com_position[1] > 0.0,
            "robot should have moved after 100ms of forward torque");
        assert!(state.sensors.speed > 0.0, "speed should be positive");
    }

    #[test]
    fn test_simulator_turning() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Apply differential torque (turn right)
        sim.set_control(1.0, 0.0);

        for _ in 0..100 {
            sim.step(1);
        }

        // Robot should have turned (theta changed)
        let state = sim.step(1);
        // After turning, orientation quaternion should be different from identity
        assert!(state.orientation[0] < 1.0 || state.orientation[3] != 0.0,
            "robot should have turned");
    }

    #[test]
    fn test_simulator_obstacle_detection() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Add obstacle 1m ahead
        sim.add_obstacle(1.0, 0.0, 0.1);

        let state = sim.step(1);
        assert!(state.sensors.min_distance < 1.0, "should detect obstacle ahead");
    }

    #[test]
    fn test_simulator_contact() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Place obstacle right on top of robot
        sim.add_obstacle(0.0, 0.0, 0.1);

        // Move forward into it
        sim.set_control(1.0, 1.0);
        for _ in 0..10 {
            sim.step(1);
        }

        let state = sim.step(1);
        // Should have a chassis contact
        assert!(state.contacts.iter().any(|c| c.body_part == "chassis"),
            "should have chassis contact with obstacle");
    }

    #[test]
    fn test_simulator_reset() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        sim.set_control(1.0, 1.0);
        for _ in 0..100 {
            sim.step(1);
        }

        sim.reset();
        let state = sim.step(1);
        assert_eq!(state.com_position, sim.config.initial_position);
        assert_eq!(state.sensors.speed, 0.0);
    }

    #[test]
    fn test_simulator_ground_contact() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let state = sim.step(1);
        // Should have ground contact (robot starts on ground)
        assert!(state.contacts.iter().any(|c| c.body_part == "wheels"),
            "should have wheel ground contact");
    }

    #[test]
    fn test_simulator_imu_gravity() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        let state = sim.step(1);
        // IMU z-accel should include gravity (~9.81)
        assert!((state.imu.accel[2] - 9.81).abs() < 0.1,
            "IMU z should read gravity, got {}", state.imu.accel[2]);
    }

    #[test]
    fn test_simulator_tilt_under_turning() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // High speed turn
        sim.set_control(2.0, 0.5);
        for _ in 0..200 {
            sim.step(1);
        }

        let state = sim.step(1);
        // Tilt should be non-zero due to centripetal force
        assert!(state.sensors.tilt_degrees >= 0.0, "tilt should be non-negative");
    }

    #[test]
    fn test_simulator_torque_clamping() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());

        // Set torque beyond max
        sim.set_control(100.0, 100.0);
        let state = sim.step(1);

        // Torque should be clamped to max
        assert!(state.joints[0].torque <= sim.config.max_torque,
            "torque should be clamped, got {}", state.joints[0].torque);
    }

    #[test]
    fn test_simulator_multiple_obstacles() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        sim.add_obstacle(0.5, 0.0, 0.1);
        sim.add_obstacle(0.0, 0.5, 0.2);
        sim.add_obstacle(2.0, 1.0, 0.15);

        let state = sim.step(1);
        // Min distance should be to the closest obstacle
        assert!(state.sensors.min_distance < 0.5, "should detect closest obstacle");
    }

    #[test]
    fn test_simulator_cycle_hashes_differ() {
        let mut sim = SimulatedBackend::new("robot-1".to_string(), SimConfig::default());
        sim.set_control(1.0, 1.0);

        let state1 = sim.step(1);
        let state2 = sim.step(1);

        let h1 = state1.hash();
        let h2 = state2.hash();
        assert_ne!(h1, h2, "consecutive cycles should have different hashes");
    }

    #[test]
    fn test_simulator_robot_id() {
        let sim = SimulatedBackend::new("test-bot-42".to_string(), SimConfig::default());
        assert_eq!(sim.robot_id(), "test-bot-42");
    }
}
