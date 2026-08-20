# JunoClaw Gazebo Simulation Demo

End-to-end demo: a warehouse robot navigates in Gazebo, generates ZK safety proofs, submits them on-chain, and the circuit breaker trips on a safety violation.

## What This Demo Shows

1. A robot navigates a warehouse in Gazebo
2. The ROS2 bridge captures sensor data and action results
3. The prover daemon generates ZK safety proofs (187ms)
4. Proofs are submitted to the zk-verifier contract on-chain
5. When the robot violates its safety envelope (e.g., exceeds max speed), the circuit breaker trips
6. The robot's intent-tier is locked — no new navigation commands accepted

## Prerequisites

- ROS2 Humble or Iron (with Gazebo)
- Python 3.9+
- Rust 1.78+
- Docker (for local chain)

## Quick Start

```bash
# 1. Start a local Juno chain (with zk-verifier contract)
cd simulations/gazebo-warehouse
docker compose up -d junod

# 2. Deploy contracts
./deploy_contracts.sh

# 3. Start the ROS2 bridge (simulation mode — no Gazebo needed for basic demo)
python -m junoclaw_ros2_bridge.main --robot-id warehouse-bot-01 --simulate --port 8080 &

# 4. Start the prover daemon
cargo run --manifest-path ../../prover-daemon/Cargo.toml -- run \
  --config prover-config.toml --interval 5

# 5. Run the Gazebo simulation (optional — requires ROS2 + Gazebo)
ros2 launch junoclaw_gazebo warehouse.launch.py

# 6. Trigger a safety violation
curl -X POST http://localhost:8080/rosbag/simulate?cycle_count=100\&violate=true
```

## Demo Script (No Gazebo Required)

The `demo.sh` script runs the full loop without Gazebo — using the bridge's simulation mode:

```bash
./demo.sh
```

This script:
1. Starts the ROS2 bridge in simulation mode
2. Starts the prover daemon
3. Generates a safe batch → proof → on-chain submit
4. Generates a violating batch → proof → circuit breaker trip
5. Shows the robot is locked
6. Resets the breaker via governance

## Gazebo World

The `worlds/warehouse.sdf` file defines a warehouse environment with:
- Shelves and obstacles
- A TurtleBot3 (or custom URDF)
- Sensor plugins: LiDAR, IMU, camera
- A navigation action server

## Robot URDF

The `robots/warehouse_bot.urdf` defines a differential-drive robot with:
- LiDAR sensor (/scan topic)
- IMU sensor (/imu topic)
- Camera (/camera/image_raw topic)
- cmd_vel subscriber (/cmd_vel topic)

## Safety Envelope

Default safety envelope for the demo:

| Parameter | Value |
|-----------|-------|
| max_speed | 2.0 m/s |
| max_force | 50 N |
| min_collision_distance | 0.5 m |
| max_tilt | 15 degrees |
| max_acceleration | 3.0 m/s² |

## Files

```
simulations/gazebo-warehouse/
├── demo.sh                    # Full demo script (no Gazebo required)
├── deploy_contracts.sh        # Deploy zk-verifier + circuit-breaker contracts
├── prover-config.toml         # Prover daemon config for demo
├── docker-compose.yml         # Local Juno chain + bridge
├── worlds/
│   └── warehouse.sdf          # Gazebo world definition
├── robots/
│   └── warehouse_bot.urdf     # Robot URDF
├── launch/
│   └── warehouse.launch.py    # ROS2 launch file
└── scripts/
    ├── simulate_safe.py       # Generate safe sensor batches
    ├── simulate_violation.py  # Generate violating sensor batches
    └── check_breaker.py       # Check circuit breaker state
```
