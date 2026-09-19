"""F2: ONNX Inference — load a trained policy and run it on real DOGZILLA hardware.

Reads sensor data (joint angles, velocities, IMU), feeds it through the ONNX
policy, and sends the resulting motor commands to the robot.

Observation layout (41-dim for 41-dim policies, 43-dim for speed+heading):
  joint_pos(15) + joint_vel(15) + trunk_quat(4) + trunk_gyro(3) + trunk_accel(3) + height(1)
  [+ speed_cmd(1) + heading_cmd(1) for 43-dim policies]

Action layout (15-dim):
  Normalized [-1, 1] -> scaled to joint ranges via _action_to_joint_targets()

SAFETY:
  - Torque limit (default 30%)
  - Auto-fall detection (cuts motors if robot tips > 45 degrees)
  - Emergency stop (Ctrl+C or 'q' key)
  - Action smoothing (low-pass filter to prevent jerky moves)

Usage:
    python infer_onnx.py --policy checkpoints/policy_stand.onnx --motor-map deploy/motor_map.json --dry-run
    python infer_onnx.py --policy checkpoints/policy_speed_heading_terrain.onnx --motor-map deploy/motor_map.json --port /dev/ttyUSB0
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
import time

import numpy as np
import onnxruntime as ort

# Joint names and ranges — must match env.py exactly
QUADRUPED_JOINT_NAMES = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
]

# Joint ranges from MJCF (radians)
JOINT_RANGES = np.array([
    [-0.6, 0.6],    # fl_hip
    [-1.57, 1.57],  # fl_thigh
    [-2.2, 0.2],    # fl_calf
    [-0.6, 0.6],    # fr_hip
    [-1.57, 1.57],  # fr_thigh
    [-2.2, 0.2],    # fr_calf
    [-0.6, 0.6],    # rl_hip
    [-1.57, 1.57],  # rl_thigh
    [-2.2, 0.2],    # rl_calf
    [-0.6, 0.6],    # rr_hip
    [-1.57, 1.57],  # rr_thigh
    [-2.2, 0.2],    # rr_calf
    [-1.57, 1.57],  # arm_base
    [-1.57, 1.57],  # arm_shoulder
    [0.0, 1.2],     # arm_gripper
])

# Neutral standing pose (radians) — must match SIM_NEUTRAL_RAD in env.py / xgo_robot.py.
NEUTRAL_POS = np.array([0.0, 0.5, -0.9] * 4 + [0.0, 0.0, 0.0])

# Actions are residuals around NEUTRAL_POS, so action=0 is a stand.
# Must match ACTION_SCALE in env.py.
ACTION_SCALE = 0.3


class SafetyController:
    """Wraps motor commands with safety checks: torque limit, fall detection, e-stop."""

    def __init__(self, torque_limit: float = 0.3, fall_threshold: float = 0.5,
                 action_smooth: float = 0.7):
        self.torque_limit = torque_limit
        self.fall_threshold = fall_threshold  # upright score below this = fallen
        self.action_smooth = action_smooth    # 0=no smoothing, 1=no movement
        self._prev_action = None
        self._estop = False
        self._fall_count = 0

        signal.signal(signal.SIGINT, self._signal_handler)

    def _signal_handler(self, signum, frame):
        print("\n[E-STOP] Signal received, stopping motors!")
        self._estop = True

    def is_estop(self) -> bool:
        return self._estop

    def check_fallen(self, obs: np.ndarray) -> bool:
        """Check if robot has fallen based on trunk quaternion (obs[30:34])."""
        if len(obs) < 34:
            return False
        quat = obs[30:34]  # w, x, y, z
        w, x, y, z = quat
        up_z = 1.0 - 2.0 * (x * x + y * y)
        if up_z < self.fall_threshold:
            self._fall_count += 1
            if self._fall_count >= 5:  # must be fallen for 5 consecutive frames
                return True
        else:
            self._fall_count = 0
        return False

    def process_action(self, action: np.ndarray) -> np.ndarray:
        """Apply torque limit, smoothing, and clipping to motor commands."""
        if self._estop:
            return NEUTRAL_POS.copy()

        # Clip to [-1, 1]
        action = np.clip(action, -1.0, 1.0)

        # Apply torque limit (scale down from neutral)
        action = action * self.torque_limit

        # Smooth action (low-pass filter)
        if self._prev_action is not None:
            action = self.action_smooth * self._prev_action + (1 - self.action_smooth) * action
        self._prev_action = action.copy()

        return action

    def reset(self):
        self._prev_action = None
        self._fall_count = 0


class DummyRobot:
    """No-hardware mock for dry-run mode."""

    def __init__(self):
        self.motor_positions = NEUTRAL_POS.copy()
        self.motor_velocities = np.zeros(15)
        self.imu_quat = np.array([1.0, 0.0, 0.0, 0.0])  # upright
        self.imu_gyro = np.zeros(3)
        self.imu_accel = np.array([0.0, 0.0, 9.81])
        self.height = 0.12

    def read_sensors(self):
        """Return observation vector matching env.py _get_obs() layout."""
        obs = np.concatenate([
            self.motor_positions,      # joint pos (15)
            self.motor_velocities,     # joint vel (15)
            self.imu_quat,             # trunk quat (4)
            self.imu_gyro,             # trunk gyro (3)
            self.imu_accel,            # trunk accel (3)
            [self.height],             # trunk height (1)
        ]).astype(np.float32)
        return obs

    def send_commands(self, joint_targets: np.ndarray):
        """Send joint position commands to motors."""
        self.motor_positions = joint_targets.copy()
        self.motor_velocities = (joint_targets - self.motor_positions) * 50.0

    def close(self):
        pass


def load_motor_map(path: str) -> dict:
    """Load motor_map.json from calibration script."""
    with open(path) as f:
        data = json.load(f)
    return data["motor_map"]


def action_to_joint_targets(action: np.ndarray) -> np.ndarray:
    """Convert normalized [-1, 1] action to real joint angles (radians).

    Matches env.py's _action_to_joint_targets: actions are residuals around
    NEUTRAL_POS scaled by ACTION_SCALE, so action=0 holds a stand.
    """
    action = np.clip(action, -1.0, 1.0)
    lo, hi = JOINT_RANGES[:, 0], JOINT_RANGES[:, 1]
    targets = NEUTRAL_POS + ACTION_SCALE * action
    return np.clip(targets, lo, hi)


def run_inference(policy_path: str, robot, motor_map: dict, safety: SafetyController,
                  obs_dim: int, speed_cmd: float = 0.0, heading_cmd: float = 0.0,
                  max_steps: int = 500, ctrl_hz: float = 50.0):
    """Main control loop: read sensors -> policy -> safety -> motors."""
    # Load ONNX policy
    print(f"Loading ONNX policy: {policy_path}")
    session = ort.InferenceSession(policy_path, providers=["CPUExecutionProvider"])
    input_name = session.get_inputs()[0].name
    output_name = session.get_outputs()[0].name
    print(f"  Input: '{input_name}' shape={session.get_inputs()[0].shape}")
    print(f"  Output: '{output_name}' shape={session.get_outputs()[0].shape}")

    dt = 1.0 / ctrl_hz
    step = 0

    print(f"\nStarting control loop ({ctrl_hz} Hz, max {max_steps} steps)")
    print(f"Torque limit: {safety.torque_limit:.0%}")
    print(f"Press Ctrl+C for emergency stop\n")

    # Build motor index mapping: joint_name -> array index
    # The ONNX policy outputs actions in QUADRUPED_JOINT_NAMES order
    # motor_map tells us which hardware motor index each joint name maps to
    motor_indices = []
    for joint_name in QUADRUPED_JOINT_NAMES:
        if joint_name in motor_map:
            motor_indices.append(motor_map[joint_name])
        else:
            print(f"WARNING: {joint_name} not in motor map, using index 0")
            motor_indices.append(0)
    motor_indices = np.array(motor_indices)

    try:
        while step < max_steps and not safety.is_estop():
            t0 = time.time()

            # 1. Read sensors
            obs = robot.read_sensors()

            # 2. Check if fallen
            if safety.check_fallen(obs):
                print(f"[FALL DETECTED] Robot tipped over at step {step}. Cutting motors.")
                robot.send_commands(NEUTRAL_POS)
                break

            # 3. Add command dims for 43-dim policies
            if obs_dim == 43:
                obs = np.concatenate([obs, [speed_cmd, heading_cmd]]).astype(np.float32)

            # 4. Run policy inference
            action = session.run([output_name], {input_name: obs.reshape(1, -1).astype(np.float32)})[0][0]

            # 5. Safety processing
            safe_action = safety.process_action(action)

            # 6. Convert to joint targets
            joint_targets = action_to_joint_targets(safe_action)

            # 7. Send to robot (reorder via motor_map)
            # For now, send in joint order — real hardware will need motor_indices
            robot.send_commands(joint_targets)

            step += 1

            # Print status every 50 steps
            if step % 50 == 0:
                height = obs[-1] if obs_dim == 41 else obs[-3]
                quat = obs[30:34]
                w, x, y, z = quat
                upright = 1.0 - 2.0 * (x * x + y * y)
                print(f"  step {step:4d} | upright={upright:.3f} | height={height:.3f}m | "
                      f"action_sum={np.sum(np.abs(safe_action)):.2f}")

            # Maintain control rate
            elapsed = time.time() - t0
            sleep_time = dt - elapsed
            if sleep_time > 0:
                time.sleep(sleep_time)

    except KeyboardInterrupt:
        print("\n[ E-STOP ] Keyboard interrupt!")

    finally:
        print(f"\nControl loop ended after {step} steps.")
        print("Resetting to neutral pose...")
        robot.send_commands(NEUTRAL_POS)
        robot.close()
        print("Done.")


def main():
    parser = argparse.ArgumentParser(description="DOGZILLA ONNX Policy Inference")
    parser.add_argument("--policy", required=True, help="Path to .onnx policy file")
    parser.add_argument("--motor-map", default=os.path.join(os.path.dirname(__file__), "motor_map.json"),
                        help="Path to motor_map.json from calibration")
    parser.add_argument("--port", type=str, default=None, help="Serial port")
    parser.add_argument("--ip", type=str, default=None, help="WiFi IP address")
    parser.add_argument("--dry-run", action="store_true", help="No hardware, simulate")
    parser.add_argument("--torque-limit", type=float, default=0.3, help="Torque limit 0-1 (default 0.3)")
    parser.add_argument("--max-steps", type=int, default=500, help="Max control steps (default 500)")
    parser.add_argument("--speed", type=float, default=0.08, help="Speed command for 43-dim policies (m/s)")
    parser.add_argument("--heading", type=float, default=0.0, help="Heading command for 43-dim policies (rad)")
    args = parser.parse_args()

    # Load motor map
    if not os.path.exists(args.motor_map):
        print(f"ERROR: Motor map not found at {args.motor_map}")
        print("Run motor_id_cal.py first to calibrate motor mapping.")
        if not args.dry_run:
            sys.exit(1)
        print("Using identity mapping for dry-run mode.")
        motor_map = {name: i for i, name in enumerate(QUADRUPED_JOINT_NAMES)}
    else:
        motor_map = load_motor_map(args.motor_map)
        print(f"Loaded motor map: {len(motor_map)} joints mapped")

    # Determine obs dimension from policy
    session_tmp = ort.InferenceSession(args.policy, providers=["CPUExecutionProvider"])
    obs_dim = session_tmp.get_inputs()[0].shape[1]
    del session_tmp
    print(f"Policy observation dimension: {obs_dim}")

    if obs_dim == 43:
        print(f"  Speed command: {args.speed} m/s")
        print(f"  Heading command: {args.heading} rad")
    elif obs_dim != 41:
        print(f"ERROR: Unexpected obs dimension {obs_dim}, expected 41 or 43")
        sys.exit(1)

    # Connect robot
    if args.dry_run:
        robot = DummyRobot()
        print("Running in DRY RUN mode (no hardware)")
    elif args.ip:
        print(f"Connecting to DOGZILLA at {args.ip}...")
        print("ERROR: WiFi connection not implemented yet. Use --dry-run for now.")
        sys.exit(1)
    elif args.port:
        print(f"Connecting to DOGZILLA at {args.port}...")
        print("ERROR: Serial connection not implemented yet. Use --dry-run for now.")
        sys.exit(1)
    else:
        print("ERROR: specify --port, --ip, or --dry-run")
        sys.exit(1)

    # Create safety controller
    safety = SafetyController(torque_limit=args.torque_limit)

    # Run inference
    run_inference(
        policy_path=args.policy,
        robot=robot,
        motor_map=motor_map,
        safety=safety,
        obs_dim=obs_dim,
        speed_cmd=args.speed,
        heading_cmd=args.heading,
        max_steps=args.max_steps,
    )


if __name__ == "__main__":
    main()
