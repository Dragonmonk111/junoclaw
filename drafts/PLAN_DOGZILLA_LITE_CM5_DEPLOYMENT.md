# DOGZILLA-Lite CM5 — ROS2 Humble Deployment & Validation Plan

*Plan for bringing the JunoClaw stack from simulation to the DOGZILLA-Lite CM5 robot.*

## Hardware on Order

- **Delivery ETA: 4th September 2026**
- **DOGZILLA-Lite with Raspberry Pi CM5** (~$639)
- 15 DOF: 12 leg + 3 arm (arm + gripper)
- IPS face display
- IMU, foot contact, camera (low-res, not critical for JunoClaw v0)
- Known fragility: aluminum leg joints, avoid falls from > 30 cm

## Goal

Close the sim-to-real loop: run the same `QuadrupedBackend` / `SafetyEnvelope` / `TrustLearner` / ROS2 bridge that passed 80/80 tests in simulation on real hardware.

---

## Phase 0 — Unbox & Inspect

1. Verify all 15 servos are intact
2. Verify CM5 module boots from included SD/eMMC
3. Note factory OS — likely Raspberry Pi OS or Ubuntu
4. Test main switch / battery behavior (reviewers noted it does not fully cut power; remove battery for long-term storage)
5. Mount robot on a foam pad or book to prevent feet from slipping during first tests

---

## Phase 1 — Base CM5 Setup

1. Connect CM5 to monitor, keyboard, ethernet/Wi-Fi
2. Update system:
   ```bash
   sudo apt update && sudo apt upgrade -y
   ```
3. Install base tools: `git`, `curl`, `build-essential`, `python3-pip`, `vim`
4. Enable SSH and tailscale/wireguard for remote access

---

## Phase 2 — ROS2 Humble

The factory software may use Python micro-ROS or a custom stack. JunoClaw needs standard ROS2 Humble.

1. Install ROS2 Humble per official docs:
   ```bash
   # Add ROS2 apt source
   sudo apt install -y ros-humble-ros-base ros-humble-ros2launch
   echo "source /opt/ros/humble/setup.bash" >> ~/.bashrc
   source ~/.bashrc
   ```
2. Install `colcon` build tool:
   ```bash
   sudo apt install -y python3-colcon-common-extensions
   ```
3. Create workspace:
   ```bash
   mkdir -p ~/ros2_ws/src
   cd ~/ros2_ws
   ```
4. Clone or copy the JunoClaw `plugin-ros2/bridge` into `src/junoclaw_ros2_bridge`
5. Build bridge:
   ```bash
   colcon build --packages-select junoclaw_ros2_bridge
   source install/setup.bash
   ```
6. Verify with:
   ```bash
   ros2 pkg list | grep junoclaw
   ```

---

## Phase 3 — DOGZILLA Servo Driver

The robot likely uses FE-URT-1 or similar 12-bit servos with a UART/485 bus.

1. Identify servo bus: `/dev/ttyUSB0` or `/dev/ttyAMA0` or GPIO UART
2. Install or write minimal servo driver node:
   - Read positions
   - Write goal positions
   - Read torque/load
3. Map 15 joints to names in `QUADRUPED_JOINT_NAMES`:
   ```
   fl_hip, fl_thigh, fl_calf, fr_hip, fr_thigh, fr_calf,
   bl_hip, bl_thigh, bl_calf, br_hip, br_thigh, br_calf,
   arm_base, arm_elbow, arm_gripper
   ```
4. Publish `/joint_states` and subscribe to `/joint_commands`

**Do not test high-torque moves on a table.** Keep power low for first moves.

---

## Phase 4 — Bridge Integration

1. Run the FastAPI bridge:
   ```bash
   cd ~/ros2_ws/src/junoclaw_ros2_bridge
   python3 -m pip install -r requirements.txt
   python3 -m junoclaw_ros2_bridge.server --ros-args
   ```
2. In another shell, test endpoints:
   ```bash
   curl http://localhost:8000/health
   curl http://localhost:8000/robot/expression -X POST -H "Content-Type: application/json" \
        -d '{"expression": "happy"}'
   ```
3. Verify the expression appears on the face screen
4. Test `/rosbag/simulate` with a short batch and check `all_invariants_maintained`

---

## Phase 5 — JunoClaw Plugin-ros2 on CM5

1. On a build host or on the CM5 itself, compile `plugin-ros2` Rust crate:
   ```bash
   cd junoclaw/plugins/plugin-ros2
   cargo build --release
   ```
   *Note: CM5 may be slow to compile. Cross-compile from x86 or use build server.*
2. Copy binary to CM5
3. Configure `robot_type = "quadruped"` in the plugin config
4. Point the plugin to the bridge URL:
   ```
   bridge_url = "http://localhost:8000"
   ```
5. Start the plugin and verify it queries the chain for the safety envelope

---

## Phase 6 — First Real-World Validation

Run these tests in order, with robot on soft foam, no walking initially:

1. **Expression mapping** — green/yellow/red verdict → happy/alert/angry face
2. **Joint state 15-DOF** — `TrustLearner`/physics crate reads `/joint_states` and sees 15 joints
3. **IMU** — `/imu` produces roll/pitch/yaw; compare with simulation
4. **Trot in place** — small, low-torque gait, no forward motion
5. **Forward crawl** — 10 cm slow forward, watch tilt < 15°
6. **Arm raise** — move arm to 3 positions, check `max_arm_force` logic
7. **Gripper open/close** — map to `arm_gripper` joint

For each test, run a short `BatchConfig::quadruped_preset` and verify `all_invariants_maintained`.

---

## Phase 7 — RL-TF Loop on Hardware

1. Feed one synthetic or real verdict to `TrustLearner` on the robot
2. Observe `AdjustedEnvelope` printed on the trust dashboard
3. Verify the middleware does not command a tighter envelope to exceed DAO limits
4. Trigger one yellow `max_tilt` and watch the gait speed reduce

---

## Phase 8 — Truth Market Integration

1. On a testnet or mainnet, submit one reflex batch attestation
2. Have one operator (your own Buzz agent or local) submit a verdict
3. Bridge consumes the verdict via on-chain query or Buzz relay
4. Robot's face and `TrustLearner` update accordingly

This is the final integration: the real robot learning from a real truth market.

---

## Safety Constraints for Lite CM5

Given the fragility reported in reviews:

| Constraint | Value | Reason |
|---|---|---|
| `max_tilt_degrees` | 15° first, later 25° | Avoid joint stress / falls |
| `max_speed` | 0.3 m/s first | Do not overdrive small servos |
| `max_arm_force` | 2 N first | Gripper may not hold loads |
| Test surface | Foam mat on floor | Drop energy absorption |
| Battery | Remove when not in use | Main switch does not isolate |
| Attitude during first gait | Belly on foam | Reduce effective drop height |

---

## Pass Criteria

- ✅ Bridge health responds
- ✅ Face shows expressions
- ✅ 15 joint states published
- ✅ Trot in place with `all_invariants_maintained`
- ✅ One yellow verdict triggers `TrustLearner` tightening
- ✅ One reflex batch attestation anchored on testnet

---

## Files to Watch

- `crates/junoclaw-physics/src/simulator.rs` — `QuadrupedBackend`
- `crates/junoclaw-physics/src/learning.rs` — `TrustLearner`
- `crates/junoclaw-physics/src/attestation.rs` — `quadruped_preset`
- `plugins/plugin-ros2/src/lib.rs` — ROS2 plugin
- `plugins/plugin-ros2/bridge/src/junoclaw_ros2_bridge/server.py` — bridge endpoints
- `plugins/plugin-ros2/bridge/tests/test_bridge.py` — bridge test cases

---

*Status: Plan ready. Awaits DOGZILLA-Lite CM5 delivery.*
