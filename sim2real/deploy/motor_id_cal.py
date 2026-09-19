"""F1: Motor ID Calibration — sweep each motor to map hardware indices to joint names.

The DOGZILLA-Lite has 12 leg servos (3 per leg x 4 legs) + 3 arm servos = 15 total.
This script sends a small position command to ONE motor at a time, pauses, and
asks the user to confirm which joint moved. The result is a motor_map.json file
that the inference script uses to route ONNX policy actions to the correct motors.

PREREQUISITES:
- DOGZILLA powered on, connected via USB or WiFi
- DOGZILLA SDK installed (pip install dogzilla-sdk or equivalent)
- Robot on a stand or held so legs can move freely

SAFETY:
- Only moves ONE motor at a time
- Small movement (10 degrees from neutral)
- 1 second pause between motors
- User confirms each movement

Usage:
    python motor_id_cal.py --port /dev/ttyUSB0    # serial
    python motor_id_cal.py --ip 192.168.1.100      # wifi
    python motor_id_cal.py --dry-run               # no hardware, just print
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

# Joint names in the same order as our sim (env.py QUADRUPED_JOINT_NAMES)
EXPECTED_JOINTS = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
]

# Neutral positions (radians) — standing pose from MJCF keyframe
NEUTRAL_POS = {
    "fl_hip": 0.0, "fl_thigh": 0.0, "fl_calf": -1.4,
    "fr_hip": 0.0, "fr_thigh": 0.0, "fr_calf": -1.4,
    "rl_hip": 0.0, "rl_thigh": 0.0, "rl_calf": -1.4,
    "rr_hip": 0.0, "rr_thigh": 0.0, "rr_calf": -1.4,
    "arm_base": 0.0, "arm_shoulder": 0.0, "arm_gripper": 0.0,
}

# Small test movement (10 degrees in radians)
TEST_DELTA = 0.175

OUTPUT_FILE = os.path.join(os.path.dirname(__file__), "motor_map.json")


class DummyRobot:
    """No-hardware mock for dry-run mode."""
    def __init__(self):
        self.positions = [0.0] * 15

    def set_motor(self, idx: int, position: float):
        self.positions[idx] = position
        print(f"  [DRY RUN] motor[{idx}] -> {position:.3f} rad ({position * 57.296:.1f} deg)")

    def get_motor(self, idx: int) -> float:
        return self.positions[idx]

    def close(self):
        pass


def connect_robot(args):
    """Connect to the DOGZILLA hardware. Replace with actual SDK calls."""
    if args.dry_run:
        return DummyRobot()

    if args.ip:
        print(f"Connecting to DOGZILLA at {args.ip}...")
        # TODO: Replace with actual DOGZILLA WiFi SDK
        # from dogzilla import Dogzilla
        # return Dogzilla(ip=args.ip)
        print("ERROR: WiFi connection not implemented yet. Use --dry-run for now.")
        sys.exit(1)
    elif args.port:
        print(f"Connecting to DOGZILLA at {args.port}...")
        # TODO: Replace with actual DOGZILLA serial SDK
        # from dogzilla import Dogzilla
        # return Dogzilla(port=args.port)
        print("ERROR: Serial connection not implemented yet. Use --dry-run for now.")
        sys.exit(1)
    else:
        print("ERROR: specify --port, --ip, or --dry-run")
        sys.exit(1)


def calibrate(robot, n_motors: int = 15):
    """Sweep each motor, ask user to identify which joint moved."""
    print("\n" + "=" * 60)
    print("MOTOR ID CALIBRATION")
    print("=" * 60)
    print(f"\nWe will move {n_motors} motors one at a time.")
    print(f"Each motor will move ~10 degrees from neutral, hold for 1s, then return.")
    print(f"Watch carefully and tell us which joint moved.\n")
    print("Joint options:")
    for i, name in enumerate(EXPECTED_JOINTS):
        print(f"  {i}: {name}")
    print()

    # Reset all motors to neutral
    print("Resetting all motors to neutral position...")
    for i in range(n_motors):
        robot.set_motor(i, 0.0)
    time.sleep(1.0)

    motor_map = {}  # joint_name -> motor_index

    for motor_idx in range(n_motors):
        print(f"\n--- Motor {motor_idx} ---")
        input(f"Press ENTER to move motor {motor_idx}... (watch the robot)")

        # Move motor slightly
        robot.set_motor(motor_idx, TEST_DELTA)
        time.sleep(1.0)

        # Return to neutral
        robot.set_motor(motor_idx, 0.0)
        time.sleep(0.5)

        # Ask user which joint moved
        while True:
            response = input(f"Which joint moved? (0-{len(EXPECTED_JOINTS)-1} or 'skip' or 'none'): ").strip()
            if response.lower() == "skip":
                print(f"  Skipping motor {motor_idx}")
                break
            if response.lower() == "none":
                print(f"  Motor {motor_idx} -> NONE (no visible movement)")
                break
            try:
                joint_idx = int(response)
                if 0 <= joint_idx < len(EXPECTED_JOINTS):
                    joint_name = EXPECTED_JOINTS[joint_idx]
                    motor_map[joint_name] = motor_idx
                    print(f"  Motor {motor_idx} -> {joint_name}")
                    break
                else:
                    print(f"  Invalid index. Enter 0-{len(EXPECTED_JOINTS)-1}")
            except ValueError:
                print(f"  Enter a number 0-{len(EXPECTED_JOINTS)-1}, 'skip', or 'none'")

    return motor_map


def main():
    parser = argparse.ArgumentParser(description="DOGZILLA Motor ID Calibration")
    parser.add_argument("--port", type=str, default=None, help="Serial port (e.g. /dev/ttyUSB0)")
    parser.add_argument("--ip", type=str, default=None, help="WiFi IP address")
    parser.add_argument("--dry-run", action="store_true", help="No hardware, just simulate")
    parser.add_argument("--n-motors", type=int, default=15, help="Number of motors (default 15)")
    args = parser.parse_args()

    robot = connect_robot(args)

    try:
        motor_map = calibrate(robot, args.n_motors)

        # Save results
        result = {
            "motor_map": motor_map,
            "joint_order": EXPECTED_JOINTS,
            "neutral_pos": NEUTRAL_POS,
            "n_motors_scanned": args.n_motors,
            "joints_mapped": len(motor_map),
            "joints_missing": [j for j in EXPECTED_JOINTS if j not in motor_map],
        }

        with open(OUTPUT_FILE, "w") as f:
            json.dump(result, f, indent=2)

        print("\n" + "=" * 60)
        print("CALIBRATION COMPLETE")
        print("=" * 60)
        print(f"\nMapped {len(motor_map)}/{len(EXPECTED_JOINTS)} joints:")
        for joint_name, motor_idx in sorted(motor_map.items(), key=lambda x: x[1]):
            print(f"  motor[{motor_idx}] -> {joint_name}")

        if result["joints_missing"]:
            print(f"\nWARNING: {len(result['joints_missing'])} joints not mapped:")
            for j in result["joints_missing"]:
                print(f"  - {j}")
            print("Re-run calibration for missing joints or map manually.")

        print(f"\nSaved to {OUTPUT_FILE}")
        print(f"Next: python infer_onnx.py --policy checkpoints/policy_stand.onnx --motor-map {OUTPUT_FILE}")

    finally:
        # Reset all motors to neutral before exiting
        print("\nResetting all motors to neutral...")
        for i in range(args.n_motors):
            robot.set_motor(i, 0.0)
        robot.close()


if __name__ == "__main__":
    main()
