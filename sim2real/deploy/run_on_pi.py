"""Run a sim-trained ONNX policy on real DOGZILLA-Lite (XGO) hardware.

This script is meant to be run directly on the Raspberry Pi inside the robot.

Usage on the robot:
    source /home/pi/RaspberryPi-CM5/xgovenv/bin/activate
    python3 run_on_pi.py --policy policy_stand.onnx --calibrate-only
    python3 run_on_pi.py --policy policy_stand.onnx --torque-limit 0.3 --max-steps 100
"""

from __future__ import annotations

import argparse
import os
import signal
import sys
import time

import numpy as np

try:
    import onnxruntime as ort
except ImportError:
    ort = None  # type: ignore

from xgo_robot import XGORobot, JOINT_NAMES, save_offsets, load_offsets


def _fallback_numpy_policy(onnx_path: str):
    """Load ONNX weights into pure NumPy if onnxruntime is unavailable."""
    try:
        import onnx
    except ImportError:
        raise RuntimeError(
            "Neither onnxruntime nor onnx is installed. "
            "Install with: pip install onnxruntime"
        )

    model = onnx.load(onnx_path)
    init = {i.name: i for i in model.graph.initializer}

    def get_tensor(name: str) -> np.ndarray:
        t = init[name]
        return np.array(t.float_data).reshape(t.dims).astype(np.float64)

    tensors = {i.name: get_tensor(i.name) for i in init.values()}

    W1 = b1 = W2 = b2 = W3 = b3 = None
    for name, arr in tensors.items():
        if len(arr.shape) == 2:
            if arr.shape[1] >= 41 and arr.shape[0] == 256:
                W1 = arr.T
            elif arr.shape[1] == 256 and arr.shape[0] == 256:
                if W2 is None:
                    W2 = arr.T
                else:
                    W3 = arr.T
        elif len(arr.shape) == 1:
            if arr.shape[0] == 256:
                if b1 is None:
                    b1 = arr
                else:
                    b2 = arr
            elif arr.shape[0] == 15:
                b3 = arr

    if W1 is None or W2 is None or W3 is None:
        raise RuntimeError("Could not auto-extract MLP weights from ONNX")

    def predict(obs: np.ndarray) -> np.ndarray:
        x = obs.astype(np.float64)
        x = np.maximum(x @ W1 + b1, 0)
        x = np.maximum(x @ W2 + b2, 0)
        return np.tanh(x @ W3 + b3).astype(np.float32)

    return predict


class SafetyController:
    """Wraps motor commands with safety checks."""

    def __init__(
        self,
        torque_limit: float = 1.0,
        fall_threshold: float = 0.5,
        action_smooth: float = 0.0,
    ):
        self.torque_limit = torque_limit
        self.fall_threshold = fall_threshold
        self.action_smooth = action_smooth
        self._prev_action: np.ndarray | None = None
        self._estop = False
        self._fall_count = 0
        try:
            signal.signal(signal.SIGINT, self._signal_handler)
        except (ValueError, OSError):
            pass  # not in main thread — that's fine

    def _signal_handler(self, signum, frame):
        print("\n[E-STOP] Signal received, stopping motors!")
        self._estop = True

    def is_estop(self) -> bool:
        return self._estop

    def check_fallen(self, obs: np.ndarray) -> bool:
        if len(obs) < 34:
            return False
        quat = obs[30:34]
        w, x, y, z = quat
        up_z = 1.0 - 2.0 * (x * x + y * y)
        if up_z < self.fall_threshold:
            self._fall_count += 1
            if self._fall_count >= 5:
                return True
        else:
            self._fall_count = 0
        return False

    def process_action(self, action: np.ndarray) -> np.ndarray:
        if self._estop:
            return np.zeros(15, dtype=np.float32)
        action = np.clip(action, -self.torque_limit, self.torque_limit)
        if self._prev_action is not None and self.action_smooth > 0:
            action = self.action_smooth * self._prev_action + (1 - self.action_smooth) * action
        self._prev_action = action.copy()
        return action

    def reset(self):
        self._prev_action = None
        self._fall_count = 0


def run_policy(
    policy_path: str,
    robot: XGORobot,
    safety: SafetyController,
    obs_dim: int,
    speed_cmd: float = 0.0,
    heading_cmd: float = 0.0,
    max_steps: int = 100,
    ctrl_hz: float = 50.0,
):
    if ort is not None:
        session = ort.InferenceSession(policy_path, providers=["CPUExecutionProvider"])
        input_name = session.get_inputs()[0].name
        output_name = session.get_outputs()[0].name
        print(f"  ONNX input: {session.get_inputs()[0].shape}")
        print(f"  ONNX output: {session.get_outputs()[0].shape}")

        def predict(obs: np.ndarray) -> np.ndarray:
            return session.run([output_name], {input_name: obs.reshape(1, -1).astype(np.float32)})[0][0]
    else:
        print("[WARN] onnxruntime not installed; using pure NumPy fallback")
        predict = _fallback_numpy_policy(policy_path)

    dt = 1.0 / ctrl_hz
    step = 0

    print(f"\nStarting control loop ({ctrl_hz} Hz, max {max_steps} steps)")
    print(f"Torque limit: {safety.torque_limit:.0%}")
    print(f"Speed command: {speed_cmd} m/s")
    print(f"Heading command: {heading_cmd} rad")
    print("Press Ctrl+C for emergency stop\n")

    print("Moving to neutral pose...")
    robot.reset_pose()
    time.sleep(0.5)

    try:
        while step < max_steps and not safety.is_estop():
            t0 = time.time()

            obs = robot.read_sensors(dt=dt)

            if safety.check_fallen(obs):
                print(f"[FALL DETECTED] Robot tipped over at step {step}. Stopping.")
                break

            if obs_dim == 43:
                obs = np.concatenate([obs, [speed_cmd, heading_cmd]]).astype(np.float32)

            action = predict(obs)
            safe_action = safety.process_action(action)
            robot.send_joint_targets(safe_action, torque_limit=1.0)

            step += 1

            # Debug: print raw action and key obs for first 10 steps, then every 50
            if step <= 10 or step % 50 == 0:
                act_str = np.array2string(safe_action, precision=3, separator=',', suppress_small=True)
                obs_jpos = obs[:15]
                jpos_str = np.array2string(obs_jpos, precision=3, separator=',', suppress_small=True)
                print(
                    f"  step {step:4d} | action=[{act_str}]"
                )
                if step <= 5:
                    print(f"           | jpos=[{jpos_str}]")
                    print(f"           | jvel=[{np.array2string(obs[15:30], precision=3, separator=',', suppress_small=True)}]")
                if step <= 3:
                    print(f"           | quat=[{np.array2string(obs[30:34], precision=3, separator=',', suppress_small=True)}]")
                    print(f"           | gyro=[{np.array2string(obs[34:37], precision=3, separator=',', suppress_small=True)}]")
                    print(f"           | accel=[{np.array2string(obs[37:40], precision=3, separator=',', suppress_small=True)}]")
                    print(f"           | height={obs[40]:.4f}")
                    if obs_dim == 43:
                        print(f"           | speed_cmd={obs[41]:.3f} heading_cmd={obs[42]:.3f}")

            if step % 25 == 0:
                height = obs[-1] if obs_dim == 41 else obs[-3]
                quat = obs[30:34]
                w, x, y, z = quat
                upright = 1.0 - 2.0 * (x * x + y * y)
                # Per-leg action breakdown
                fl = np.sum(np.abs(safe_action[0:3]))
                fr = np.sum(np.abs(safe_action[3:6]))
                rl = np.sum(np.abs(safe_action[6:9]))
                rr = np.sum(np.abs(safe_action[9:12]))
                arm = np.sum(np.abs(safe_action[12:15]))
                print(
                    f"  step {step:4d} | upright={upright:.3f} | "
                    f"height={height:.3f}m | sum={np.sum(np.abs(safe_action)):.2f} | "
                    f"FL={fl:.2f} FR={fr:.2f} RL={rl:.2f} RR={rr:.2f} ARM={arm:.2f}"
                )

            elapsed = time.time() - t0
            sleep_time = dt - elapsed
            if sleep_time > 0:
                time.sleep(sleep_time)

    except KeyboardInterrupt:
        print("\n[E-STOP] Keyboard interrupt!")
    finally:
        print(f"\nControl loop ended after {step} steps.")
        print("Returning to neutral pose...")
        try:
            robot.reset_pose()
        except Exception as e:
            print(f"Could not reset pose: {e}")


def main():
    parser = argparse.ArgumentParser(description="Run ONNX policy on DOGZILLA-Lite")
    parser.add_argument("--policy", required=True, help="Path to .onnx policy")
    parser.add_argument("--offsets", default="/home/pi/xgo_offsets.json",
                        help="Path to saved offsets")
    parser.add_argument("--calibrate-only", action="store_true",
                        help="Capture neutral offsets and exit")
    parser.add_argument("--torque-limit", type=float, default=1.0,
                        help="Action clip limit 0-1 (default 1.0 = full range, matches sim)")
    parser.add_argument("--max-steps", type=int, default=100,
                        help="Max control steps (default 100 = 2 sec)")
    parser.add_argument("--speed", type=float, default=0.08,
                        help="Speed command for 43-dim policies (m/s)")
    parser.add_argument("--heading", type=float, default=0.0,
                        help="Heading command for 43-dim policies (rad)")
    parser.add_argument("--fall-threshold", type=float, default=0.5,
                        help="Upright score below this triggers fall detection")
    parser.add_argument("--action-smooth", type=float, default=0.0,
                        help="Action low-pass smoothing 0-1 (default 0 = no smoothing, matches sim)")
    args = parser.parse_args()

    if not os.path.exists(args.policy):
        print(f"ERROR: Policy not found: {args.policy}")
        sys.exit(1)

    offsets = None
    if os.path.exists(args.offsets):
        try:
            offsets = load_offsets(args.offsets)
            print(f"Loaded offsets from {args.offsets}")
        except Exception as e:
            print(f"Could not load offsets ({e}); will calibrate")

    robot = XGORobot(offsets=offsets, noise=True)
    print(f"Firmware: {robot.read_firmware()}")
    print(f"Battery:  {robot.read_battery()}")

    if args.calibrate_only:
        save_offsets(args.offsets, robot)
        print(f"Saved offsets to {args.offsets}")
        return

    if ort is not None:
        obs_dim = ort.InferenceSession(args.policy).get_inputs()[0].shape[1]
    else:
        import onnx
        obs_dim = onnx.load(args.policy).graph.input[0].type.tensor_type.shape.dim[1].dim_value

    print(f"Policy observation dimension: {obs_dim}")

    safety = SafetyController(
        torque_limit=args.torque_limit,
        fall_threshold=args.fall_threshold,
        action_smooth=args.action_smooth,
    )

    run_policy(
        policy_path=args.policy,
        robot=robot,
        safety=safety,
        obs_dim=obs_dim,
        speed_cmd=args.speed,
        heading_cmd=args.heading,
        max_steps=args.max_steps,
    )


if __name__ == "__main__":
    main()
