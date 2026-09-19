"""Real XGO robot interface for DOGZILLA-Lite.

Runs on the Raspberry Pi inside the robot, using xgolib. Converts sim
observation/action conventions to/from the XGO servo/IMU API.
"""

from __future__ import annotations

import json
import math
import os
import time
from typing import Any

import numpy as np

try:
    from xgolib import XGO
except ImportError:
    XGO = None  # type: ignore


# Neutral sim pose (radians) matching the MJCF "stand" keyframe exactly
# (models/dogzilla_lite.xml): hip=0.0, thigh=+0.5, calf=-0.9 per leg.
SIM_NEUTRAL_RAD = np.array([
    0.0, 0.5, -0.9,  # fl hip/thigh/calf
    0.0, 0.5, -0.9,  # fr hip/thigh/calf
    0.0, 0.5, -0.9,  # rl hip/thigh/calf
    0.0, 0.5, -0.9,  # rr hip/thigh/calf
    0.0, 0.0, 0.0,    # arm base/shoulder/gripper
], dtype=np.float64)

# Joint names in the same order as sim (env.py QUADRUPED_JOINT_NAMES).
JOINT_NAMES = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
]

# XGO motor IDs in sim order [hip, thigh, calf] per leg.
# Hardware order is [calf, thigh, hip] (IDs 11=calf, 12=thigh, 13=hip per leg).
# We list IDs in sim order so send_joint_targets sends each command to the correct servo.
MOTOR_IDS = [13, 12, 11, 23, 22, 21, 33, 32, 31, 43, 42, 41, 51, 52, 53]

# Permutation: read_motor() returns [calf, thigh, hip, ...] in hardware order.
# This reorders to sim order [hip, thigh, calf, ...] for observations and offsets.
HW_TO_SIM = np.array([2, 1, 0, 5, 4, 3, 8, 7, 6, 11, 10, 9, 12, 13, 14])

# Actions are residuals around SIM_NEUTRAL_RAD, so action=0 is a stand.
# Must match ACTION_SCALE in env.py.
ACTION_SCALE = 0.3

# Default joint ranges from MJCF (radians).
JOINT_RANGES = np.array([
    [-0.6, 0.6], [-1.57, 1.57], [-2.2, 0.2],   # fl
    [-0.6, 0.6], [-1.57, 1.57], [-2.2, 0.2],   # fr
    [-0.6, 0.6], [-1.57, 1.57], [-2.2, 0.2],   # rl
    [-0.6, 0.6], [-1.57, 1.57], [-2.2, 0.2],   # rr
    [-1.57, 1.57], [-1.57, 1.57], [0.0, 1.2],  # arm
])

# Observation noise std used during sim training (must match env.py).
_OBS_NOISE_STD = np.array(
    [0.01] * 15 + [0.02] * 15 + [0.01] * 4
    + [0.03] * 3 + [0.05] * 3 + [0.005],
    dtype=np.float32,
)


def _rpy_to_quat(roll_deg: float, pitch_deg: float, yaw_deg: float):
    """Convert roll/pitch/yaw (degrees) to quaternion (w, x, y, z).

    Assumes intrinsic rotation order: yaw (z), pitch (y'), roll (x'').
    Body starts upright (identity quat) and is rotated by yaw, then pitch,
    then roll.
    """
    r = math.radians(roll_deg)
    p = math.radians(pitch_deg)
    y = math.radians(yaw_deg)

    cr, sr = math.cos(r / 2), math.sin(r / 2)
    cp, sp = math.cos(p / 2), math.sin(p / 2)
    cy, sy = math.cos(y / 2), math.sin(y / 2)

    w = cr * cp * cy + sr * sp * sy
    x = sr * cp * cy - cr * sp * sy
    y = cr * sp * cy + sr * cp * sy
    z = cr * cp * sy - sr * sp * cy

    q = np.array([w, x, y, z], dtype=np.float32)
    return q / (np.linalg.norm(q) + 1e-8)


class XGORobot:
    """Interface between the sim-trained ONNX policy and real XGO hardware.

    Observation (41-dim, can be extended to 43 with speed/heading):
      [joint_pos(15), joint_vel(15), trunk_quat(4), trunk_gyro(3),
       trunk_accel(3), trunk_height(1)]
    """

    def __init__(
        self,
        offsets: list[float] | np.ndarray | None = None,
        height: float = 0.12,
        use_imu_raw: bool = False,
        gyro_scale: float = 1.0 / 16.4 * math.pi / 180.0,  # MPU6050 raw -> rad/s
        accel_scale: float = 1.0,  # placeholder
        noise: bool = False,
        vel_alpha: float = 0.3,  # EMA smoothing for joint velocities
        gyro_alpha: float = 0.3,  # EMA smoothing for gyro
        vel_clip: float = 3.0,  # max joint velocity (rad/s)
        gyro_clip: float = 5.0,  # max gyro (rad/s)
    ):
        if XGO is None:
            raise RuntimeError("xgolib not available. Run on the DOGZILLA Pi.")

        self.dog = XGO(port="/dev/ttyAMA0", version="xgolite")
        self.height = height
        self.use_imu_raw = use_imu_raw
        self.gyro_scale = gyro_scale
        self.accel_scale = accel_scale
        self.noise = noise
        self.vel_alpha = vel_alpha
        self.gyro_alpha = gyro_alpha
        self.vel_clip = vel_clip
        self.gyro_clip = gyro_clip

        # Offsets convert sim radians -> real servo degrees.
        # If None, compute from current pose.
        self.offsets = self._ensure_offsets(offsets)

        self._prev_motor_deg: np.ndarray | None = None
        self._prev_rpy: np.ndarray | None = None
        self._prev_t: float | None = None
        self._smoothed_jvel = np.zeros(15, dtype=np.float64)
        self._smoothed_gyro = np.zeros(3, dtype=np.float64)
        self._rng = np.random.default_rng()

    def _ensure_offsets(self, offsets):
        if offsets is None:
            print("[XGORobot] No offsets provided; capturing current pose as neutral...")
            deg = np.array(self.dog.read_motor(), dtype=np.float64)[HW_TO_SIM]
            offsets = deg - np.degrees(SIM_NEUTRAL_RAD)
            print("[XGORobot] Computed offsets (deg):", offsets.tolist())
        return np.asarray(offsets, dtype=np.float64)

    def read_battery(self) -> Any:
        return self.dog.read_battery()

    def read_firmware(self) -> Any:
        return self.dog.read_firmware()

    def reset_pose(self):
        """Put robot in default standing pose."""
        self.dog.reset()
        time.sleep(0.5)

    def read_sensors(self, dt: float | None = None) -> np.ndarray:
        """Build the 41-dim observation vector."""
        t = time.time()
        motor_deg = np.array(self.dog.read_motor(), dtype=np.float64)[HW_TO_SIM]
        joint_pos = np.radians(motor_deg - self.offsets).astype(np.float32)

        # Joint velocities from finite difference, with EMA smoothing and clipping.
        if self._prev_motor_deg is not None and self._prev_t is not None:
            _dt = dt if dt is not None else (t - self._prev_t)
            if _dt > 1e-6:
                raw_vel = np.radians(motor_deg - self._prev_motor_deg) / _dt
            else:
                raw_vel = np.zeros(15, dtype=np.float64)
        else:
            raw_vel = np.zeros(15, dtype=np.float64)
        # Clip spikes from servo quantization noise.
        raw_vel = np.clip(raw_vel, -self.vel_clip, self.vel_clip)
        # Exponential moving average to smooth out servo jitter.
        self._smoothed_jvel = self.vel_alpha * raw_vel + (1.0 - self.vel_alpha) * self._smoothed_jvel
        joint_vel = self._smoothed_jvel.astype(np.float32)
        self._prev_motor_deg = motor_deg

        # IMU orientation.
        roll = float(self.dog.read_roll())
        pitch = float(self.dog.read_pitch())
        yaw = float(self.dog.read_yaw())
        rpy = np.array([roll, pitch, yaw], dtype=np.float64)
        quat = _rpy_to_quat(roll, pitch, yaw).astype(np.float32)

        # Gyro from Euler derivative, with EMA smoothing and clipping.
        if self._prev_rpy is not None and self._prev_t is not None:
            _dt = dt if dt is not None else (t - self._prev_t)
            if _dt > 1e-6:
                d_rpy = np.radians(rpy - self._prev_rpy) / _dt
            else:
                d_rpy = np.zeros(3, dtype=np.float64)
        else:
            d_rpy = np.zeros(3, dtype=np.float64)
        d_rpy = np.clip(d_rpy, -self.gyro_clip, self.gyro_clip)
        self._smoothed_gyro = self.gyro_alpha * d_rpy + (1.0 - self.gyro_alpha) * self._smoothed_gyro
        gyro = self._smoothed_gyro.astype(np.float32)

        # If raw IMU is requested, try to use it instead.
        if self.use_imu_raw:
            try:
                raw = self.dog.read_imu()
                if len(raw) >= 6:
                    # Indices may be [acc_x, acc_y, acc_z, gyro_x, gyro_y, gyro_z]
                    accel = np.array(raw[:3], dtype=np.float64) * self.accel_scale
                    gyro_raw = np.array(raw[3:6], dtype=np.float64) * self.gyro_scale
                    accel = accel.astype(np.float32)
                    gyro_raw = np.clip(gyro_raw, -self.gyro_clip, self.gyro_clip)
                    self._smoothed_gyro = self.gyro_alpha * gyro_raw + (1.0 - self.gyro_alpha) * self._smoothed_gyro
                    gyro = self._smoothed_gyro.astype(np.float32)
                else:
                    accel = np.zeros(3, dtype=np.float32)
            except Exception:
                accel = np.zeros(3, dtype=np.float32)
        else:
            # Synthesize gravity in body frame from roll/pitch.
            r = math.radians(roll)
            p = math.radians(pitch)
            accel = np.array([
                math.sin(p) * 9.81,
                -math.sin(r) * math.cos(p) * 9.81,
                math.cos(r) * math.cos(p) * 9.81,
            ], dtype=np.float32)

        self._prev_rpy = rpy
        self._prev_t = t

        # Estimate trunk height from leg kinematics (thigh + calf projection)
        # instead of using a fixed constant. The sim uses qpos[2] which varies
        # during walking; a fixed 0.12 hides the bobbing motion the policy
        # learned to use as feedback.
        thigh_angles = joint_pos[1::3]  # thigh for each leg (indices 1,4,7,10)
        calf_angles = joint_pos[2::3]  # calf for each leg (indices 2,5,8,11)
        # Leg length: 0.06m per link. Height = sum of vertical projections.
        leg_heights = 0.06 * np.cos(thigh_angles) + 0.06 * np.cos(thigh_angles + calf_angles)
        est_height = float(np.mean(leg_heights))

        obs = np.concatenate([
            joint_pos,
            joint_vel,
            quat,
            gyro,
            accel,
            [est_height],
        ]).astype(np.float32)

        if self.noise:
            obs = obs + self._rng.normal(0.0, _OBS_NOISE_STD).astype(np.float32)

        return obs

    def send_joint_targets(self, actions: np.ndarray, torque_limit: float = 1.0,
                           max_step_deg: float = 8.0):
        """Send 15 normalized actions in [-1, 1] to real servos.

        Matches the sim's _action_to_joint_targets: actions are residuals
        around the neutral stand pose, so action=0 holds a stand. Sends
        full target positions directly — the sim's position actuators also
        command the full target and let the joint track as fast as physics
        allows. Rate limiting was removed because read_motor() returns
        actual (slow) servo positions, creating a feedback loop where the
        delta is always clipped and servos never reach the target.
        """
        actions = np.asarray(actions, dtype=np.float64).flatten()
        if actions.shape != (15,):
            raise ValueError(f"Expected 15 joint targets, got {actions.shape}")

        # Residual around the neutral pose (radians), same as sim env.
        lo, hi = JOINT_RANGES[:, 0], JOINT_RANGES[:, 1]
        joint_targets_rad = SIM_NEUTRAL_RAD + ACTION_SCALE * np.clip(actions, -1.0, 1.0)

        # Clip to sim joint ranges BEFORE adding offsets.
        joint_targets_rad = np.clip(joint_targets_rad, lo, hi)

        # Convert to degrees and add hardware offsets.
        target_deg = np.degrees(joint_targets_rad) + self.offsets

        # Send to XGO. motor() takes motor_id and angle in degrees.
        for motor_id, target in zip(MOTOR_IDS, target_deg.tolist()):
            self.dog.motor(motor_id, target)

    def close(self):
        pass


def load_offsets(path: str) -> np.ndarray:
    with open(path) as f:
        data = json.load(f)
    return np.array(data["offsets"], dtype=np.float64)


def save_offsets(path: str, robot: XGORobot):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w") as f:
        json.dump({
            "offsets": robot.offsets.tolist(),
            "joint_names": JOINT_NAMES,
        }, f, indent=2)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="XGO robot sensor test")
    parser.add_argument("--calibrate", action="store_true", help="Capture offsets and save")
    parser.add_argument("--offsets-out", default="/home/pi/xgo_offsets.json")
    parser.add_argument("--sweep-motors", action="store_true",
                        help="Move each motor 1-15 one at a time to verify mapping")
    args = parser.parse_args()

    robot = XGORobot()
    print("Firmware:", robot.read_firmware())
    print("Battery:", robot.read_battery())
    print("Offsets (deg):", robot.offsets.tolist())
    print("First observation:", robot.read_sensors()[:15])

    if args.calibrate:
        save_offsets(args.offsets_out, robot)
        print(f"Saved offsets to {args.offsets_out}")

    if args.sweep_motors:
        print("\n=== MOTOR SWEEP ===")
        print("Each motor will move +20deg then back. Watch which leg/joint moves.")
        print("Expected mapping: 13=fl_hip 12=fl_thigh 11=fl_calf "
              "23=fr_hip 22=fr_thigh 21=fr_calf "
              "33=rl_hip 32=rl_thigh 31=rl_calf "
              "43=rr_hip 42=rr_thigh 41=rr_calf "
              "51=arm_base 52=arm_shoulder 53=arm_gripper")
        input("Press Enter to start (make sure robot is on a stable surface)...")
        hw_ids = [11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43, 51, 52, 53]
        for idx, mid in enumerate(MOTOR_IDS):
            hw_idx = hw_ids.index(mid)
            current = float(robot.dog.read_motor()[hw_idx])
            print(f"\nMotor {mid} ({JOINT_NAMES[idx]}): current={current:.1f}deg, moving +20deg...")
            robot.dog.motor(mid, current + 20.0)
            time.sleep(1.5)
            print(f"  returning to {current:.1f}deg...")
            robot.dog.motor(mid, current)
            time.sleep(1.0)
        print("\nSweep complete. Robot returning to neutral...")
        robot.reset_pose()
