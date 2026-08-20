# JunoClaw ROS2 Bridge

HTTP bridge that exposes ROS2 action server results and sensor data to the JunoClaw `plugin-ros2` adapter.

## Architecture

```
ROS2 Robot (rclpy/rclcpp)
    ↓ action server results, sensor topics
ROS2 Bridge (FastAPI + rclpy)
    ↓ HTTP JSON
plugin-ros2 (Rust, on robot or edge)
    ↓ IntentMessage + ReflexBatchAttestation
JunoClaw Gate → Consensus → Truth Market → Settlement
```

The bridge runs on the robot (or on an edge device connected to the robot's ROS2 network). It exposes three HTTP endpoints that `plugin-ros2` calls:

- `GET /intent/{intent_id}` — fetch an action server result as `IntentMessage` JSON
- `GET /rosbag/{batch_id}` — fetch sensor log batch for Merkle tree construction
- `GET /health` — bridge health check

## Endpoints

### `GET /intent/{intent_id}`

Returns the result of a ROS2 action server goal, formatted as `IntentMessage` fields:

```json
{
  "robot_id": "warehouse-bot-01",
  "action": "navigate",
  "params": {"target_x": 12.5, "target_y": 8.3},
  "sensor_snapshot": "base64-encoded sensor data at decision time",
  "controller_timestamp": 1724073600000,
  "rationale": "route to loading bay 3",
  "execution_proof_ref": "rosbag2_2026_08_19/batch_42"
}
```

### `GET /rosbag/{batch_id}`

Returns a batch of reflex cycle hashes for Merkle tree construction:

```json
{
  "robot_id": "warehouse-bot-01",
  "batch_id": "batch_42",
  "cycles": [
    {
      "cycle_id": 0,
      "timestamp": 1724073600000,
      "sensor_readings": {"speed": 1.2, "distance": 3.5, "tilt": 2.1},
      "invariant_checks": {"max_speed": true, "min_collision_distance": true},
      "control_outputs": {"left_motor": 0.8, "right_motor": 0.8},
      "cycle_hash": "sha256..."
    }
  ],
  "merkle_root": "sha256...",
  "cycle_count": 1000,
  "all_invariants_maintained": true,
  "violated_invariants": []
}
```

### `GET /health`

```json
{
  "status": "ok",
  "robot_id": "warehouse-bot-01",
  "ros2_connected": true,
  "action_servers": ["navigate", "pick_object", "place_object"],
  "uptime_seconds": 3600
}
```

## Installation

```bash
# On the robot (or edge device with ROS2 access)
pip install junoclaw-ros2-bridge

# Or from source
cd plugins/plugin-ros2/bridge
pip install -e .
```

## Usage

```bash
# Start the bridge (connects to ROS2 network)
junoclaw-ros2-bridge \
  --robot-id warehouse-bot-01 \
  --ros2-domain 0 \
  --port 8080

# The bridge will:
# 1. Connect to the ROS2 network
# 2. Subscribe to sensor topics
# 3. Listen for action server results
# 4. Expose HTTP endpoints for plugin-ros2
```

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `JUNOCLAW_ROBOT_ID` | `robot-01` | Unique robot identifier |
| `JUNOCLAW_ROS2_DOMAIN` | `0` | ROS2 domain ID |
| `JUNOCLAW_BRIDGE_PORT` | `8080` | HTTP server port |
| `JUNOCLAW_SENSOR_TOPICS` | `/cmd_vel,/scan,/imu` | Comma-separated sensor topics to subscribe |
| `JUNOCLAW_ACTION_SERVERS` | `navigate,pick_object` | Comma-separated action servers to monitor |

## Without ROS2 (Simulation Mode)

The bridge can run without a real ROS2 installation using `--simulate`:

```bash
junoclaw-ros2-bridge --robot-id sim-bot-01 --simulate --port 8080
```

This generates fake sensor data and action results — useful for testing `plugin-ros2` without a robot.
