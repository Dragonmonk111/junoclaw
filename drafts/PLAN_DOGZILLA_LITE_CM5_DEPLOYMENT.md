# DOGZILLA-Lite CM5 — ROS2 Humble Deployment & Validation Plan

*Plan for bringing the JunoClaw stack from simulation to the DOGZILLA-Lite CM5 robot.*

## ⚡ Arrival Quick-Start (do this first, in order)

The CM5 compute module arrived Aug 31; the full DOGZILLA unit is arriving in
the coming hours. When the box is open, work Phase 0 → Phase 4 below in
order before anything else — everything past Phase 4 needs a working bridge
first. Condensed punch-list:

1. **Unbox** (Phase 0) — check all 15 servos move freely by hand, confirm CM5
   boots, note the factory OS, **pull the battery when not actively testing**
   (main switch does not fully cut power per reviews).
2. **Mount on foam/a book** so feet can't slip or the robot can't walk off a
   table during first power-on.
3. **Base OS** (Phase 1) — SSH + tailscale up first, so the rest can be done
   remotely instead of hunched over a monitor.
4. **ROS2 Humble + bridge repo** (Phase 2) — get `plugin-ros2/bridge` built
   and importable before touching the servo bus.
5. **Before wiring the real servo driver (Phase 3): run the bridge in
   `--simulate` mode first** and open `/viewer` from a phone — confirms the
   whole software stack (bridge, viewer, skills) works before any real
   joint is commanded. This is the fastest way to catch a config problem
   with zero physical risk.
6. **Servo driver** (Phase 3) — low power, no table, map joints to
   `QUADRUPED_JOINT_NAMES` by name (this is what makes skill retargeting work
   later).
7. **First live checks** (Phase 4/6) — `/health`, one expression, 15
   `/joint_states`, trot-in-place on foam only. Do not skip straight to
   walking.

Everything below is the full detail behind each of those steps.

## Hardware on Order

- **CM5 module delivered: August 31, 2026** (ahead of original Sept 4 ETA); **full DOGZILLA-Lite unit arriving in the coming hours**
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

## Phase 4.5 — No-Install Browser Viewer

Bridge now serves a single-file live viewer at `GET /viewer` — no app install,
just open the URL from any phone or PC on the same network (or over
Tailscale). Parity with "Open Duck Mini Viewer" demos:

- Live joint + IMU telemetry via `WS /ws/state` (~10Hz)
- 15 joint teleop sliders → `POST /robot/joint_command`
- 8 expression buttons → existing `POST /robot/expression`
- Works standalone in `--simulate` mode before hardware is even wired up

```bash
# from the bridge host
python3 -m junoclaw_ros2_bridge.server --ros-args
# then on phone/PC:
http://<cm5-ip>:8000/viewer
```

In simulate mode, joint sliders directly update the reported state (no
physical robot needed) — good for a quick demo of the UI itself before
Phase 3's servo driver is wired in. Once the servo driver publishes real
`/joint_states`, the viewer reflects live hardware telemetry automatically.

---

## Phase 4.6 — Skills: Teach Once, Run Anywhere

The `/viewer` page has a Skills panel: **Start Recording**, pose or drive
the robot (sliders, or physically move real hardware once wired), **Stop &
Save**, and the demonstration is captured as a named, portable artifact —
manifest (name, description, author, joint schema, license) + a keyframe
sequence. Export downloads it as JSON; Import accepts any robot's exported
skill and reports a **retarget coverage** (what fraction of the skill's
joints exist, by name, on this robot) before playing it back.

This is implemented twice, JSON-schema-compatible:

- `crates/junoclaw-physics/src/skill.rs` — `Skill`, `SkillRecorder`,
  `retarget()` (Rust side, for sim-trained or replay-derived skills, with
  `provenance_batch_root` tying a skill to the Merkle-anchored batch it was
  captured within — the same provenance property as everything else in
  this crate)
- `plugins/plugin-ros2/bridge/.../server.py` — `POST /skills/record/start`,
  `POST /skills/record/stop`, `GET /skills`, `GET /skills/{name}/export`,
  `POST /skills/import`, `POST /skills/{name}/play` (bridge side, for
  teaching directly on hardware or via the browser viewer)

Retargeting is intentionally honest, not magic: a joint only transfers if
the importing robot has a joint with the *same name*. Two robots that share
`QUADRUPED_JOINT_NAMES` (any robot built against this stack) get full
coverage automatically; anything else gets a transparent partial-coverage
report instead of a silent wrong mapping.

**Sharing / open-source distribution:** a skill is just JSON, so the
existing Buzz relay infra already carries it — upload via the relay's
Blossom endpoint (`PUT /upload`, already NIP-98 authed, content-addressed
by sha256) and reference the blob from a Nostr event (`POST /events`) so
it's discoverable per-community. No new relay schema needed for v0. A
dedicated skill-registry / marketplace listing (gated by Truth Market, per
the TODO in `bridge.rs::register_robot`) is the natural next step once
there's a second real skill to trade.

**Safety gating is now built, two layers deep:**

- `crates/junoclaw-physics/src/skill.rs::SkillGate` checks every frame
  against the L2 `WorldModel` + L1 memory before it plays — reject if the
  predicted next state lands near a red memory. 12 tests passing.
- The bridge's `play_skill` doesn't yet embed `junoclaw-physics` in-process
  (it only talks to it over HTTP today), so it enforces a hard kinematic
  safety clamp instead — reject any single-frame joint delta over
  `MAX_JOINT_DELTA_PER_CYCLE_RAD` (0.6 rad), fail-closed, checked every
  cycle. `GET /skills/playback/status` reports exactly which frame was
  rejected and why. This is the honest interim measure until `plugin-ros2`
  wires `SkillGate` in directly.

**On-chain listing:** `GET /skills/{name}/registry_msg` and
`GET /skills/{name}/marketplace_msg` generate ready-to-sign CosmWasm
`ExecuteMsg` payloads — `registry_msg` against the already-deployed
`skill-registry` contract (real sha256 hash, real address), `marketplace_msg`
against the built-but-undeployed `marketplace` contract (honestly flagged
`marketplace_deployed: false`). Nothing is broadcast by the bridge itself —
it holds no wallet key.

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

- `crates/junoclaw-physics/src/simulator.rs` — `QuadrupedBackend` (L0 reflex)
- `crates/junoclaw-physics/src/learning.rs` — `TrustLearner`
- `crates/junoclaw-physics/src/attestation.rs` — `quadruped_preset`
- `crates/junoclaw-physics/src/memory.rs` — `MemoryIndex`, `MemoryFetch`, `RootCache` (L1 memory)
- `crates/junoclaw-physics/src/worldmodel.rs` — `WorldModel`, `evaluate_action`, `select_action` (L2 world model)
- `crates/junoclaw-physics/src/pipeline.rs` — `ReflexPipeline` (wires L2→L1→L0)
- `crates/junoclaw-physics/src/dataset.rs` — `DatasetExporter` (transition corpus export)
- `crates/junoclaw-physics/src/fleet.rs` — `FleetRegistry` (cross-fleet memory)
- `crates/junoclaw-physics/src/skill.rs` — `Skill`, `SkillRecorder`, `retarget` (teach-once, cross-embodiment transfer)
- `crates/junoclaw-physics/src/replay.rs` — deterministic replay from Merkle log
- `crates/junoclaw-physics/src/watchdog.rs` — redundant reflex path
- `crates/junoclaw-physics/src/audit.rs` — audit bundle export
- `plugins/plugin-ros2/src/lib.rs` — ROS2 plugin
- `plugins/plugin-ros2/bridge/src/junoclaw_ros2_bridge/server.py` — bridge endpoints
- `plugins/plugin-ros2/bridge/tests/test_bridge.py` — bridge test cases

---

*Status: All software built and tested in simulation. L0 (QuadrupedBackend), L1 (MemoryFetch), L2 (WorldModel), ReflexPipeline, DatasetExporter, FleetRegistry, Replay, Watchdog, Audit, Skill (teach/retarget/export/import), SkillGate (L2+L1 gated playback) — all compiled and tested (163 tests in junoclaw-physics, 80/80 coordination tests). ROS2 bridge extended with a no-install browser viewer (`/viewer`: live telemetry, joint teleop, skill record/play/import/export), a fail-closed kinematic safety clamp on playback, and registry/marketplace message generation (28/28 bridge tests). Buzz relay live on Akash for fleet sync. DOGZILLA-Lite CM5 module delivered Aug 31, 2026; full unit arriving in the coming hours — Phase 0 (unbox & inspect) starts on arrival.*
