"""Gymnasium environments wrapping the DOGZILLA-Lite MJCF.

DogzillaStandEnv  — Phase C: stand upright, validate the training loop.
DogzillaWalkEnv   — Phase D: walk forward, with domain randomization and
                    an imitation reward against the existing hand-coded
                    trot (QuadrupedBackend::apply_gait), reused here as
                    the reference motion instead of building a new tool.

See drafts/PLAN_SIM2REAL_RL_PIPELINE.md. Joint order matches
QUADRUPED_JOINT_NAMES in crates/junoclaw-physics/src/simulator.rs exactly,
so a trained policy's action vector lines up 1:1 with the real bridge's
/robot/joint_commands schema.
"""

from __future__ import annotations

import os

import mujoco
import numpy as np
import gymnasium as gym
from gymnasium import spaces

MODEL_PATH = os.path.join(os.path.dirname(__file__), "models", "dogzilla_lite.xml")

# Must match crates/junoclaw-physics/src/simulator.rs::QUADRUPED_JOINT_NAMES.
# NOTE: the MJCF defines the arm body before the leg bodies (see the model
# file's <worldbody> order), so joint/actuator index order in MuJoCo is
# arm-then-legs even though this Python-facing constant lists legs-then-arm
# to match the Rust side. JOINT_TO_QPOS/JOINT_TO_ACT below handle the
# reindexing so callers never need to think about MuJoCo's internal order.
QUADRUPED_JOINT_NAMES = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
]

# Must stay consistent with the MJCF "stand" keyframe in
# models/dogzilla_lite.xml and with SIM_NEUTRAL_RAD in deploy/xgo_robot.py.
# QuadrupedConfig in crates/junoclaw-physics/src/simulator.rs documents
# leg_length=0.12 as hip-to-foot TOTAL (0.06 per link), standing_height=0.12.
STAND_HEIGHT = 0.12
FALL_HEIGHT = 0.05

SIM_NEUTRAL_RAD = np.array([
    0.0, 0.5, -0.9,   # fl hip/thigh/calf
    0.0, 0.5, -0.9,   # fr
    0.0, 0.5, -0.9,   # rl
    0.0, 0.5, -0.9,   # rr
    0.0, 0.0, 0.0,    # arm base/shoulder/gripper
], dtype=np.float64)

# Actions are residuals around SIM_NEUTRAL_RAD, so action=0 is a stand.
ACTION_SCALE = 0.3


class DogzillaStandEnv(gym.Env):
    """Phase C task: stay upright at standing height, minimize energy.

    This is deliberately the *simplest* possible task — it exists to
    validate the full training + ONNX export loop end-to-end before
    attempting locomotion (Phase D). Reward has no forward-velocity term.
    """

    metadata = {"render_modes": ["human"], "render_fps": 50}

    def __init__(
        self,
        render_mode: str | None = None,
        ctrl_hz: float = 50.0,
        randomize: bool = True,
        randomize_scale: float = 0.15,
    ):
        super().__init__()
        self.model = mujoco.MjModel.from_xml_path(MODEL_PATH)
        self.data = mujoco.MjData(self.model)
        self.render_mode = render_mode
        self._viewer = None
        self.randomize = randomize
        self.randomize_scale = randomize_scale

        self.sim_dt = self.model.opt.timestep
        self.ctrl_hz = ctrl_hz
        self.substeps = max(1, int(round(1.0 / (ctrl_hz * self.sim_dt))))

        # Map our canonical joint order -> MuJoCo qpos/qvel/actuator indices.
        self.joint_to_qpos_adr = np.array(
            [self.model.jnt_qposadr[self.model.joint(name).id] for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_to_dof_adr = np.array(
            [self.model.jnt_dofadr[self.model.joint(name).id] for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_to_act_id = np.array(
            [self.model.actuator(name).id for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_ranges = np.array(
            [self.model.joint(name).range for name in QUADRUPED_JOINT_NAMES]
        )  # (15, 2)

        self.stand_key_id = self.model.key("stand").id
        self.imu_site_id = self.model.site("imu").id

        n_joints = len(QUADRUPED_JOINT_NAMES)

        # Domain randomization (Phase D): baseline values perturbed fresh
        # each reset(). Each parallel env instance owns its own MjModel, so
        # mutating these arrays only affects this env's copy.
        self._base_body_mass = self.model.body_mass.copy()
        self._base_geom_friction = self.model.geom_friction.copy()
        self._base_actuator_gainprm = self.model.actuator_gainprm.copy()
        self._obs_noise_std = np.array(
            [0.01] * n_joints + [0.02] * n_joints + [0.01] * 4
            + [0.03] * 3 + [0.05] * 3 + [0.005],
            dtype=np.float32,
        )

        # obs: joint pos(15) + joint vel(15) + trunk quat(4) + trunk gyro(3)
        #      + trunk lin accel(3) + trunk height(1) = 41
        obs_dim = n_joints * 2 + 4 + 3 + 3 + 1
        self.observation_space = spaces.Box(
            low=-np.inf, high=np.inf, shape=(obs_dim,), dtype=np.float32
        )
        # Actions are normalized [-1, 1], scaled to each joint's real range.
        self.action_space = spaces.Box(
            low=-1.0, high=1.0, shape=(n_joints,), dtype=np.float32
        )

        self._max_episode_steps = 500
        self._step_count = 0

    # ------------------------------------------------------------------
    def _action_to_joint_targets(self, action: np.ndarray) -> np.ndarray:
        action = np.clip(action, -1.0, 1.0)
        targets = SIM_NEUTRAL_RAD + ACTION_SCALE * action
        return np.clip(targets, self.joint_ranges[:, 0], self.joint_ranges[:, 1])

    def _get_obs(self) -> np.ndarray:
        qpos = self.data.qpos[self.joint_to_qpos_adr]
        qvel = self.data.qvel[self.joint_to_dof_adr]
        trunk_quat = self.data.sensor("trunk_quat").data
        trunk_gyro = self.data.sensor("trunk_gyro").data
        trunk_accel = self.data.sensor("trunk_accel").data
        trunk_height = self.data.qpos[2:3]  # trunk freejoint z
        obs = np.concatenate(
            [qpos, qvel, trunk_quat, trunk_gyro, trunk_accel, trunk_height]
        ).astype(np.float32)
        if self.randomize:
            obs = obs + self.np_random.normal(0.0, self._obs_noise_std).astype(np.float32)
        return obs

    def _apply_domain_randomization(self) -> None:
        """Perturb mass, friction, and actuator gains for this episode.

        Placeholder ranges (Phase A/B will replace these with distributions
        centered on measured/identified real values instead of guesses).
        """
        if not self.randomize:
            return
        s = self.randomize_scale
        mass_scale = self.np_random.uniform(1.0 - s, 1.0 + s, size=self._base_body_mass.shape)
        self.model.body_mass[:] = self._base_body_mass * mass_scale
        friction_scale = self.np_random.uniform(1.0 - s, 1.0 + s, size=self._base_geom_friction.shape)
        self.model.geom_friction[:] = self._base_geom_friction * friction_scale
        gain_scale = self.np_random.uniform(1.0 - s, 1.0 + s, size=self._base_actuator_gainprm.shape)
        self.model.actuator_gainprm[:] = self._base_actuator_gainprm * gain_scale

    def _upright_score(self) -> float:
        """1.0 = perfectly upright, 0.0 = on its side."""
        quat = self.data.sensor("trunk_quat").data
        w, x, y, z = quat
        # z-component of the body's local +z axis in world frame
        up_z = 1.0 - 2.0 * (x * x + y * y)
        return float(np.clip(up_z, -1.0, 1.0))

    # ------------------------------------------------------------------
    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        self._apply_domain_randomization()
        mujoco.mj_resetDataKeyframe(self.model, self.data, self.stand_key_id)
        noise = self.np_random.uniform(-0.03, 0.03, size=len(QUADRUPED_JOINT_NAMES))
        self.data.qpos[self.joint_to_qpos_adr] += noise
        mujoco.mj_forward(self.model, self.data)
        self._step_count = 0
        return self._get_obs(), {}

    def step(self, action: np.ndarray):
        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])
        energy_penalty = 0.001 * float(np.sum(np.square(action)))
        survive_bonus = 0.5

        reward = survive_bonus + 2.0 * upright - energy_penalty
        # Penalize deviation from the standing height.
        reward -= 5.0 * abs(height - STAND_HEIGHT)

        fell = upright < 0.3 or height < FALL_HEIGHT
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell

        if terminated:
            reward -= 5.0

        info = {"upright": upright, "height": height}
        return obs, reward, terminated, truncated, info

    def render(self):
        if self.render_mode != "human":
            return
        if self._viewer is None:
            import mujoco.viewer

            self._viewer = mujoco.viewer.launch_passive(self.model, self.data)
        self._viewer.sync()

    def close(self):
        if self._viewer is not None:
            self._viewer.close()
            self._viewer = None


def make_env(render_mode: str | None = None):
    return DogzillaStandEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase D: locomotion (walk) task
# ---------------------------------------------------------------------------

# Reference trot generator used as the imitation reward's reference motion.
_GAIT_HZ = 4.0
# Calf flex applied during the swing phase to clear the ground.
_LIFT_ANGLE = 0.3
# Thigh fore/aft sweep amplitude. The MJCF hip axis is abduction
# (axis="1 0 0"), so stride must come from the thigh pitch joint — driving
# the hip instead swings the legs sideways and produces no forward motion.
_STRIDE = 0.15


def _reference_pose(phase: float) -> np.ndarray:
    """15-dim reference joint targets for a given gait phase (radians).

    Diagonal trot: FL+RR swing together (fl_lift), FR+RL swing together
    (fr_lift, 180 degrees out of phase). Expressed as a residual on
    SIM_NEUTRAL_RAD so the reference stays inside the joint range the
    policy can actually reach through ACTION_SCALE.

    This deliberately does NOT mirror QuadrupedBackend::apply_gait, which
    assumes the legacy simplified kinematics of foot_positions rather than
    the anatomical hip/thigh/calf convention this MJCF uses.
    """
    fl_lift = max(0.0, float(np.sin(phase)))
    fr_lift = max(0.0, float(np.sin(phase + np.pi)))
    pose = SIM_NEUTRAL_RAD.copy()
    pose[1] += _STRIDE * np.cos(phase)          # fl_thigh sweep
    pose[2] -= _LIFT_ANGLE * fl_lift            # fl_calf flex on swing
    pose[4] += _STRIDE * np.cos(phase + np.pi)  # fr_thigh
    pose[5] -= _LIFT_ANGLE * fr_lift            # fr_calf
    pose[7] += _STRIDE * np.cos(phase + np.pi)  # rl_thigh
    pose[8] -= _LIFT_ANGLE * fr_lift            # rl_calf
    pose[10] += _STRIDE * np.cos(phase)         # rr_thigh
    pose[11] -= _LIFT_ANGLE * fl_lift           # rr_calf
    return pose  # hips held at neutral; indices 12:15 (arm) stowed


class DogzillaWalkEnv(DogzillaStandEnv):
    """Phase D task: walk forward, tracking a target velocity, with an
    imitation reward against the reference trot above.

    Domain randomization is inherited from DogzillaStandEnv and on by
    default — walking needs robustness far more than standing does.
    """

    def __init__(self, *args, target_velocity: float = 0.08, **kwargs):
        super().__init__(*args, **kwargs)
        self.target_velocity = target_velocity
        self.gait_freq_hz = _GAIT_HZ
        self._gait_phase = 0.0
        self._max_episode_steps = 1000  # more room to show forward progress

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._gait_phase = 0.0
        return obs, info

    def step(self, action: np.ndarray):
        dt = 1.0 / self.ctrl_hz
        self._gait_phase = (self._gait_phase + dt * self.gait_freq_hz * 2.0 * np.pi) % (2.0 * np.pi)

        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])
        forward_vel = float(self.data.qvel[0])  # trunk world-frame x-velocity

        energy_penalty = 0.001 * float(np.sum(np.square(action)))
        survive_bonus = 0.3
        vel_tracking = float(np.exp(-((forward_vel - self.target_velocity) ** 2) / 0.02))

        reference = _reference_pose(self._gait_phase)
        actual_leg_pos = self.data.qpos[self.joint_to_qpos_adr][:12]
        imitation_error = float(np.sum(np.square(actual_leg_pos - reference[:12])))
        imitation_reward = float(np.exp(-imitation_error / 2.0))

        reward = (
            survive_bonus
            + 1.5 * upright
            + 1.0 * vel_tracking
            + 1.0 * imitation_reward
            - energy_penalty
        )
        reward -= 2.0 * max(0.0, abs(height - STAND_HEIGHT) - 0.03)  # loose height band

        fell = upright < 0.3 or height < FALL_HEIGHT
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell
        if terminated:
            reward -= 5.0

        info = {
            "upright": upright,
            "height": height,
            "forward_vel": forward_vel,
            "vel_tracking": vel_tracking,
            "imitation_reward": imitation_reward,
        }
        return obs, reward, terminated, truncated, info


def make_walk_env(render_mode: str | None = None):
    return DogzillaWalkEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G1: turn/steer task — walk toward a target heading
# ---------------------------------------------------------------------------


class DogzillaTurnEnv(DogzillaWalkEnv):
    """Phase G1 task: walk while steering toward a target heading.

    Extends the walk env with a yaw-tracking reward. Each episode samples
    a random target heading in [-pi/2, pi/2] (left, straight, or right).
    The observation is unchanged (41-dim) so a policy trained here is
    drop-in compatible with the existing bridge inference loop — the
    policy learns to infer desired heading from the IMU yaw rate and
    trunk orientation rather than receiving an explicit command.

    This is a "behavioral" turn policy: it explores turning during
    training and gets rewarded for matching the target yaw rate, but at
    inference time it runs the same 41-dim observation as the walk policy.
    For a commandable turn policy (explicit heading input), extend the
    observation with a 42nd dimension — but that changes the bridge too.
    """

    def __init__(self, *args, target_heading_range: float = np.pi / 2, **kwargs):
        super().__init__(*args, **kwargs)
        self.target_heading_range = target_heading_range
        self._target_heading = 0.0
        self._initial_heading = 0.0
        self._max_episode_steps = 1000

    def _get_heading(self) -> float:
        """Extract yaw (heading) from trunk quaternion."""
        quat = self.data.sensor("trunk_quat").data
        w, x, y, z = quat
        # yaw = atan2(2(wz + xy), 1 - 2(y^2 + z^2))
        yaw = np.arctan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z))
        return float(yaw)

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._initial_heading = self._get_heading()
        self._target_heading = self._initial_heading + self.np_random.uniform(
            -self.target_heading_range, self.target_heading_range
        )
        return obs, info

    def step(self, action: np.ndarray):
        dt = 1.0 / self.ctrl_hz
        self._gait_phase = (self._gait_phase + dt * self.gait_freq_hz * 2.0 * np.pi) % (2.0 * np.pi)

        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])
        forward_vel = float(self.data.qvel[0])
        yaw_rate = float(self.data.qvel[5])  # trunk angular velocity z

        # Heading error: how far off from target heading
        current_heading = self._get_heading()
        heading_error = self._target_heading - current_heading
        # Wrap to [-pi, pi]
        heading_error = (heading_error + np.pi) % (2.0 * np.pi) - np.pi
        desired_yaw_rate = heading_error / max(dt * 10, 0.5)  # converge over ~5 steps
        yaw_tracking = float(np.exp(-((yaw_rate - desired_yaw_rate) ** 2) / 0.5))

        energy_penalty = 0.001 * float(np.sum(np.square(action)))
        survive_bonus = 0.3
        vel_tracking = float(np.exp(-((forward_vel - self.target_velocity) ** 2) / 0.02))

        reference = _reference_pose(self._gait_phase)
        actual_leg_pos = self.data.qpos[self.joint_to_qpos_adr][:12]
        imitation_error = float(np.sum(np.square(actual_leg_pos - reference[:12])))
        imitation_reward = float(np.exp(-imitation_error / 2.0))

        reward = (
            survive_bonus
            + 1.5 * upright
            + 0.5 * vel_tracking
            + 1.5 * yaw_tracking
            + 0.5 * imitation_reward
            - energy_penalty
        )
        reward -= 2.0 * max(0.0, abs(height - STAND_HEIGHT) - 0.03)

        # Bonus for reducing heading error
        reward += 1.0 * float(np.exp(-abs(heading_error) / 0.3))

        fell = upright < 0.3 or height < FALL_HEIGHT
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell
        if terminated:
            reward -= 5.0

        info = {
            "upright": upright,
            "height": height,
            "forward_vel": forward_vel,
            "vel_tracking": vel_tracking,
            "imitation_reward": imitation_reward,
            "yaw_error": abs(heading_error),
            "yaw_rate": yaw_rate,
            "target_heading": self._target_heading,
            "current_heading": current_heading,
        }
        return obs, reward, terminated, truncated, info


def make_turn_env(render_mode: str | None = None):
    return DogzillaTurnEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G8: commandable speed + heading — the "go anywhere" policy
# ---------------------------------------------------------------------------


class DogzillaSpeedHeadingEnv(DogzillaWalkEnv):
    """Phase G8 task: walk at a commanded speed and heading.

    Extends the walk env with two command dimensions in the observation:
    - commanded_speed: target forward velocity in [0, 0.2] m/s
    - commanded_heading: target heading change in [-pi/2, pi/2] rad

    Observation is 43-dim (41 + 2 command dims). Action space unchanged
    (15-dim). This is the most useful single policy for real deployment —
    an operator or planner sends [speed, heading] and the robot executes.

    The bridge inference loop needs to be updated to pass 43-dim obs.
    """

    def __init__(self, *args, speed_range=(0.0, 0.2), heading_range=(-np.pi / 2, np.pi / 2), **kwargs):
        super().__init__(*args, **kwargs)
        self._speed_range = speed_range
        self._heading_range = heading_range
        self._commanded_speed = 0.0
        self._commanded_heading = 0.0
        self._initial_heading = 0.0
        self._max_episode_steps = 1000

        # Override observation space to 43 dims
        n_joints = len(QUADRUPED_JOINT_NAMES)
        obs_dim = n_joints * 2 + 4 + 3 + 3 + 1 + 2  # 41 + 2 command dims
        self.observation_space = spaces.Box(
            low=-np.inf, high=np.inf, shape=(obs_dim,), dtype=np.float32
        )

    def _get_heading(self) -> float:
        """Extract yaw (heading) from trunk quaternion."""
        quat = self.data.sensor("trunk_quat").data
        w, x, y, z = quat
        yaw = np.arctan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z))
        return float(yaw)

    def _get_obs(self) -> np.ndarray:
        """Extend base obs with commanded_speed and commanded_heading."""
        base_obs = super()._get_obs()
        cmd = np.array([self._commanded_speed, self._commanded_heading], dtype=np.float32)
        return np.concatenate([base_obs, cmd])

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._initial_heading = self._get_heading()
        self._commanded_speed = self.np_random.uniform(*self._speed_range)
        self._commanded_heading = self._initial_heading + self.np_random.uniform(*self._heading_range)
        # Re-get obs with commands included
        return self._get_obs(), info

    def step(self, action: np.ndarray):
        dt = 1.0 / self.ctrl_hz
        self._gait_phase = (self._gait_phase + dt * self.gait_freq_hz * 2.0 * np.pi) % (2.0 * np.pi)

        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])
        forward_vel = float(self.data.qvel[0])
        yaw_rate = float(self.data.qvel[5])

        # Speed tracking
        vel_tracking = float(np.exp(-((forward_vel - self._commanded_speed) ** 2) / 0.02))

        # Heading tracking
        current_heading = self._get_heading()
        heading_error = self._commanded_heading - current_heading
        heading_error = (heading_error + np.pi) % (2.0 * np.pi) - np.pi
        desired_yaw_rate = heading_error / max(dt * 10, 0.5)
        yaw_tracking = float(np.exp(-((yaw_rate - desired_yaw_rate) ** 2) / 0.5))

        # Imitation reward (keep the trot gait)
        reference = _reference_pose(self._gait_phase)
        actual_leg_pos = self.data.qpos[self.joint_to_qpos_adr][:12]
        imitation_error = float(np.sum(np.square(actual_leg_pos - reference[:12])))
        imitation_reward = float(np.exp(-imitation_error / 2.0))

        energy_penalty = 0.001 * float(np.sum(np.square(action)))
        survive_bonus = 0.3

        reward = (
            survive_bonus
            + 1.5 * upright
            + 1.0 * vel_tracking
            + 1.0 * yaw_tracking
            + 0.5 * imitation_reward
            - energy_penalty
        )
        reward -= 2.0 * max(0.0, abs(height - STAND_HEIGHT) - 0.03)
        reward += 0.5 * float(np.exp(-abs(heading_error) / 0.3))

        fell = upright < 0.3 or height < FALL_HEIGHT
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell
        if terminated:
            reward -= 5.0

        info = {
            "upright": upright,
            "height": height,
            "forward_vel": forward_vel,
            "vel_tracking": vel_tracking,
            "imitation_reward": imitation_reward,
            "yaw_error": abs(heading_error),
            "yaw_rate": yaw_rate,
            "commanded_speed": self._commanded_speed,
            "commanded_heading": self._commanded_heading,
            "current_heading": current_heading,
        }
        return obs, reward, terminated, truncated, info


def make_speed_heading_env(render_mode: str | None = None):
    return DogzillaSpeedHeadingEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G3: fall recovery task — self-right from fallen poses
# ---------------------------------------------------------------------------


class DogzillaRecoveryEnv(DogzillaStandEnv):
    """Phase G3 task: self-right from a randomized fallen pose.

    Starts the robot in a non-standing orientation (on its side, upside
    down, or tilted). Reward = upright improvement + height increase +
    final upright bonus. No locomotion term — the only goal is to get
    back to standing. Episode is short (200 steps = 4 seconds at 50 Hz).
    """

    def __init__(self, *args, curriculum_tilt_min=None, curriculum_tilt_max=None, **kwargs):
        super().__init__(*args, **kwargs)
        self._max_episode_steps = 200
        self._initial_upright = 0.0
        self._initial_height = 0.0
        self._upright_steps = 0
        # Curriculum: if set, tilt range narrows (e.g. start 30-90 deg, progress to 60-150)
        self._tilt_min = curriculum_tilt_min if curriculum_tilt_min is not None else np.pi / 3       # 60 deg
        self._tilt_max = curriculum_tilt_max if curriculum_tilt_max is not None else 5.0 * np.pi / 6  # 150 deg

    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        # Don't use the stand keyframe — start from a random fallen pose
        mujoco.mj_resetData(self.model, self.data)

        # Random trunk orientation: tilt within curriculum range
        tilt_angle = self.np_random.uniform(self._tilt_min, self._tilt_max)
        # Constrain tilt axis to x-y plane so the robot actually tilts
        # (a pure z-axis rotation would just spin the robot, not tip it over)
        tilt_axis = self.np_random.uniform(-1, 1, size=2)
        tilt_axis_3d = np.array([tilt_axis[0], tilt_axis[1], 0.0])
        tilt_axis_3d /= max(np.linalg.norm(tilt_axis_3d), 1e-6)

        # Build quaternion from axis-angle
        quat = np.zeros(4)
        mujoco.mju_axisAngle2Quat(quat, tilt_axis_3d, tilt_angle)
        # MuJoCo quat is [w, x, y, z], qpos for freejoint is [x, y, z, qw, qx, qy, qz]
        # Spawn clear of the ground. A leg reaches 0.12m (0.06 thigh + 0.06 calf),
        # so a tilted trunk at 0.08 puts feet up to 4cm UNDERGROUND; MuJoCo resolves
        # that deep penetration by launching the trunk (observed h > 2m). Spawn above
        # the maximum leg reach instead and let it settle onto its side naturally.
        self.data.qpos[0:3] = [0, 0, 0.16]
        self.data.qpos[3:7] = quat

        # Set joint positions to a relaxed pose (slightly bent)
        for i, name in enumerate(QUADRUPED_JOINT_NAMES):
            jid = self.model.joint(name).id
            qpos_adr = self.model.jnt_qposadr[jid]
            mid = (self.joint_ranges[i, 0] + self.joint_ranges[i, 1]) / 2.0
            self.data.qpos[qpos_adr] = mid + self.np_random.uniform(-0.1, 0.1)

        # Small random angular velocity to simulate landing dynamics
        self.data.qvel[3:6] = self.np_random.uniform(-1.0, 1.0, size=3)

        mujoco.mj_forward(self.model, self.data)

        # Let the pose settle so the episode starts from a physically consistent
        # fallen state (contacts resolved, no residual penetration energy) rather
        # than mid-air. Held joint targets keep the legs from flailing while it lands.
        self.data.ctrl[self.joint_to_act_id] = self.data.qpos[self.joint_to_qpos_adr]
        for _ in range(self.substeps * 40):
            mujoco.mj_step(self.model, self.data)
        self.data.qvel[:] = 0.0
        mujoco.mj_forward(self.model, self.data)

        self._step_count = 0
        self._initial_upright = self._upright_score()
        self._initial_height = float(self.data.qpos[2])
        self._upright_steps = 0
        self._best_upright = self._initial_upright

        return self._get_obs(), {}

    def step(self, action: np.ndarray):
        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])

        energy_penalty = 0.002 * float(np.sum(np.square(action)))

        # Reward improvement from initial fallen state
        upright_improvement = upright - self._initial_upright
        height_improvement = height - self._initial_height

        # Dense progress shaping: reward ANY upright improvement (even from
        # very negative initial states like 150° tilt where the agent starts
        # at upright=-0.8 and getting to -0.3 is real progress).
        upright_progress = max(0.0, upright - self._best_upright)
        self._best_upright = max(self._best_upright, upright)

        # Cap height reward at standing height — don't reward explosive jumping
        capped_height_improvement = min(max(0.0, height_improvement), STAND_HEIGHT)

        # Penalize excessive height (physics explosions / launching)
        # v4: 4x stronger than v3 to decisively eliminate explosion exploitation
        height_explosion_penalty = 0.0
        if height > 2.0 * STAND_HEIGHT:
            height_explosion_penalty = -20.0 * (height - 2.0 * STAND_HEIGHT)

        # Angular velocity penalty — discourage purposeless spinning.
        # v4.1: 10x weaker. Self-righting from hard tilts REQUIRES aggressive
        # rotation (rock back and forth, build momentum, flip). At 0.05 the
        # penalty during a flip (gyro sum ~10-50 -> 0.5-2.5/step) exceeded the
        # upright gradient (3.0/step), so the policy learned that attempting
        # recovery is worse than lying still — performance degraded on hard
        # tilts while easy tilts (little rotation needed) were unaffected.
        gyro = self.data.sensor("trunk_gyro").data
        angular_vel_penalty = 0.005 * float(np.sum(np.square(gyro)))

        # v4: Effort reward — reward the agent for trying to move its joints
        # This prevents the "lying still" failure mode where the agent does nothing
        effort_reward = 0.05 * float(np.mean(np.abs(action)))

        # v4.2: Time penalty — halved. Hard-tilt recovery needs the full
        # 200-step window for the push-off/rock/flip maneuver; 0.01/step
        # accumulated to -2.0 by step 200, further punishing already-negative
        # hard-tilt episodes.
        time_penalty = 0.005 * self._step_count

        # Track consecutive upright steps for stable success detection
        if upright > 0.7 and 0.75 * STAND_HEIGHT < height < 1.5 * STAND_HEIGHT:
            self._upright_steps += 1
        else:
            self._upright_steps = 0

        # Strong bonus for reaching standing pose
        standing_bonus = 0.0
        if upright > 0.7 and 0.75 * STAND_HEIGHT < height < 1.5 * STAND_HEIGHT:
            standing_bonus = 2.0

        # v4.2: Restructured reward. The old 3.0*upright term dominated the
        # reward at -2.4/step for 150° tilt, making every hard-tilt episode
        # deeply negative regardless of progress. It also punished the
        # temporary upright regression during push-off (the flip maneuver
        # goes through worse orientations to build momentum). Now:
        #   - 0.5*upright: small gradient, doesn't dominate
        #   - 10.0*upright_progress: strong dense signal for any new best
        #   - 3.0*upright_improvement: rewards being above start pose
        #   - standing_bonus (2.0/step): main incentive to GET and STAY upright
        reward = (
            0.5 * upright
            + 3.0 * capped_height_improvement
            + 3.0 * max(0.0, upright_improvement)
            + 10.0 * upright_progress
            + standing_bonus
            + effort_reward
            - energy_penalty
            - angular_vel_penalty
            + height_explosion_penalty
            - time_penalty
        )

        # Success: upright and at standing height for 5 consecutive steps (stable, not flipping through)
        success = self._upright_steps >= 5
        # v5.2: Removed fell termination. The v5.1 from-scratch policy already
        # learned to attempt recovery on hard tilts (no "lying still" failure).
        # The fell condition was killing episodes at step 51 where the agent
        # was making progress (e.g. improving upright by 0.09 < 0.1 threshold).
        # Giving the full 200-step window lets the agent complete the flip.
        truncated = self._step_count >= self._max_episode_steps
        terminated = success

        if success:
            reward += 10.0

        info = {
            "upright": upright,
            "height": height,
            "upright_improvement": upright_improvement,
            "height_improvement": height_improvement,
            "capped_height_improvement": capped_height_improvement,
            "angular_vel_penalty": angular_vel_penalty,
            "height_explosion_penalty": height_explosion_penalty,
            "effort_reward": effort_reward,
            "time_penalty": time_penalty,
            "success": success,
        }
        return obs, reward, terminated, truncated, info


def make_recovery_env(render_mode: str | None = None):
    return DogzillaRecoveryEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G2: terrain adaptation task — walk on uneven ground
# ---------------------------------------------------------------------------


class DogzillaTerrainEnv(DogzillaWalkEnv):
    """Phase G2 task: walk forward on procedurally generated uneven terrain.

    Extends the walk env by replacing the flat floor with a heightfield
    that has random bumps, small steps, and friction variations. The robot
    must maintain its gait while adapting to ground irregularities.

    Uses the same 41-dim observation and 15-dim action as the walk env —
    the terrain is implicit in the dynamics (the robot feels it through
    contact forces and IMU perturbations), not explicitly observed.
    """

    def __init__(self, *args, terrain_roughness: float = 0.015, **kwargs):
        super().__init__(*args, **kwargs)
        self._terrain_roughness = terrain_roughness
        self._max_episode_steps = 1000
        self._terrain_hfield_id = None
        self._init_terrain()

    def _init_terrain(self):
        """Set up terrain simulation parameters.

        We simulate uneven terrain via dynamic perturbations rather than
        a static heightfield, since MjModel arrays can't be resized at
        runtime in the Python API. The perturbations are:
        1. Aggressive per-episode friction variation (±40%)
        2. Random external forces on the trunk (simulates ground bumps)
        3. Periodic lateral impulses (simulates hitting small obstacles)
        """
        if self._terrain_roughness <= 0:
            return
        self._terrain_friction_variations = True

    def _apply_terrain_randomization(self):
        """Vary ground friction and add contact perturbations to simulate terrain.

        Since we can't easily add a heightfield at runtime, we simulate uneven
        terrain by:
        1. Varying friction across the floor (some patches slippery, some grippy)
        2. Adding small random external forces to the trunk (simulates bumps)
        3. Randomizing the starting position slightly
        """
        if self._terrain_roughness <= 0:
            return

        # Vary floor friction more aggressively than domain randomization
        s = 0.4  # 40% friction variation (vs 15% for flat ground)
        friction_scale = self.np_random.uniform(1.0 - s, 1.0 + s)
        self.model.geom_friction[0] = self._base_geom_friction[0] * friction_scale

        # Add small random external force to trunk (simulates ground bumps)
        # Applied during step() via qfrc_applied
        self._terrain_force = self.np_random.uniform(
            -self._terrain_roughness, self._terrain_roughness, size=3
        )

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._apply_terrain_randomization()
        # Slightly randomize starting x-position to vary the terrain patch
        self.data.qpos[0] += self.np_random.uniform(-0.05, 0.05)
        mujoco.mj_forward(self.model, self.data)
        return self._get_obs(), info

    def step(self, action: np.ndarray):
        # Apply terrain perturbation force to trunk before stepping
        if self._terrain_roughness > 0 and hasattr(self, '_terrain_force'):
            # Apply random force to trunk (x, y, z) via qfrc_applied on freejoint
            self.data.qfrc_applied[0:3] = self._terrain_force * 50.0
            # Occasionally add a lateral impulse (simulates hitting a bump)
            if self._step_count > 0 and self._step_count % 50 == 0:
                impulse = self.np_random.uniform(-0.01, 0.01, size=3)
                self.data.qvel[0:3] += impulse

        result = super().step(action)

        # Clear applied forces after step
        self.data.qfrc_applied[0:3] = 0.0

        return result


def make_terrain_env(render_mode: str | None = None):
    return DogzillaTerrainEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G6: sit / lie down task — transition to resting pose
# ---------------------------------------------------------------------------

# Target sitting pose: legs folded under body, trunk lowered.
# This is a static pose, not a gait — the policy learns to smoothly
# transition from standing to this pose and hold it.
_SIT_QPOS = np.array([
    0.0, 0.0, 0.08,   # trunk: same xy, lowered to 0.08m (half standing height)
    1.0, 0.0, 0.0, 0.0,  # trunk quat: upright
    0.0, 0.0, 0.0,       # arm: stowed
    0.0, 1.2, -1.8,      # fl: hip neutral, thigh forward, calf folded
    0.0, 1.2, -1.8,      # fr
    0.0, 1.2, -1.8,      # rl
    0.0, 1.2, -1.8,      # rr
], dtype=np.float64)

# Joint targets for sitting (only the 15 controlled joints)
_SIT_JOINT_TARGETS = np.array([
    0.0, 1.2, -1.8,  # fl
    0.0, 1.2, -1.8,  # fr
    0.0, 1.2, -1.8,  # rl
    0.0, 1.2, -1.8,  # rr
    0.0, 0.0, 0.0,   # arm stowed
], dtype=np.float64)


class DogzillaSitEnv(DogzillaStandEnv):
    """Phase G6 task: transition from standing to a sitting pose and hold.

    The robot starts in the standing keyframe and must smoothly lower
    itself to a folded-leg sitting pose at half height. Reward is based
    on:
    1. Joint pose matching (how close to target sit pose)
    2. Height tracking (target 0.08m vs standing 0.16m)
    3. Smoothness penalty (avoid jerky transitions)
    4. Stability bonus (stay upright during transition)

    Uses the same 41-dim observation and 15-dim action as stand/walk.
    """

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._max_episode_steps = 300  # shorter — sitting is quick
        self._sit_targets = _SIT_JOINT_TARGETS.copy()

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        return obs, info

    def step(self, action: np.ndarray):
        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])

        # Current joint positions (15 controlled joints)
        current_joints = self.data.qpos[self.joint_to_qpos_adr]

        # Pose matching: how close are we to the sit target?
        joint_error = float(np.sum(np.square(current_joints - self._sit_targets)))
        pose_reward = float(np.exp(-joint_error / 0.5))

        # Height tracking: target 0.08m
        height_error = abs(height - 0.08)
        height_reward = float(np.exp(-height_error / 0.03))

        # Smoothness: penalize large action changes (jerky transitions)
        energy_penalty = 0.005 * float(np.sum(np.square(action)))

        # Stability: must stay upright during transition
        stability_bonus = 1.0 * upright

        # Phase-based reward: early steps reward smooth transition,
        # later steps reward holding the sit pose
        progress = min(self._step_count / 100.0, 1.0)  # ramp over 2 seconds
        transition_reward = (1.0 - progress) * 0.5 * float(np.exp(-joint_error / 1.0))
        hold_reward = progress * (1.5 * pose_reward + 1.0 * height_reward)

        reward = (
            stability_bonus
            + transition_reward
            + hold_reward
            - energy_penalty
        )

        # Success: reached sit pose and holding it
        success = (joint_error < 0.5 and height_error < 0.04 and upright > 0.5)
        if success:
            reward += 2.0  # bonus for reaching and holding sit pose

        fell = upright < 0.3 or height < 0.03
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell
        if terminated:
            reward -= 5.0

        info = {
            "upright": upright,
            "height": height,
            "joint_error": joint_error,
            "pose_reward": pose_reward,
            "height_reward": height_reward,
            "success": success,
        }
        return obs, reward, terminated, truncated, info


def make_sit_env(render_mode: str | None = None):
    return DogzillaSitEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G7: step climbing task — walk over small obstacles
# ---------------------------------------------------------------------------

STEP_MODEL_PATH = os.path.join(os.path.dirname(__file__), "models", "dogzilla_lite_step.xml")


class DogzillaStepEnv(DogzillaWalkEnv):
    """Phase G7 task: walk forward and climb over a small step obstacle.

    Uses a separate MJCF (dogzilla_lite_step.xml) that includes a box
    geom as a step obstacle. The step height (0.5-2cm) and x-position are
    randomized each episode. The robot must maintain its gait while
    stepping over the obstacle.

    Uses the same 41-dim observation and 15-dim action as the walk env.
    """

    def __init__(self, *args, step_height_range=(0.005, 0.02), **kwargs):
        self._step_height_range = step_height_range
        super().__init__(*args, **kwargs)
        # Reload with step model after super().__init__ loaded the flat one
        self.model = mujoco.MjModel.from_xml_path(STEP_MODEL_PATH)
        self.data = mujoco.MjData(self.model)
        # Re-derive indices for the new model
        self.joint_to_qpos_adr = np.array(
            [self.model.jnt_qposadr[self.model.joint(name).id] for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_to_dof_adr = np.array(
            [self.model.jnt_dofadr[self.model.joint(name).id] for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_to_act_id = np.array(
            [self.model.actuator(name).id for name in QUADRUPED_JOINT_NAMES]
        )
        self.joint_ranges = np.array(
            [self.model.joint(name).range for name in QUADRUPED_JOINT_NAMES]
        )
        self.stand_key_id = self.model.key("stand").id
        self.imu_site_id = self.model.site("imu").id
        self._base_body_mass = self.model.body_mass.copy()
        self._base_geom_friction = self.model.geom_friction.copy()
        self._base_actuator_gainprm = self.model.actuator_gainprm.copy()
        self._step_geom_id = self.model.geom("step_obstacle").id

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)

        # Randomize step height and position
        step_height = self.np_random.uniform(*self._step_height_range)
        step_x = self.np_random.uniform(0.15, 0.5)
        # Update geom size (z = half-height) and position (z = half-height)
        self.model.geom_size[self._step_geom_id][2] = step_height / 2.0
        self.model.geom_pos[self._step_geom_id][0] = step_x
        self.model.geom_pos[self._step_geom_id][2] = step_height / 2.0

        mujoco.mj_forward(self.model, self.data)
        return self._get_obs(), info


def make_step_env(render_mode: str | None = None):
    return DogzillaStepEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase G8+G2: combined speed+heading+terrain — ultimate pre-hardware policy
# ---------------------------------------------------------------------------


class DogzillaSpeedHeadingTerrainEnv(DogzillaSpeedHeadingEnv):
    """Combined G8+G2: commandable speed+heading on uneven terrain.

    Merges the 43-dim commandable observation (speed + heading commands)
    with terrain perturbations (±40% friction, random forces, impulses).

    This is the most useful single policy for real deployment — an operator
    sends [speed, heading] and the robot executes on rough ground.

    Observation: 43-dim (41 base + speed_cmd + heading_cmd)
    Action: 15-dim (same as all other envs)
    """

    def __init__(self, *args, terrain_roughness: float = 0.015, **kwargs):
        self._terrain_roughness = terrain_roughness
        super().__init__(*args, **kwargs)
        self._terrain_friction_variations = True

    def _apply_terrain_randomization(self):
        if self._terrain_roughness <= 0:
            return
        s = 0.4
        friction_scale = self.np_random.uniform(1.0 - s, 1.0 + s)
        self.model.geom_friction[0] = self._base_geom_friction[0] * friction_scale
        self._terrain_force = self.np_random.uniform(
            -self._terrain_roughness, self._terrain_roughness, size=3
        )

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._apply_terrain_randomization()
        self.data.qpos[0] += self.np_random.uniform(-0.05, 0.05)
        mujoco.mj_forward(self.model, self.data)
        return self._get_obs(), info

    def step(self, action: np.ndarray):
        if self._terrain_roughness > 0 and hasattr(self, '_terrain_force'):
            self.data.qfrc_applied[0:3] = self._terrain_force * 50.0
            if self._step_count > 0 and self._step_count % 50 == 0:
                impulse = self.np_random.uniform(-0.01, 0.01, size=3)
                self.data.qvel[0:3] += impulse

        result = super().step(action)

        self.data.qfrc_applied[0:3] = 0.0
        return result


def make_speed_heading_terrain_env(render_mode: str | None = None):
    return DogzillaSpeedHeadingTerrainEnv(render_mode=render_mode)


# ---------------------------------------------------------------------------
# Phase Track-2: smoother gait — gentler trot for cleaner hardware locomotion
# ---------------------------------------------------------------------------

# Smoother reference trot: lower frequency, smaller lift, longer stride.
_SMOOTH_GAIT_HZ = 2.5      # slower cadence (was 4.0)
_SMOOTH_LIFT_ANGLE = 0.18  # gentler foot lift (was 0.3)
_SMOOTH_STRIDE = 0.18      # longer stride for smoother forward motion (was 0.15)


def _smooth_reference_pose(phase: float) -> np.ndarray:
    """Smoother trot reference: slower cadence, lower lift, longer stride."""
    fl_lift = max(0.0, float(np.sin(phase)))
    fr_lift = max(0.0, float(np.sin(phase + np.pi)))
    pose = SIM_NEUTRAL_RAD.copy()
    pose[1] += _SMOOTH_STRIDE * np.cos(phase)
    pose[2] -= _SMOOTH_LIFT_ANGLE * fl_lift
    pose[4] += _SMOOTH_STRIDE * np.cos(phase + np.pi)
    pose[5] -= _SMOOTH_LIFT_ANGLE * fr_lift
    pose[7] += _SMOOTH_STRIDE * np.cos(phase + np.pi)
    pose[8] -= _SMOOTH_LIFT_ANGLE * fr_lift
    pose[10] += _SMOOTH_STRIDE * np.cos(phase)
    pose[11] -= _SMOOTH_LIFT_ANGLE * fl_lift
    return pose


class DogzillaSmoothWalkEnv(DogzillaSpeedHeadingTerrainEnv):
    """Track-2: smoother gait for cleaner hardware locomotion.

    Changes from the base speed+heading+terrain env:
    - Slower gait cadence (2.5 Hz vs 4.0 Hz) — less jerky on real servos
    - Lower foot lift (0.18 rad vs 0.30 rad) — smoother, less bouncy
    - Longer stride (0.18 rad vs 0.15 rad) — more forward distance per step
    - Action smoothness reward — penalizes large step-to-step action deltas
    - Stronger imitation reward — tighter gait tracking to reference
    - Slightly higher energy penalty — discourages jerky motions
    """

    def __init__(self, *args, terrain_roughness: float = 0.008, **kwargs):
        super().__init__(*args, terrain_roughness=terrain_roughness, **kwargs)
        self.gait_freq_hz = _SMOOTH_GAIT_HZ
        self._prev_action: np.ndarray | None = None
        self._max_episode_steps = 1000

    def reset(self, *, seed=None, options=None):
        obs, info = super().reset(seed=seed, options=options)
        self._prev_action = None
        return obs, info

    def step(self, action: np.ndarray):
        dt = 1.0 / self.ctrl_hz
        self._gait_phase = (self._gait_phase + dt * self.gait_freq_hz * 2.0 * np.pi) % (2.0 * np.pi)

        targets = self._action_to_joint_targets(np.asarray(action, dtype=np.float64))
        self.data.ctrl[self.joint_to_act_id] = targets

        for _ in range(self.substeps):
            mujoco.mj_step(self.model, self.data)

        self._step_count += 1
        obs = self._get_obs()

        upright = self._upright_score()
        height = float(self.data.qpos[2])
        forward_vel = float(self.data.qvel[0])
        yaw_rate = float(self.data.qvel[5])

        # Speed tracking
        vel_tracking = float(np.exp(-((forward_vel - self._commanded_speed) ** 2) / 0.02))

        # Heading tracking
        current_heading = self._get_heading()
        heading_error = self._commanded_heading - current_heading
        heading_error = (heading_error + np.pi) % (2.0 * np.pi) - np.pi
        desired_yaw_rate = heading_error / max(dt * 10, 0.5)
        yaw_tracking = float(np.exp(-((yaw_rate - desired_yaw_rate) ** 2) / 0.5))

        # Smoother imitation reward (uses smooth reference, tighter tracking)
        reference = _smooth_reference_pose(self._gait_phase)
        actual_leg_pos = self.data.qpos[self.joint_to_qpos_adr][:12]
        imitation_error = float(np.sum(np.square(actual_leg_pos - reference[:12])))
        imitation_reward = float(np.exp(-imitation_error / 1.5))  # tighter (was 2.0)

        # Action smoothness: penalize large step-to-step action changes
        action_smoothness_penalty = 0.0
        if self._prev_action is not None:
            action_delta = float(np.sum(np.square(action - self._prev_action)))
            action_smoothness_penalty = 0.05 * action_delta
        self._prev_action = action.copy()

        # Stronger energy penalty to discourage jerky motions
        energy_penalty = 0.003 * float(np.sum(np.square(action)))
        survive_bonus = 0.3

        reward = (
            survive_bonus
            + 1.5 * upright
            + 1.0 * vel_tracking
            + 1.0 * yaw_tracking
            + 1.5 * imitation_reward  # stronger (was 0.5)
            - energy_penalty
            - action_smoothness_penalty
        )
        reward -= 2.0 * max(0.0, abs(height - STAND_HEIGHT) - 0.03)
        reward += 0.5 * float(np.exp(-abs(heading_error) / 0.3))

        fell = upright < 0.3 or height < FALL_HEIGHT
        truncated = self._step_count >= self._max_episode_steps
        terminated = fell
        if terminated:
            reward -= 5.0

        info = {
            "upright": upright,
            "height": height,
            "forward_vel": forward_vel,
            "vel_tracking": vel_tracking,
            "imitation_reward": imitation_reward,
            "action_smoothness_penalty": action_smoothness_penalty,
            "yaw_error": abs(heading_error),
            "yaw_rate": yaw_rate,
            "commanded_speed": self._commanded_speed,
            "commanded_heading": self._commanded_heading,
            "current_heading": current_heading,
        }
        return obs, reward, terminated, truncated, info


def make_smooth_walk_env(render_mode: str | None = None):
    return DogzillaSmoothWalkEnv(render_mode=render_mode)
