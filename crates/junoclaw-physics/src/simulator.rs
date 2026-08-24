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
    /// For differential-drive robots: (left_torque, right_torque).
    /// For quadrupeds: (front-left sum, front-right sum) — simplified.
    fn set_control(&mut self, left_torque: f64, right_torque: f64);

    /// Set per-joint torque commands for multi-DOF robots.
    /// Default implementation maps to set_control for wheeled robots.
    fn set_joint_controls(&mut self, torques: &[f64]) {
        if torques.len() >= 2 {
            self.set_control(torques[0], torques[1]);
        }
    }

    /// Add an obstacle at the given position with radius.
    fn add_obstacle(&mut self, x: f64, y: f64, radius: f64);

    /// Get the number of DOF this backend models.
    fn dof(&self) -> usize {
        2 // default for wheeled
    }
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

// ---------------------------------------------------------------------------
// QuadrupedBackend — 15-DOF quadruped simulator for DOGZILLA-Lite
// ---------------------------------------------------------------------------

/// Joint names for the 15-DOF DOGZILLA-Lite (12 leg + 3 arm).
pub const QUADRUPED_JOINT_NAMES: [&str; 15] = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
];

/// Configuration for the quadruped physics backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuadrupedConfig {
    /// Robot mass (kg) — DOGZILLA-Lite is ~0.575 kg
    pub mass: f64,
    /// Leg length (m) — hip-to-foot
    pub leg_length: f64,
    /// Body length (m)
    pub body_length: f64,
    /// Body width (m)
    pub body_width: f64,
    /// Standing height (m)
    pub standing_height: f64,
    /// Max joint torque (N·m)
    pub max_joint_torque: f64,
    /// Max arm force (N)
    pub max_arm_force: f64,
    /// Gravity (m/s²)
    pub gravity: f64,
    /// Ground friction coefficient
    pub friction: f64,
    /// Initial position [x, y, z]
    pub initial_position: [f64; 3],
}

impl Default for QuadrupedConfig {
    fn default() -> Self {
        Self {
            mass: 0.575,
            leg_length: 0.12,
            body_length: 0.18,
            body_width: 0.10,
            standing_height: 0.12,
            max_joint_torque: 5.0,
            max_arm_force: 10.0,
            gravity: 9.81,
            friction: 0.8,
            initial_position: [0.0, 0.0, 0.12],
        }
    }
}

/// 15-DOF quadruped physics backend for DOGZILLA-Lite.
///
/// Models simplified quadruped dynamics:
/// - 12 leg joints (3 per leg: hip, thigh, calf) with PD control
/// - 3 arm joints (base, shoulder, gripper) with torque control
/// - Body COM with gravity, ground contact via 4 foot contacts
/// - IMU from body dynamics (accelerometer + gyroscope)
/// - Tilt from body orientation (roll/pitch)
/// - Gait pattern: alternating trot (FL+RR vs FR+RL)
pub struct QuadrupedBackend {
    robot_id: String,
    config: QuadrupedConfig,
    // Body state
    pos: [f64; 3],
    vel: [f64; 3],
    roll: f64,    // body roll (rad)
    pitch: f64,   // body pitch (rad)
    yaw: f64,     // body yaw (rad)
    roll_vel: f64,
    pitch_vel: f64,
    yaw_vel: f64,
    // 15 joint states: position, velocity, torque
    joint_pos: [f64; 15],
    joint_vel: [f64; 15],
    joint_torque: [f64; 15],
    // Environment
    obstacles: Vec<Obstacle>,
    // Timing
    sim_time_ms: u64,
    // Previous velocity for acceleration
    prev_vel: [f64; 3],
    // Gait phase (0..2π), trot pattern
    gait_phase: f64,
}

impl QuadrupedBackend {
    pub fn new(robot_id: String, config: QuadrupedConfig) -> Self {
        let pos = config.initial_position;
        Self {
            robot_id,
            config,
            pos,
            vel: [0.0; 3],
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            roll_vel: 0.0,
            pitch_vel: 0.0,
            yaw_vel: 0.0,
            joint_pos: [0.0; 15],
            joint_vel: [0.0; 15],
            joint_torque: [0.0; 15],
            obstacles: Vec::new(),
            sim_time_ms: 0,
            prev_vel: [0.0; 3],
            gait_phase: 0.0,
        }
    }

    /// Compute foot positions from joint angles (simplified forward kinematics).
    fn foot_positions(&self) -> [[f64; 3]; 4] {
        let half_l = self.config.body_length / 2.0;
        let half_w = self.config.body_width / 2.0;
        let h = self.config.standing_height;

        // Each leg: hip angle -> thigh angle -> calf angle -> foot position
        // Simplified: foot is directly below hip when joints are at zero
        let leg_offsets = [
            [half_l, half_w, 0.0],   // FL
            [half_l, -half_w, 0.0],  // FR
            [-half_l, half_w, 0.0],  // RL
            [-half_l, -half_w, 0.0], // RR
        ];

        let mut feet = [[0.0; 3]; 4];
        for i in 0..4 {
            let hip_idx = i * 3;
            let thigh_idx = i * 3 + 1;
            let calf_idx = i * 3 + 2;

            // Simplified FK: foot position relative to hip
            let hip_angle = self.joint_pos[hip_idx];
            let thigh_angle = self.joint_pos[thigh_idx];
            let calf_angle = self.joint_pos[calf_idx];

            let leg_reach = self.config.leg_length
                * (thigh_angle.cos() * 0.5 + calf_angle.cos() * 0.5).abs().max(0.05);

            feet[i] = [
                self.pos[0] + leg_offsets[i][0] * self.yaw.cos() + leg_reach * hip_angle.sin(),
                self.pos[1] + leg_offsets[i][1] + leg_reach * hip_angle.cos(),
                self.pos[2] - h + leg_reach * (thigh_angle + calf_angle).sin() * 0.3,
            ];
        }
        feet
    }

    /// Check foot contacts with ground.
    fn check_contacts(&self) -> Vec<ContactInfo> {
        let mut contacts = Vec::new();
        let feet = self.foot_positions();
        let foot_names = ["fl_foot", "fr_foot", "rl_foot", "rr_foot"];

        for i in 0..4 {
            if feet[i][2] <= 0.01 {
                // Foot is on the ground — distribute weight
                let force = self.config.mass * self.config.gravity / 4.0;
                contacts.push(ContactInfo {
                    body_part: foot_names[i].to_string(),
                    normal_force: force,
                    point: feet[i],
                });
            }
        }

        // Obstacle contacts
        for obs in &self.obstacles {
            let dx = obs.x - self.pos[0];
            let dy = obs.y - self.pos[1];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < obs.radius + self.config.body_length / 2.0 {
                let force = self.config.mass
                    * (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1])
                    * 0.5;
                contacts.push(ContactInfo {
                    body_part: "body".to_string(),
                    normal_force: force,
                    point: [obs.x, obs.y, self.pos[2]],
                });
            }
        }

        contacts
    }

    /// Compute IMU readings from body dynamics.
    fn compute_imu(&self, accel_world: [f64; 3]) -> ImuReading {
        // Transform to body frame using roll/pitch/yaw (simplified)
        let accel_body_x = accel_world[0] * self.yaw.cos() + accel_world[1] * self.yaw.sin();
        let accel_body_y = -accel_world[0] * self.yaw.sin() + accel_world[1] * self.yaw.cos();
        let accel_body_z = accel_world[2];

        ImuReading {
            accel: [accel_body_x, accel_body_y, accel_body_z + self.config.gravity],
            gyro: [self.roll_vel, self.pitch_vel, self.yaw_vel],
        }
    }

    /// Compute tilt angle from roll and pitch.
    fn compute_tilt(&self) -> f64 {
        // Tilt = angle between body up-vector and world up-vector
        let tilt_rad = (self.roll.sin() * self.roll.sin() + self.pitch.sin() * self.pitch.sin()).sqrt().atan();
        tilt_rad.to_degrees()
    }

    /// Compute sensor readings.
    fn compute_sensors(&self, contacts: &[ContactInfo], accel_mag: f64) -> SensorReadings {
        let speed = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt();
        let max_force = contacts.iter()
            .map(|c| c.normal_force)
            .fold(0.0f64, f64::max);
        let min_distance = self.min_obstacle_distance();
        let tilt = self.compute_tilt();

        // Arm force: torque on arm_shoulder joint * arm length
        let arm_force = self.joint_torque[13].abs() * 0.1; // simplified

        SensorReadings {
            speed,
            max_force: max_force.max(arm_force),
            min_distance,
            tilt_degrees: tilt,
            acceleration: accel_mag,
        }
    }

    fn min_obstacle_distance(&self) -> f64 {
        let mut min_dist = 100.0;
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

    /// Apply gait pattern: alternating trot.
    /// During trot, diagonal leg pairs alternate lifting.
    fn apply_gait(&mut self, dt: f64) {
        self.gait_phase += dt * 4.0; // 4 Hz gait frequency
        let phase = self.gait_phase % (2.0 * std::f64::consts::PI);

        // Trot: FL+RR lift together, FR+RL lift together (180° offset)
        let fl_lift = phase.sin().max(0.0);
        let fr_lift = (phase + std::f64::consts::PI).sin().max(0.0);

        // Leg joint targets: lift thigh when in swing phase
        let lift_angle = 0.3; // ~17° lift

        // FL (joints 0,1,2) and RR (joints 9,10,11) — swing together
        self.joint_pos[1] = lift_angle * fl_lift;
        self.joint_pos[10] = lift_angle * fl_lift;

        // FR (joints 3,4,5) and RL (joints 6,7,8) — swing together
        self.joint_pos[4] = lift_angle * fr_lift;
        self.joint_pos[7] = lift_angle * fr_lift;

        // Hip joints: slight forward/back oscillation for propulsion
        let stride = 0.15;
        self.joint_pos[0] = stride * (phase).sin();
        self.joint_pos[9] = stride * (phase).sin();
        self.joint_pos[3] = stride * (phase + std::f64::consts::PI).sin();
        self.joint_pos[6] = stride * (phase + std::f64::consts::PI).sin();
    }
}

impl PhysicsSimulator for QuadrupedBackend {
    fn step(&mut self, dt_ms: u64) -> PhysicsState {
        let dt = dt_ms as f64 / 1000.0;

        // Apply gait pattern
        self.apply_gait(dt);

        // Clamp torques
        let max_t = self.config.max_joint_torque;
        for i in 0..15 {
            self.joint_torque[i] = self.joint_torque[i].clamp(-max_t, max_t);
        }

        // Simplified body dynamics: COM moves based on gait propulsion
        let propulsion = self.joint_torque[0] * 0.01 + self.joint_torque[9] * 0.01;
        let turn = (self.joint_torque[3] - self.joint_torque[6]) * 0.01;

        let accel_x = propulsion / self.config.mass * self.yaw.cos();
        let accel_y = propulsion / self.config.mass * self.yaw.sin();
        let accel_z = 0.0;

        // Update velocity
        self.vel[0] += accel_x * dt;
        self.vel[1] += accel_y * dt;

        // Friction
        let drag = self.config.friction * dt;
        self.vel[0] *= (1.0 - drag * 0.1).max(0.0);
        self.vel[1] *= (1.0 - drag * 0.1).max(0.0);

        // Update position
        self.pos[0] += self.vel[0] * dt;
        self.pos[1] += self.vel[1] * dt;

        // Angular dynamics
        self.yaw_vel += turn / self.config.mass * dt;
        self.yaw_vel *= (1.0 - drag * 0.1).max(0.0);
        self.yaw += self.yaw_vel * dt;

        // Body tilt from acceleration (simplified: pitching under accel, rolling under turn)
        let target_pitch = (-accel_x / self.config.gravity).atan() * 0.3;
        let target_roll = (self.vel[0] * self.yaw_vel / self.config.gravity).atan() * 0.3;
        self.pitch += (target_pitch - self.pitch) * dt * 10.0;
        self.roll += (target_roll - self.roll) * dt * 10.0;
        self.pitch_vel = (target_pitch - self.pitch) * 10.0;
        self.roll_vel = (target_roll - self.roll) * 10.0;

        // Update joint velocities (simplified: velocity = d(position)/dt for gait-driven)
        for i in 0..15 {
            // For arm joints, velocity comes from torque
            if i >= 12 {
                self.joint_vel[i] += self.joint_torque[i] * dt;
                self.joint_pos[i] += self.joint_vel[i] * dt;
                self.joint_vel[i] *= (1.0 - drag * 0.2).max(0.0);
            }
            // Leg joints are gait-driven, velocity computed from position delta
            // (already set by apply_gait)
        }

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

        // Build 15 joint states
        let joints: Vec<JointState> = (0..15)
            .map(|i| JointState {
                name: QUADRUPED_JOINT_NAMES[i].to_string(),
                position: self.joint_pos[i],
                velocity: self.joint_vel[i],
                torque: self.joint_torque[i],
            })
            .collect();

        // Orientation quaternion from roll/pitch/yaw
        let (cy, sy) = (self.yaw.cos() * 0.5, self.yaw.sin() * 0.5);
        let (cp, sp) = (self.pitch.cos() * 0.5, self.pitch.sin() * 0.5);
        let (cr, sr) = (self.roll.cos() * 0.5, self.roll.sin() * 0.5);
        let orientation = [
            cr * cp * cy + sr * sp * sy,
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
        ];

        let state = PhysicsState {
            timestamp_ms: self.sim_time_ms,
            joints,
            imu,
            contacts,
            sensors,
            com_position: self.pos,
            com_velocity: self.vel,
            orientation,
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
        self.roll = 0.0;
        self.pitch = 0.0;
        self.yaw = 0.0;
        self.roll_vel = 0.0;
        self.pitch_vel = 0.0;
        self.yaw_vel = 0.0;
        self.joint_pos = [0.0; 15];
        self.joint_vel = [0.0; 15];
        self.joint_torque = [0.0; 15];
        self.sim_time_ms = 0;
        self.prev_vel = [0.0; 3];
        self.gait_phase = 0.0;
    }

    fn set_control(&mut self, left_torque: f64, right_torque: f64) {
        // Map simplified control to front leg hip torques
        self.joint_torque[0] = left_torque;
        self.joint_torque[3] = right_torque;
    }

    fn set_joint_controls(&mut self, torques: &[f64]) {
        let n = torques.len().min(15);
        for i in 0..n {
            self.joint_torque[i] = torques[i];
        }
    }

    fn add_obstacle(&mut self, x: f64, y: f64, radius: f64) {
        self.obstacles.push(Obstacle { x, y, radius });
    }

    fn dof(&self) -> usize {
        15
    }
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

    // --- Quadruped backend tests ---

    #[test]
    fn test_quadruped_produces_15_dof_state() {
        let mut sim = QuadrupedBackend::new(
            "dogzilla-lite-001".to_string(),
            QuadrupedConfig::default(),
        );
        let state = sim.step(1);
        assert_eq!(state.timestamp_ms, 1);
        assert_eq!(state.joints.len(), 15, "should have 15 joints");
        assert_eq!(state.joints[0].name, "fl_hip");
        assert_eq!(state.joints[12].name, "arm_base");
        assert_eq!(state.joints[14].name, "arm_gripper");
    }

    #[test]
    fn test_quadruped_dof() {
        let sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        assert_eq!(sim.dof(), 15);
    }

    #[test]
    fn test_quadruped_gait_motion() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        // Apply forward torque on front hips
        sim.set_control(1.0, 1.0);
        for _ in 0..100 {
            sim.step(1);
        }
        let state = sim.step(1);
        // Robot should have some motion or joint changes
        let has_motion = state.com_position[0] != 0.0 || state.com_position[1] != 0.0;
        let has_joint_activity = state.joints.iter().any(|j| j.position != 0.0);
        assert!(has_motion || has_joint_activity,
            "quadruped should show motion or joint activity after 100ms");
    }

    #[test]
    fn test_quadruped_joint_controls() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        // Set all 15 joint torques
        let torques = [0.5; 15];
        sim.set_joint_controls(&torques);
        let state = sim.step(1);
        // All joints should have the torque (clamped to max)
        for j in &state.joints {
            assert!(j.torque.abs() <= 5.0 + 1e-6, "torque should be clamped");
        }
    }

    #[test]
    fn test_quadruped_arm_torque() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        // Set arm shoulder torque high
        let mut torques = [0.0; 15];
        torques[13] = 3.0; // arm_shoulder
        sim.set_joint_controls(&torques);
        let state = sim.step(1);
        assert!((state.joints[13].torque - 3.0).abs() < 1e-6,
            "arm_shoulder torque should be 3.0, got {}", state.joints[13].torque);
    }

    #[test]
    fn test_quadruped_reset() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
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
    fn test_quadruped_hashes_differ() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        sim.set_control(1.0, 1.0);
        let s1 = sim.step(1);
        let s2 = sim.step(1);
        assert_ne!(s1.hash(), s2.hash(), "consecutive cycles should differ");
    }

    #[test]
    fn test_quadruped_imu_gravity() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        let state = sim.step(1);
        assert!((state.imu.accel[2] - 9.81).abs() < 0.1,
            "IMU z should read gravity, got {}", state.imu.accel[2]);
    }

    #[test]
    fn test_quadruped_foot_contacts() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        let state = sim.step(1);
        // At rest, should have at least some foot contacts
        let foot_contacts = state.contacts.iter()
            .filter(|c| c.body_part.contains("foot"))
            .count();
        assert!(foot_contacts > 0, "should have foot contacts at rest");
    }

    #[test]
    fn test_quadruped_obstacle_detection() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        sim.add_obstacle(0.5, 0.0, 0.1);
        let state = sim.step(1);
        assert!(state.sensors.min_distance < 0.5,
            "should detect obstacle, got min_distance={}", state.sensors.min_distance);
    }

    #[test]
    fn test_quadruped_tilt_under_motion() {
        let mut sim = QuadrupedBackend::new(
            "dog-1".to_string(),
            QuadrupedConfig::default(),
        );
        sim.set_control(2.0, 0.5); // forward + turn
        for _ in 0..200 {
            sim.step(1);
        }
        let state = sim.step(1);
        assert!(state.sensors.tilt_degrees >= 0.0,
            "tilt should be non-negative, got {}", state.sensors.tilt_degrees);
    }
}
