#!/usr/bin/env python3
"""Gazebo launch helper for DOGZILLA S2 + JunoClaw ROS2 bridge.

Usage:
    # Launch Gazebo with DOGZILLA S2 + JunoClaw bridge (simulation mode)
    python3 launch_gazebo.py --simulate

    # Launch with real ROS2 (requires Gazebo + ROS2 Humble installed)
    python3 launch_gazebo.py --robot-id dogzilla-s2-001

Prerequisites:
    - ROS2 Humble installed
    - Gazebo (Ignition Fortress or Gazebo Garden)
    - DOGZILLA S2 URDF/XACRO from Yahboom's GitHub repo
    - junoclaw-ros2-bridge installed: pip install -e plugins/plugin-ros2/bridge

The launch sequence:
    1. Start Gazebo with empty world
    2. Spawn DOGZILLA S2 URDF in Gazebo
    3. Start junoclaw-ros2-bridge (connects to Gazebo's ROS2 topics)
    4. Bridge subscribes to /cmd_vel, /scan, /imu/data, /joint_states
    5. Send navigation commands via ros2 topic pub /cmd_vel
    6. Bridge captures sensor data → IntentMessage + ReflexBatchAttestation
"""

import argparse
import subprocess
import sys
import time
import os
import signal


def check_ros2():
    """Check if ROS2 is available."""
    try:
        result = subprocess.run(
            ["ros2", "--version"], capture_output=True, text=True, timeout=5
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def check_gazebo():
    """Check if Gazebo is available."""
    for cmd in ["gz", "ign gazebo", "gazebo"]:
        try:
            result = subprocess.run(
                cmd.split(), capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0 or "usage" in result.stderr.lower():
                return cmd.split()[0]
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
    return None


def launch_gazebo_world(gazebo_cmd):
    """Launch Gazebo with an empty world."""
    print(f"[Gazebo] Launching with command: {gazebo_cmd}")
    proc = subprocess.Popen(
        [gazebo_cmd, "empty.sdf"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    time.sleep(3)
    return proc


def spawn_robot(urdf_path, robot_name="dogzilla_s2"):
    """Spawn the robot URDF in Gazebo."""
    if not os.path.exists(urdf_path):
        print(f"[WARN] URDF not found at {urdf_path}")
        print("[WARN] Skipping robot spawn. Gazebo will run with empty world.")
        print("[WARN] Download DOGZILLA S2 URDF from Yahboom's GitHub and pass via --urdf")
        return None

    print(f"[Gazebo] Spawning {robot_name} from {urdf_path}")
    proc = subprocess.Popen(
        [
            "ros2", "run", "ros_gz_sim", "create",
            "-name", robot_name,
            "-file", urdf_path,
            "-x", "0", "-y", "0", "-z", "0.5",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    time.sleep(2)
    return proc


def launch_bridge(robot_id, simulate, port, robot_type):
    """Launch the JunoClaw ROS2 bridge."""
    cmd = [
        sys.executable, "-m", "junoclaw_ros2_bridge.main",
        "--robot-id", robot_id,
        "--port", str(port),
        "--robot-type", robot_type,
    ]
    if simulate:
        cmd.append("--simulate")

    print(f"[Bridge] Launching: {' '.join(cmd)}")
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    time.sleep(2)
    return proc


def main():
    parser = argparse.ArgumentParser(
        description="Launch Gazebo + DOGZILLA S2 + JunoClaw ROS2 bridge"
    )
    parser.add_argument(
        "--robot-id",
        default="dogzilla-s2-001",
        help="Robot identifier (default: dogzilla-s2-001)",
    )
    parser.add_argument(
        "--urdf",
        default="",
        help="Path to DOGZILLA S2 URDF/XACRO file",
    )
    parser.add_argument(
        "--simulate",
        action="store_true",
        help="Run bridge in simulation mode (no ROS2 required for bridge)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8080,
        help="Bridge HTTP port (default: 8080)",
    )
    parser.add_argument(
        "--robot-type",
        default="quadruped",
        choices=["wheeled", "quadruped"],
        help="Robot type (default: quadruped)",
    )
    parser.add_argument(
        "--skip-gazebo",
        action="store_true",
        help="Skip Gazebo launch (only start the bridge)",
    )

    args = parser.parse_args()

    procs = []

    try:
        if not args.skip_gazebo:
            if not check_ros2():
                print("[ERROR] ROS2 not found. Install ROS2 Humble or use --skip-gazebo")
                sys.exit(1)

            gazebo_cmd = check_gazebo()
            if not gazebo_cmd:
                print("[ERROR] Gazebo not found. Install Gazebo or use --skip-gazebo")
                sys.exit(1)

            gz_proc = launch_gazebo_world(gazebo_cmd)
            procs.append(gz_proc)

            if args.urdf:
                spawn_proc = spawn_robot(args.urdf)
                if spawn_proc:
                    procs.append(spawn_proc)
            else:
                print("[INFO] No URDF provided. Gazebo running with empty world.")
                print("[INFO] Bridge will run in simulate mode with quadruped data.")
                args.simulate = True

        bridge_proc = launch_bridge(
            robot_id=args.robot_id,
            simulate=args.simulate,
            port=args.port,
            robot_type=args.robot_type,
        )
        procs.append(bridge_proc)

        print("\n" + "=" * 60)
        print("JunoClaw DOGZILLA Simulation Ready!")
        print("=" * 60)
        print(f"  Bridge:    http://localhost:{args.port}")
        print(f"  Robot ID:  {args.robot_id}")
        print(f"  Type:      {args.robot_type}")
        print(f"  Mode:      {'SIMULATE' if args.simulate else 'GAZEBO+ROS2'}")
        print()
        print("Test endpoints:")
        print(f"  curl http://localhost:{args.port}/health")
        print(f"  curl -X POST http://localhost:{args.port}/intent/simulate?action=gait_trot&target_x=3.0&target_y=2.0")
        print(f"  curl -X POST http://localhost:{args.port}/rosbag/simulate?cycle_count=1000&violate=false")
        print(f"  curl -X POST http://localhost:{args.port}/rosbag/simulate?cycle_count=1000&violate=true")
        print()
        print("Press Ctrl+C to stop all processes.")
        print("=" * 60)

        while True:
            time.sleep(1)
            for proc in procs:
                if proc.poll() is not None:
                    print(f"[WARN] Process {proc.pid} exited with code {proc.returncode}")

    except KeyboardInterrupt:
        print("\n[Shutdown] Stopping all processes...")
    finally:
        for proc in procs:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
        print("[Shutdown] Done.")


if __name__ == "__main__":
    main()
