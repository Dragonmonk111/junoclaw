"""Bridge server — FastAPI app exposing ROS2 data to JunoClaw plugin-ros2."""

import asyncio
import hashlib
import json
import time
import uuid
from typing import Any, Optional

from pydantic import BaseModel, Field

try:
    from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
    from fastapi.responses import HTMLResponse, JSONResponse
except ImportError:
    raise ImportError(
        "FastAPI not installed. Run: pip install junoclaw-ros2-bridge"
    )


class IntentResult(BaseModel):
    robot_id: str
    action: str
    params: dict[str, Any] = Field(default_factory=dict)
    sensor_snapshot: str = ""
    controller_timestamp: int
    rationale: Optional[str] = None
    execution_proof_ref: Optional[str] = None


class CycleData(BaseModel):
    cycle_id: int
    timestamp: int
    sensor_readings: dict[str, float]
    invariant_checks: dict[str, bool]
    control_outputs: dict[str, float]
    cycle_hash: str


class BatchResult(BaseModel):
    robot_id: str
    batch_id: str
    cycles: list[CycleData]
    merkle_root: str
    cycle_count: int
    all_invariants_maintained: bool
    violated_invariants: list[str] = Field(default_factory=list)


class HealthResponse(BaseModel):
    status: str
    robot_id: str
    ros2_connected: bool
    action_servers: list[str]
    uptime_seconds: int


class PolicyRuntime:
    """Thin wrapper around an onnxruntime InferenceSession for a sim2real
    RL policy exported by sim2real/export_onnx.py. Holds just enough state
    (the session + input tensor name) to run one inference step per call;
    all safety gating lives in Ros2Bridge.start_policy, not here.
    """

    def __init__(self, session: Any, input_name: str, onnx_path: str):
        self.session = session
        self.input_name = input_name
        self.onnx_path = onnx_path

    def infer(self, obs: list[float]) -> list[float]:
        import numpy as np

        obs_arr = np.asarray([obs], dtype=np.float32)
        outputs = self.session.run(None, {self.input_name: obs_arr})
        return outputs[0][0].tolist()


class Ros2Bridge:
    """Core bridge logic — works with or without a real ROS2 installation."""

    # Quadruped joint names for DOGZILLA-Lite (15 DOF: 12 leg + 3 arm)
    QUADRUPED_JOINTS = [
        "fl_hip", "fl_thigh", "fl_calf",
        "fr_hip", "fr_thigh", "fr_calf",
        "rl_hip", "rl_thigh", "rl_calf",
        "rr_hip", "rr_thigh", "rr_calf",
        "arm_base", "arm_shoulder", "arm_gripper",
    ]

    def __init__(
        self,
        robot_id: str,
        simulate: bool = False,
        ros2_domain: int = 0,
        sensor_topics: list[str] | None = None,
        action_servers: list[str] | None = None,
        robot_type: str = "wheeled",
    ):
        self.robot_id = robot_id
        self.simulate = simulate
        self.ros2_domain = ros2_domain
        self.robot_type = robot_type
        if sensor_topics is not None:
            self.sensor_topics = sensor_topics
        elif robot_type == "quadruped":
            self.sensor_topics = ["/cmd_vel", "/scan", "/imu/data", "/joint_states"]
        else:
            self.sensor_topics = ["/cmd_vel", "/scan", "/imu"]
        self.action_servers = action_servers or ["navigate", "stand", "sit", "gait_trot"]
        self.start_time = time.time()
        self._ros2_node = None
        self._intent_store: dict[str, IntentResult] = {}
        self._batch_store: dict[str, BatchResult] = {}
        self._latest_joint_states: dict[str, float] = {}
        self._latest_joint_velocities: dict[str, float] = {}
        self._sim_joint_cmd_ts: dict[str, float] = {}
        self._latest_imu: dict[str, float] = {}
        self._latest_expression: str = "neutral"
        # Phase E — ONNX policy runtime (see PolicyRuntime below).
        self._policy: Optional["PolicyRuntime"] = None
        self._policy_task: Optional[asyncio.Task] = None
        self._last_policy_status: dict[str, Any] = {"status": "idle"}
        # Taught skills — JSON-schema-compatible with the Rust `Skill` type
        # in junoclaw-physics/src/skill.rs (manifest + keyframes), so a
        # skill exported here can be imported there and vice versa.
        self.skills: dict[str, dict[str, Any]] = {}
        self._record_task: Optional[asyncio.Task] = None
        self._record_buffer: list[list[float]] = []
        self._record_joint_names: list[str] = []
        self._play_task: Optional[asyncio.Task] = None
        self._last_playback: dict[str, Any] = {"status": "idle"}

        if not simulate:
            self._init_ros2()

    def _init_ros2(self):
        """Initialize rclpy node and subscribe to topics."""
        try:
            import rclpy
            from rclpy.node import Node
            from std_msgs.msg import String
            from geometry_msgs.msg import Twist
            from sensor_msgs.msg import LaserScan, Imu

            rclpy.init()
            self._ros2_node = Node("junoclaw_bridge")

            from sensor_msgs.msg import JointState

            for topic in self.sensor_topics:
                if topic == "/cmd_vel":
                    self._ros2_node.create_subscription(
                        Twist, topic, self._on_cmd_vel, 10
                    )
                elif topic == "/scan":
                    self._ros2_node.create_subscription(
                        LaserScan, topic, self._on_scan, 10
                    )
                elif topic in ("/imu", "/imu/data"):
                    self._ros2_node.create_subscription(
                        Imu, topic, self._on_imu, 10
                    )
                elif topic == "/joint_states":
                    self._ros2_node.create_subscription(
                        JointState, topic, self._on_joint_states, 10
                    )

            self._ros2_node.get_logger().info(
                f"JunoClaw bridge connected to ROS2 domain {self.ros2_domain}, "
                f"subscribed to {len(self.sensor_topics)} topics"
            )
        except ImportError:
            raise RuntimeError(
                "rclpy not available. Install ROS2 or run with --simulate flag."
            )

    def _on_cmd_vel(self, msg):
        """Callback for /cmd_vel topic."""
        pass

    def _on_scan(self, msg):
        """Callback for /scan topic."""
        pass

    def _on_imu(self, msg):
        """Callback for /imu or /imu/data topic."""
        self._latest_imu = {
            "accel_x": msg.linear_acceleration.x,
            "accel_y": msg.linear_acceleration.y,
            "accel_z": msg.linear_acceleration.z,
            "gyro_x": msg.angular_velocity.x,
            "gyro_y": msg.angular_velocity.y,
            "gyro_z": msg.angular_velocity.z,
            "orient_w": msg.orientation.w,
            "orient_x": msg.orientation.x,
            "orient_y": msg.orientation.y,
            "orient_z": msg.orientation.z,
        }

    def _on_joint_states(self, msg):
        """Callback for /joint_states topic (quadruped)."""
        for name, pos, vel, eff in zip(msg.name, msg.position, msg.velocity, msg.effort):
            self._latest_joint_states[name] = pos
            self._latest_joint_velocities[name] = vel

    def store_intent(self, intent: IntentResult):
        """Store an intent result from an action server callback."""
        intent_id = str(uuid.uuid4())
        self._intent_store[intent_id] = intent
        return intent_id

    def get_intent(self, intent_id: str) -> IntentResult:
        if intent_id not in self._intent_store:
            raise HTTPException(status_code=404, detail=f"intent {intent_id} not found")
        return self._intent_store[intent_id]

    def store_batch(self, batch: BatchResult):
        """Store a reflex batch result."""
        self._batch_store[batch.batch_id] = batch

    def get_batch(self, batch_id: str) -> BatchResult:
        if batch_id not in self._batch_store:
            raise HTTPException(status_code=404, detail=f"batch {batch_id} not found")
        return self._batch_store[batch_id]

    def set_joint_command(self, joint: str, position: float) -> None:
        """Command a single joint to a target position (radians).

        In ROS2 mode, publishes to /joint_commands. In simulate mode
        (no CM5 attached yet, or bridge running standalone for the viewer),
        directly updates the last-known state so the browser viewer reflects
        the command immediately — useful for pre-hardware demo/UI testing.
        """
        if joint not in self.QUADRUPED_JOINTS:
            raise HTTPException(
                status_code=400,
                detail=f"unknown joint '{joint}', expected one of {self.QUADRUPED_JOINTS}",
            )
        if self._ros2_node:
            try:
                from sensor_msgs.msg import JointState

                pub = self._ros2_node.create_publisher(JointState, "/joint_commands", 10)
                msg = JointState()
                msg.name = [joint]
                msg.position = [position]
                pub.publish(msg)
            except Exception as e:
                print(f"[JointCommand] Failed to publish: {e}")
        else:
            # Simulate mode has no real /joint_states publisher, so derive a
            # finite-difference velocity estimate from the position delta —
            # used only as an observation-space stand-in for policy inference
            # (see PolicyRuntime._build_observation); real hardware fills
            # _latest_joint_velocities from _on_joint_states instead.
            now = time.monotonic()
            prev_pos = self._latest_joint_states.get(joint, position)
            prev_t = self._sim_joint_cmd_ts.get(joint, now)
            dt = max(now - prev_t, 1e-3)
            self._latest_joint_velocities[joint] = (position - prev_pos) / dt
            self._sim_joint_cmd_ts[joint] = now
        self._latest_joint_states[joint] = position

    def state_snapshot(self) -> dict[str, Any]:
        """Current joints + IMU + expression, for the WS live viewer."""
        joints = {
            jn: self._latest_joint_states.get(jn, 0.0) for jn in self.QUADRUPED_JOINTS
        }
        return {
            "ts": int(time.time() * 1000),
            "joints": joints,
            "imu": self._latest_imu,
            "expression": self._latest_expression,
            "ros2_connected": not self.simulate,
        }

    # -----------------------------------------------------------------------
    # Skills — teach in sim (or by posing real hardware), export as a
    # portable artifact, retarget by joint name onto any other robot, play
    # back. Schema matches junoclaw-physics/src/skill.rs so a skill taught
    # here can be consumed by the Rust stack and vice versa.
    # -----------------------------------------------------------------------

    def start_recording(self, cycle_dt_ms: int = 100) -> None:
        if self._record_task is not None:
            raise HTTPException(status_code=409, detail="a recording is already in progress")
        self._record_joint_names = list(self.QUADRUPED_JOINTS)
        self._record_buffer = []
        self._record_task = asyncio.create_task(self._record_loop(cycle_dt_ms))

    async def _record_loop(self, cycle_dt_ms: int) -> None:
        try:
            while True:
                row = [self._latest_joint_states.get(jn, 0.0) for jn in self._record_joint_names]
                self._record_buffer.append(row)
                await asyncio.sleep(cycle_dt_ms / 1000.0)
        except asyncio.CancelledError:
            pass

    def stop_recording(self, name: str, description: str, license_: str, cycle_dt_ms: int = 100) -> dict[str, Any]:
        if self._record_task is None:
            raise HTTPException(status_code=409, detail="no recording in progress")
        self._record_task.cancel()
        self._record_task = None

        skill = {
            "manifest": {
                "name": name,
                "description": description,
                "author_robot_id": self.robot_id,
                "joint_names": self._record_joint_names,
                "frame_count": len(self._record_buffer),
                "cycle_dt_ms": cycle_dt_ms,
                "license": license_,
                "provenance_batch_root": "",
                "created_at_ms": int(time.time() * 1000),
            },
            "keyframes": self._record_buffer,
        }
        self.skills[name] = skill
        self._record_buffer = []
        self._record_joint_names = []
        return skill

    @staticmethod
    def retarget_skill(skill: dict[str, Any], target_joint_names: list[str]) -> tuple[dict[str, Any], dict[str, Any]]:
        """Best-effort, name-based retarget onto a different joint schema.

        Mirrors `Skill::retarget` in skill.rs: only joints present on both
        sides transfer. Returns (retargeted_skill, coverage_report) so a
        partial match is legible rather than silently wrong.
        """
        manifest = skill["manifest"]
        source_joints: list[str] = manifest["joint_names"]
        keyframes: list[list[float]] = skill["keyframes"]
        target_set = set(target_joint_names)

        matched_idx = [i for i, n in enumerate(source_joints) if n in target_set]
        matched_joints = [source_joints[i] for i in matched_idx]
        missing_in_target = [n for n in source_joints if n not in target_set]
        matched_set = set(matched_joints)
        unused_target_joints = [n for n in target_joint_names if n not in matched_set]

        retargeted_keyframes = [[row[i] for i in matched_idx] for row in keyframes]
        coverage = (len(matched_joints) / len(source_joints)) if source_joints else 0.0

        retargeted = {
            "source_manifest": manifest,
            "joint_names": matched_joints,
            "keyframes": retargeted_keyframes,
        }
        report = {
            "matched_joints": matched_joints,
            "missing_in_target": missing_in_target,
            "unused_target_joints": unused_target_joints,
            "coverage": coverage,
        }
        return retargeted, report

    # Hard kinematic safety clamp for live skill playback: reject any frame
    # that commands a joint further than this many radians from its current
    # position in one cycle. This is deliberately NOT the L2 world-model gate
    # — that check (predict the consequence, reject if it lands near a red
    # memory) is implemented and tested in
    # `crates/junoclaw-physics/src/skill.rs::SkillGate`, but `plugin-ros2`
    # (the Rust adapter that actually drives real hardware) does not yet
    # depend on `junoclaw-physics` in-process — today it only talks to this
    # bridge over HTTP, so there is no live `WorldModel`/`MemoryFetch` here
    # to consult. This clamp is the honest interim measure: a hard bound on
    # how far any single joint may move per cycle during playback, enforced
    # every frame, fail-closed (abort, don't clip-and-continue).
    MAX_JOINT_DELTA_PER_CYCLE_RAD = 0.6

    def play_skill(self, name: str) -> dict[str, Any]:
        """Start open-loop playback of a taught skill against this robot's
        own joint schema (retargeted if the skill came from elsewhere).
        Non-blocking — returns immediately; playback runs in the background.

        Every frame is checked against `MAX_JOINT_DELTA_PER_CYCLE_RAD`
        before it is commanded. On the first frame that exceeds the clamp,
        playback aborts immediately (fail-closed) rather than commanding a
        large, unvalidated jump. See `self._last_playback` for the outcome.
        """
        if name not in self.skills:
            raise HTTPException(status_code=404, detail=f"skill '{name}' not found")
        if self._play_task is not None and not self._play_task.done():
            raise HTTPException(status_code=409, detail="a skill is already playing")

        skill = self.skills[name]
        retargeted, report = self.retarget_skill(skill, self.QUADRUPED_JOINTS)
        cycle_dt_ms = skill["manifest"].get("cycle_dt_ms", 100)

        self._last_playback = {
            "name": name,
            "status": "running",
            "frames_total": len(retargeted["keyframes"]),
            "frames_executed": 0,
            "rejected_at_frame": None,
            "reason": None,
        }

        async def _play():
            for i, row in enumerate(retargeted["keyframes"]):
                for joint, target_pos in zip(retargeted["joint_names"], row):
                    current_pos = self._latest_joint_states.get(joint, 0.0)
                    delta = abs(target_pos - current_pos)
                    if delta > self.MAX_JOINT_DELTA_PER_CYCLE_RAD:
                        self._last_playback["status"] = "rejected"
                        self._last_playback["rejected_at_frame"] = i
                        self._last_playback["reason"] = (
                            f"joint '{joint}' delta {delta:.3f} rad exceeds "
                            f"safety clamp {self.MAX_JOINT_DELTA_PER_CYCLE_RAD} rad "
                            f"(current={current_pos:.3f}, target={target_pos:.3f})"
                        )
                        return
                    self.set_joint_command(joint, target_pos)
                self._last_playback["frames_executed"] = i + 1
                await asyncio.sleep(cycle_dt_ms / 1000.0)
            self._last_playback["status"] = "completed"

        self._play_task = asyncio.create_task(_play())
        return {
            "status": "playing",
            "name": name,
            "frames": len(retargeted["keyframes"]),
            "retarget_report": report,
            "safety_clamp_rad": self.MAX_JOINT_DELTA_PER_CYCLE_RAD,
        }

    # -----------------------------------------------------------------------
    # Phase E — sim2real RL policy runtime. Loads an ONNX policy exported by
    # sim2real/export_onnx.py (obs -> action, see sim2real/env.py::_get_obs
    # and _action_to_joint_targets for the exact layout/scaling this mirrors)
    # and runs closed-loop inference against live telemetry.
    #
    # Gating: every inferred joint target is checked against the *same*
    # MAX_JOINT_DELTA_PER_CYCLE_RAD fail-closed clamp used by play_skill,
    # for the same reason documented above it — plugin-ros2 does not yet
    # depend on junoclaw-physics in-process, so this bridge has no live
    # WorldModel/SkillGate to consult. A policy step that would move any
    # joint further than the clamp allows is rejected outright (the policy
    # loop stops rather than commanding an unvalidated jump).
    # -----------------------------------------------------------------------

    # Joint ranges (radians), mirrored from sim2real/models/dogzilla_lite.xml
    # <joint range="..."> attributes — must match exactly, since the policy
    # was trained with actions scaled against these bounds
    # (env.py::_action_to_joint_targets: mid + action * half).
    JOINT_RANGES_RAD: dict[str, tuple[float, float]] = {
        "fl_hip": (-0.6, 0.6), "fl_thigh": (-1.57, 1.57), "fl_calf": (-2.2, 0.2),
        "fr_hip": (-0.6, 0.6), "fr_thigh": (-1.57, 1.57), "fr_calf": (-2.2, 0.2),
        "rl_hip": (-0.6, 0.6), "rl_thigh": (-1.57, 1.57), "rl_calf": (-2.2, 0.2),
        "rr_hip": (-0.6, 0.6), "rr_thigh": (-1.57, 1.57), "rr_calf": (-2.2, 0.2),
        "arm_base": (-1.57, 1.57), "arm_shoulder": (-1.57, 1.57), "arm_gripper": (0.0, 1.2),
    }
    # DOGZILLA-Lite has no direct height sensor and this bridge does not run
    # a state estimator, so trunk height (one of the 41 obs dims the policy
    # was trained on) cannot be measured from live telemetry today. We feed
    # the sim's nominal standing height as a constant fallback instead of
    # guessing from IMU integration — a known, documented gap versus the
    # real observation the sim policy expects (see sim2real/README.md).
    DEFAULT_STANDING_HEIGHT_M = 0.16

    def _build_policy_observation(self) -> list[float]:
        """Assemble the 41-dim observation vector the ONNX policy expects,
        from whatever live telemetry this bridge actually has.

        Layout must match sim2real/env.py::DogzillaStandEnv._get_obs exactly:
        joint pos(15) + joint vel(15) + trunk quat wxyz(4) + trunk gyro(3)
        + trunk lin accel(3) + trunk height(1) = 41.
        """
        joint_pos = [self._latest_joint_states.get(j, 0.0) for j in self.QUADRUPED_JOINTS]
        joint_vel = [self._latest_joint_velocities.get(j, 0.0) for j in self.QUADRUPED_JOINTS]
        imu = self._latest_imu
        trunk_quat = [
            imu.get("orient_w", 1.0), imu.get("orient_x", 0.0),
            imu.get("orient_y", 0.0), imu.get("orient_z", 0.0),
        ]
        trunk_gyro = [imu.get("gyro_x", 0.0), imu.get("gyro_y", 0.0), imu.get("gyro_z", 0.0)]
        trunk_accel = [imu.get("accel_x", 0.0), imu.get("accel_y", 0.0), imu.get("accel_z", 9.81)]
        trunk_height = [self.DEFAULT_STANDING_HEIGHT_M]
        return joint_pos + joint_vel + trunk_quat + trunk_gyro + trunk_accel + trunk_height

    def load_policy(self, onnx_path: str) -> dict[str, Any]:
        """Load an ONNX policy for closed-loop inference. Does not start it —
        call start_policy() separately. Requires the `onnx` extra
        (`pip install junoclaw-ros2-bridge[onnx]`).
        """
        try:
            import onnxruntime as ort
        except ImportError:
            raise HTTPException(
                status_code=500,
                detail="onnxruntime not installed. Run: pip install junoclaw-ros2-bridge[onnx]",
            )
        if self._policy_task is not None and not self._policy_task.done():
            raise HTTPException(status_code=409, detail="a policy is already running; stop it first")
        import os
        if not os.path.exists(onnx_path):
            raise HTTPException(status_code=404, detail=f"onnx policy not found at '{onnx_path}'")

        session = ort.InferenceSession(onnx_path, providers=["CPUExecutionProvider"])
        input_meta = session.get_inputs()[0]
        expected_obs_dim = len(self._build_policy_observation())
        model_obs_dim = input_meta.shape[-1]
        if isinstance(model_obs_dim, int) and model_obs_dim != expected_obs_dim:
            raise HTTPException(
                status_code=400,
                detail=(
                    f"onnx policy expects obs dim {model_obs_dim}, "
                    f"bridge builds {expected_obs_dim} — check joint schema"
                ),
            )
        self._policy = PolicyRuntime(session=session, input_name=input_meta.name, onnx_path=onnx_path)
        self._last_policy_status = {"status": "loaded", "onnx_path": onnx_path}
        return self._last_policy_status

    def start_policy(self, hz: float = 30.0, smoothing: float = 0.2) -> dict[str, Any]:
        """Start the closed-loop inference loop against the currently
        loaded policy. Non-blocking — returns immediately.

        ``smoothing`` is an exponential-moving-average factor in [0, 1]:
        each step's target is blended as
            target = smoothing * raw_policy_target + (1 - smoothing) * prev_target
        A value of 0.2 means 20% new / 80% old — this prevents the first
        inference step from commanding a large jump that would trip the
        safety clamp, and generally smooths policy output (standard sim2real
        practice). Set to 1.0 for no smoothing (raw policy output).
        """
        if self._policy is None:
            raise HTTPException(status_code=409, detail="no policy loaded; call /robot/policy/load first")
        if self._policy_task is not None and not self._policy_task.done():
            raise HTTPException(status_code=409, detail="policy loop already running")

        self._last_policy_status = {
            "status": "running",
            "onnx_path": self._policy.onnx_path,
            "hz": hz,
            "smoothing": smoothing,
            "steps_run": 0,
            "rejected_at_step": None,
            "reason": None,
        }

        async def _loop():
            period = 1.0 / hz
            step = 0
            # Initialise smoothed targets from current joint positions so the
            # first step's delta is zero (no clamp rejection on startup).
            prev_targets: dict[str, float] = {
                j: self._latest_joint_states.get(j, 0.0) for j in self.QUADRUPED_JOINTS
            }
            try:
                while True:
                    obs = self._build_policy_observation()
                    action = self._policy.infer(obs)
                    targets = {}
                    for joint, a in zip(self.QUADRUPED_JOINTS, action):
                        a = max(-1.0, min(1.0, float(a)))
                        lo, hi = self.JOINT_RANGES_RAD[joint]
                        mid, half = (lo + hi) / 2.0, (hi - lo) / 2.0
                        raw_target = mid + a * half
                        # EMA smoothing: blend toward raw policy output
                        smoothed = smoothing * raw_target + (1.0 - smoothing) * prev_targets[joint]
                        targets[joint] = smoothed
                        prev_targets[joint] = smoothed

                    for joint, target_pos in targets.items():
                        current_pos = self._latest_joint_states.get(joint, 0.0)
                        delta = abs(target_pos - current_pos)
                        if delta > self.MAX_JOINT_DELTA_PER_CYCLE_RAD:
                            self._last_policy_status["status"] = "rejected"
                            self._last_policy_status["rejected_at_step"] = step
                            self._last_policy_status["reason"] = (
                                f"joint '{joint}' delta {delta:.3f} rad exceeds "
                                f"safety clamp {self.MAX_JOINT_DELTA_PER_CYCLE_RAD} rad "
                                f"(current={current_pos:.3f}, target={target_pos:.3f})"
                            )
                            return
                    for joint, target_pos in targets.items():
                        self.set_joint_command(joint, target_pos)

                    step += 1
                    self._last_policy_status["steps_run"] = step
                    await asyncio.sleep(period)
            except asyncio.CancelledError:
                self._last_policy_status["status"] = "stopped"

        self._policy_task = asyncio.create_task(_loop())
        return self._last_policy_status

    def stop_policy(self) -> dict[str, Any]:
        if self._policy_task is not None and not self._policy_task.done():
            self._policy_task.cancel()
        self._last_policy_status["status"] = "stopped"
        return self._last_policy_status

    def health(self) -> HealthResponse:
        return HealthResponse(
            status="ok",
            robot_id=self.robot_id,
            ros2_connected=not self.simulate,
            action_servers=self.action_servers,
            uptime_seconds=int(time.time() - self.start_time),
        )

    def generate_simulated_batch(
        self,
        cycle_count: int = 1000,
        violate: bool = False,
    ) -> BatchResult:
        """Generate a simulated reflex batch for testing without a robot."""
        cycles = []
        violated = []
        is_quadruped = self.robot_type == "quadruped"

        for i in range(cycle_count):
            ts = int(time.time() * 1000) - (cycle_count - i) * 10

            if violate and i == cycle_count // 2:
                speed = 3.5
                distance = 0.3
                tilt = 38.0
                checks = {"max_speed": False, "min_collision_distance": False, "max_tilt": False}
                violated = ["max_speed", "min_collision_distance", "max_tilt"]
            else:
                speed = 1.2 if not is_quadruped else 0.8
                distance = 3.5
                tilt = 2.1 if not is_quadruped else 12.0
                checks = {"max_speed": True, "min_collision_distance": True, "max_tilt": True}

            readings = {"speed": speed, "distance": distance, "tilt": tilt}

            if is_quadruped:
                readings["imu_accel_z"] = 9.81
                readings["imu_gyro_x"] = 0.02
                readings["imu_gyro_y"] = 0.01
                readings["num_contacts"] = 4

                outputs = {}
                for jn in self.QUADRUPED_JOINTS:
                    outputs[jn] = round(0.5 + 0.3 * ((i + hash(jn)) % 100) / 100.0, 4)
            else:
                outputs = {"left_motor": 0.8, "right_motor": 0.8}

            cycle_hash = hashlib.sha256(
                json.dumps(
                    {"i": i, "r": readings, "c": checks, "o": outputs},
                    sort_keys=True,
                ).encode()
            ).hexdigest()

            cycles.append(
                CycleData(
                    cycle_id=i,
                    timestamp=ts,
                    sensor_readings=readings,
                    invariant_checks=checks,
                    control_outputs=outputs,
                    cycle_hash=cycle_hash,
                )
            )

        merkle_root = self._compute_merkle_root([c.cycle_hash for c in cycles])

        return BatchResult(
            robot_id=self.robot_id,
            batch_id=f"batch_{int(time.time())}",
            cycles=cycles,
            merkle_root=merkle_root,
            cycle_count=cycle_count,
            all_invariants_maintained=not violate,
            violated_invariants=violated,
        )

    @staticmethod
    def _compute_merkle_root(leaf_hashes: list[str]) -> str:
        """Compute a Merkle root from a list of SHA-256 leaf hashes."""
        if not leaf_hashes:
            return hashlib.sha256(b"").hexdigest()

        level = [bytes.fromhex(h) for h in leaf_hashes]

        while len(level) > 1:
            if len(level) % 2 == 1:
                level.append(level[-1])

            next_level = []
            for i in range(0, len(level), 2):
                combined = hashlib.sha256(level[i] + level[i + 1]).digest()
                next_level.append(combined)
            level = next_level

        return level[0].hex()

    def generate_simulated_intent(
        self,
        action: str = "navigate",
        target_x: float = 12.5,
        target_y: float = 8.3,
    ) -> IntentResult:
        """Generate a simulated intent for testing without a robot."""
        is_quadruped = self.robot_type == "quadruped"

        if is_quadruped:
            sensor_data = json.dumps({
                "speed": 0.8,
                "position": {"x": 5.0, "y": 3.0},
                "obstacles": 2,
                "tilt": 12.0,
                "imu": self._latest_imu or {
                    "accel_z": 9.81, "gyro_x": 0.02, "gyro_y": 0.01,
                },
                "joints": self._latest_joint_states or {
                    jn: 0.5 for jn in self.QUADRUPED_JOINTS
                },
                "contacts": 4,
            }).encode()
        else:
            sensor_data = json.dumps(
                {"speed": 1.2, "position": {"x": 5.0, "y": 3.0}, "obstacles": 2}
            ).encode()

        import base64

        return IntentResult(
            robot_id=self.robot_id,
            action=action,
            params={"target_x": target_x, "target_y": target_y},
            sensor_snapshot=base64.b64encode(sensor_data).decode(),
            controller_timestamp=int(time.time() * 1000),
            rationale=f"simulated {action} to ({target_x}, {target_y})",
            execution_proof_ref=f"sim_batch_{int(time.time())}",
        )

    async def spin(self):
        """Spin the ROS2 node if connected."""
        if self._ros2_node:
            import rclpy

            while rclpy.ok():
                rclpy.spin_once(self._ros2_node, timeout_sec=0.1)
                await asyncio.sleep(0.01)
        else:
            while True:
                await asyncio.sleep(1.0)


def create_app(
    robot_id: str = "robot-01",
    simulate: bool = False,
    ros2_domain: int = 0,
    sensor_topics: list[str] | None = None,
    action_servers: list[str] | None = None,
    robot_type: str = "wheeled",
) -> FastAPI:
    """Create the FastAPI app for the JunoClaw ROS2 bridge."""
    bridge = Ros2Bridge(
        robot_id=robot_id,
        simulate=simulate,
        ros2_domain=ros2_domain,
        sensor_topics=sensor_topics,
        action_servers=action_servers,
        robot_type=robot_type,
    )

    app = FastAPI(
        title="JunoClaw ROS2 Bridge",
        version="0.1.0",
        description="HTTP bridge exposing ROS2 action server results and sensor data to the JunoClaw trust stack",
    )

    @app.get("/health", response_model=HealthResponse)
    async def health():
        return bridge.health()

    @app.get("/intent/{intent_id}", response_model=IntentResult)
    async def get_intent(intent_id: str):
        return bridge.get_intent(intent_id)

    @app.post("/intent/simulate", response_model=IntentResult)
    async def simulate_intent(
        action: str = "navigate",
        target_x: float = 12.5,
        target_y: float = 8.3,
    ):
        """Generate a simulated intent for testing (no ROS2 required)."""
        intent = bridge.generate_simulated_intent(action, target_x, target_y)
        bridge.store_intent(intent)
        return intent

    @app.get("/rosbag/{batch_id}", response_model=BatchResult)
    async def get_batch(batch_id: str):
        return bridge.get_batch(batch_id)

    @app.post("/rosbag/simulate", response_model=BatchResult)
    async def simulate_batch(
        cycle_count: int = 1000,
        violate: bool = False,
    ):
        """Generate a simulated reflex batch for testing (no ROS2 required)."""
        batch = bridge.generate_simulated_batch(cycle_count=cycle_count, violate=violate)
        bridge.store_batch(batch)
        return batch

    @app.post("/robot/register", response_model=dict)
    async def register_robot(request: Request):
        """Register the robot's capabilities and any taught skills against
        the on-chain skill-registry + marketplace.

        `skill-registry` is already deployed on testnet and mainnet — for
        each skill this robot has taught, this returns a ready-to-sign
        `PublishSkill` entry (real sha256 hash, real contract address).
        `marketplace` (skill listing/hire) is built and tested but not yet
        deployed, reported honestly below rather than implied as live.
        Nothing here is broadcast — the bridge holds no wallet key; see
        each skill's `registry_msg`/`marketplace_msg` endpoint for the
        exact payload to submit with an operator's own signer.
        """
        base = str(request.base_url).rstrip("/")
        skill_entries = []
        for name, skill in bridge.skills.items():
            canonical = json.dumps(skill, sort_keys=True).encode()
            skill_entries.append({
                "dapp_name": name,
                "skill_hash": hashlib.sha256(canonical).hexdigest(),
                "registry_msg_url": f"{base}/skills/{name}/registry_msg",
                "marketplace_msg_url": f"{base}/skills/{name}/marketplace_msg",
            })

        return {
            "robot_id": bridge.robot_id,
            "status": "ready_to_register" if skill_entries else "no_skills_taught_yet",
            "skill_registry": {
                "deployed": True,
                "contract_addresses": {
                    "testnet": "juno1pug0zu6f93nmvjl559s0uymr92jhmn5t76p7knh9zg4sqlpygqyq0nn8gz",
                    "mainnet": "juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s",
                },
                "skills": skill_entries,
            },
            "marketplace": {
                "deployed": False,
                "note": "built and tested (contracts/marketplace) but not yet deployed to testnet or mainnet",
            },
            "action_servers": bridge.action_servers,
            "sensor_topics": bridge.sensor_topics,
        }

    @app.post("/robot/expression")
    async def set_expression(request: Request):
        """Set the robot's face screen expression.
        
        Maps trust layer verdicts to DOGZILLA-Lite's IPS display expressions.
        In simulate mode, just logs the expression. In ROS2 mode, publishes
        to /display/expression topic which the CM5 maps to one of 35 expressions.
        """
        body = await request.json()
        expression = body.get("expression", "neutral")
        source = body.get("source", "unknown")

        valid = ["happy", "neutral", "alert", "confused", "sleeping", "angry", "scared", "curious"]
        if expression not in valid:
            return JSONResponse(
                status_code=400,
                content={"error": f"invalid expression '{expression}'", "valid": valid},
            )

        bridge._latest_expression = expression
        if bridge.simulate:
            print(f"[Expression] {expression} (source={source}, robot={bridge.robot_id})")
        else:
            # In real ROS2 mode, publish to /display/expression
            # The DOGZILLA-Lite CM5 subscriber maps this to the IPS display
            try:
                from std_msgs.msg import String
                if bridge._ros2_node:
                    pub = bridge._ros2_node.create_publisher(String, "/display/expression", 10)
                    msg = String()
                    msg.data = expression
                    pub.publish(msg)
            except Exception as e:
                print(f"[Expression] Failed to publish: {e}")

        return {
            "status": "ok",
            "robot_id": bridge.robot_id,
            "expression": expression,
            "source": source,
            "simulate": bridge.simulate,
        }

    @app.post("/robot/joint_command")
    async def joint_command(request: Request):
        """Teleop a single joint (radians). Used by the /viewer sliders.

        Body: {"joint": "fl_hip", "position": 0.5}
        """
        body = await request.json()
        joint = body.get("joint", "")
        position = float(body.get("position", 0.0))
        bridge.set_joint_command(joint, position)
        return {"status": "ok", "joint": joint, "position": position}

    @app.post("/robot/joint_commands")
    async def joint_commands(request: Request):
        """Batch teleop — set several joints in one call (one request per
        gait tick / pose preset instead of 15). Used by the viewer's
        Gaits & Actions panel.

        Body: {"joints": {"fl_hip": 0.1, "fl_thigh": 0.3, ...}}
        """
        body = await request.json()
        joints = body.get("joints", {})
        for joint, position in joints.items():
            bridge.set_joint_command(joint, float(position))
        return {"status": "ok", "joints": joints}

    @app.post("/robot/policy/load")
    async def policy_load(request: Request):
        """Load a trained sim2real ONNX policy (see sim2real/export_onnx.py)
        for closed-loop inference. Does not start it.

        Body: {"onnx_path": "sim2real/checkpoints/policy_walk.onnx"}
        """
        body = await request.json()
        onnx_path = body.get("onnx_path", "")
        if not onnx_path:
            raise HTTPException(status_code=400, detail="onnx_path is required")
        return bridge.load_policy(onnx_path)

    @app.post("/robot/policy/start")
    async def policy_start(request: Request):
        """Start the closed-loop policy inference loop.

        Body (optional): {"hz": 30}
        """
        body = await request.json() if request.headers.get("content-length", "0") != "0" else {}
        return bridge.start_policy(
            hz=float(body.get("hz", 30.0)),
            smoothing=float(body.get("smoothing", 0.2)),
        )

    @app.post("/robot/policy/stop")
    async def policy_stop():
        return bridge.stop_policy()

    @app.get("/robot/policy/status")
    async def policy_status():
        """Outcome of the most recent (or in-progress) policy inference
        loop, including whether the kinematic safety clamp rejected a step.
        """
        return bridge._last_policy_status

    @app.websocket("/ws/state")
    async def ws_state(websocket: WebSocket):
        """Push joint/IMU/expression state to the browser viewer at ~10Hz."""
        await websocket.accept()
        try:
            while True:
                await websocket.send_json(bridge.state_snapshot())
                await asyncio.sleep(0.1)
        except WebSocketDisconnect:
            pass

    @app.post("/skills/record/start")
    async def skill_record_start(request: Request):
        body = await request.json() if request.headers.get("content-length", "0") != "0" else {}
        bridge.start_recording(cycle_dt_ms=int(body.get("cycle_dt_ms", 100)))
        return {"status": "recording"}

    @app.post("/skills/record/stop")
    async def skill_record_stop(request: Request):
        body = await request.json()
        skill = bridge.stop_recording(
            name=body["name"],
            description=body.get("description", ""),
            license_=body.get("license", "CC0"),
            cycle_dt_ms=int(body.get("cycle_dt_ms", 100)),
        )
        return skill["manifest"]

    @app.get("/skills")
    async def list_skills():
        return {"skills": [s["manifest"] for s in bridge.skills.values()]}

    @app.get("/skills/{name}/export")
    async def export_skill(name: str):
        if name not in bridge.skills:
            raise HTTPException(status_code=404, detail=f"skill '{name}' not found")
        return bridge.skills[name]

    @app.post("/skills/import")
    async def import_skill(request: Request):
        """Import a skill artifact — from this bridge, another robot's
        bridge, or the Rust `Skill::to_json()` export. Returns a retarget
        coverage report against this robot's own joint schema so a partial
        cross-embodiment match is visible immediately.
        """
        skill = await request.json()
        name = skill["manifest"]["name"]
        bridge.skills[name] = skill
        _, report = bridge.retarget_skill(skill, bridge.QUADRUPED_JOINTS)
        return {"status": "ok", "name": name, "retarget_report": report}

    @app.post("/skills/{name}/play")
    async def play_skill(name: str):
        return bridge.play_skill(name)

    @app.get("/skills/playback/status")
    async def playback_status():
        """Outcome of the most recent (or in-progress) `play_skill` call,
        including whether the kinematic safety clamp rejected a frame.
        """
        return bridge._last_playback

    @app.get("/skills/{name}/registry_msg")
    async def skill_registry_msg(name: str, request: Request, uri: Optional[str] = None):
        """Generate a ready-to-sign `PublishSkill` ExecuteMsg for the
        already-deployed `skill-registry` CosmWasm contract.

        This does not sign or broadcast anything — the bridge never holds a
        wallet key. It computes the real sha256 of the exported skill
        artifact and returns the exact message + contract address for an
        operator to submit with their own wallet (CLI or MCP tool).

        `uri` defaults to this bridge's own `/skills/{name}/export` URL —
        replace it with a permanent location (e.g. a Blossom/IPFS blob URL)
        before publishing off this LAN.
        """
        if name not in bridge.skills:
            raise HTTPException(status_code=404, detail=f"skill '{name}' not found")

        skill = bridge.skills[name]
        canonical = json.dumps(skill, sort_keys=True).encode()
        skill_hash = hashlib.sha256(canonical).hexdigest()
        default_uri = str(request.base_url).rstrip("/") + f"/skills/{name}/export"

        return {
            "contract_addresses": {
                "testnet": "juno1pug0zu6f93nmvjl559s0uymr92jhmn5t76p7knh9zg4sqlpygqyq0nn8gz",
                "mainnet": "juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s",
            },
            "execute_msg": {
                "publish_skill": {
                    "dapp_name": name,
                    "chain_id": "juno-1",
                    "skill_uri": uri or default_uri,
                    "skill_hash": skill_hash,
                }
            },
            "note": (
                "Not broadcast — this bridge holds no wallet key. Submit this "
                "execute_msg to skill-registry with your own signer "
                "(e.g. `junod tx wasm execute <address> '<execute_msg>' "
                "--from <key>`). skill_uri defaults to this bridge's own "
                "export endpoint; replace with a permanent URI before "
                "publishing off this LAN."
            ),
        }

    @app.get("/skills/{name}/marketplace_msg")
    async def skill_marketplace_msg(name: str, price_ujuno: int = 0, description: str = ""):
        """Generate a ready-to-sign `ListService` ExecuteMsg for the
        `marketplace` contract, referencing this skill's `dapp_name` as
        `skill_ref` (the convention `marketplace` already expects from
        `skill-registry`).

        Honest status: `marketplace` and `truth-market` are built and
        tested (`contracts/marketplace`, `contracts/truth-market`) but not
        yet deployed to testnet or mainnet — `skill-registry` is the only
        one of the three live today (see `deploy/deployed-{testnet,mainnet}.json`).
        This endpoint returns the message shape now so listing a skill is a
        deploy-and-submit away, not a from-scratch build.
        """
        if name not in bridge.skills:
            raise HTTPException(status_code=404, detail=f"skill '{name}' not found")

        return {
            "marketplace_deployed": False,
            "note": (
                "marketplace and truth-market contracts are built and "
                "tested but not yet deployed (see deploy/deployed-*.json, "
                "which lists skill-registry only). Deploy marketplace "
                "(instantiated with the skill-registry address above) "
                "before this execute_msg can be submitted."
            ),
            "execute_msg": {
                "list_service": {
                    "skill_ref": name,
                    "price": str(price_ujuno),
                    "description": description or bridge.skills[name]["manifest"].get("description", ""),
                }
            },
        }

    @app.get("/viewer", response_class=HTMLResponse)
    async def viewer():
        """Single-file, no-install browser viewer — open from any phone/PC
        on the same network (or over tailscale). Live joint + IMU telemetry
        via WebSocket, joint teleop sliders, and expression buttons.
        """
        return VIEWER_HTML.replace("__ROBOT_ID__", bridge.robot_id)

    @app.get("/", response_class=JSONResponse)
    async def root():
        return {
            "service": "junoclaw-ros2-bridge",
            "version": "0.1.0",
            "robot_id": bridge.robot_id,
            "simulate": bridge.simulate,
            "endpoints": [
                "GET /health",
                "GET /intent/{intent_id}",
                "POST /intent/simulate",
                "GET /rosbag/{batch_id}",
                "POST /rosbag/simulate",
                "POST /robot/register",
                "POST /robot/expression",
                "POST /robot/joint_command",
                "POST /robot/joint_commands",
                "POST /robot/policy/load",
                "POST /robot/policy/start",
                "POST /robot/policy/stop",
                "GET /robot/policy/status",
                "WS /ws/state",
                "GET /viewer",
                "POST /skills/record/start",
                "POST /skills/record/stop",
                "GET /skills",
                "GET /skills/{name}/export",
                "POST /skills/import",
                "POST /skills/{name}/play",
                "GET /skills/playback/status",
                "GET /skills/{name}/registry_msg",
                "GET /skills/{name}/marketplace_msg",
            ],
            "robot_type": bridge.robot_type,
        }

    app.state.bridge = bridge
    return app


VIEWER_HTML = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>DOGZILLA-Lite — JunoClaw Live Viewer</title>
<style>
  :root { color-scheme: dark; }
  body { margin: 0; font-family: -apple-system, system-ui, sans-serif; background: #0d1117; color: #c9d1d9; }
  header { padding: 16px 20px; border-bottom: 1px solid #30363d; display: flex; align-items: center; gap: 10px; }
  header h1 { font-size: 16px; margin: 0; font-weight: 600; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: #f85149; }
  .dot.live { background: #3fb950; }
  main { padding: 16px 20px; max-width: 720px; margin: 0 auto; }
  section { margin-bottom: 24px; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: .05em; color: #8b949e; margin: 0 0 10px; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .joint { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 8px 10px; }
  .joint label { font-size: 11px; color: #8b949e; display: block; margin-bottom: 4px; }
  .joint input[type=range] { width: 100%; }
  .joint .val { font-size: 11px; color: #58a6ff; float: right; }
  .expr-row { display: flex; flex-wrap: wrap; gap: 8px; }
  .expr-row button { background: #21262d; border: 1px solid #30363d; color: #c9d1d9; border-radius: 6px; padding: 8px 14px; cursor: pointer; font-size: 13px; }
  .expr-row button:hover { background: #30363d; }
  .expr-row button.active { background: #1f6feb; border-color: #1f6feb; color: white; }
  .imu-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; font-size: 12px; }
  .imu-grid div { background: #161b22; border-radius: 6px; padding: 8px; text-align: center; }
  .imu-grid .k { color: #8b949e; display: block; font-size: 10px; }
  .skill-form { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 12px; }
  .skill-form input, .skill-form select { background: #0d1117; border: 1px solid #30363d; color: #c9d1d9; border-radius: 6px; padding: 8px 10px; font-size: 13px; }
  .skill-form input[type=text] { flex: 1; min-width: 120px; }
  .btn { background: #21262d; border: 1px solid #30363d; color: #c9d1d9; border-radius: 6px; padding: 8px 14px; cursor: pointer; font-size: 13px; }
  .btn:hover { background: #30363d; }
  .btn.record { background: #1f6feb; border-color: #1f6feb; color: white; }
  .btn.record.active { background: #da3633; border-color: #da3633; }
  .skill-row { display: flex; align-items: center; justify-content: space-between; background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 8px 12px; margin-bottom: 6px; font-size: 13px; }
  .skill-row .meta { color: #8b949e; font-size: 11px; }
  .skill-row .actions { display: flex; gap: 6px; }
  .skill-row .actions button { font-size: 12px; padding: 4px 10px; }
  .coverage-note { font-size: 12px; color: #d29922; margin-top: 6px; }
  .robot-wrap { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 0; overflow: hidden; }
  #robot3d { width: 100%; height: 320px; touch-action: none; }
  .robot-hint { font-size: 11px; color: #8b949e; margin-top: 6px; text-align: center; }
  .action-row { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 14px; }
  .stop-btn { background: #da3633; border-color: #da3633; color: white; }
  .stop-btn:hover { background: #f85149; }
  .gait-btn.active { background: #1f6feb; border-color: #1f6feb; color: white; }
  .dpad-row { display: flex; align-items: center; gap: 24px; flex-wrap: wrap; }
  .dpad { display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .dpad-mid { display: flex; gap: 44px; }
  .dpad-btn { width: 44px; height: 44px; border-radius: 8px; background: #21262d; border: 1px solid #30363d; color: #c9d1d9; font-size: 18px; cursor: pointer; user-select: none; touch-action: none; }
  .dpad-btn.active { background: #1f6feb; border-color: #1f6feb; color: white; }
  .speed-ctrl { font-size: 12px; color: #8b949e; }
  .speed-ctrl label { display: block; margin-bottom: 4px; }
  .speed-ctrl input { width: 140px; }
</style>
</head>
<body>
<header>
  <span class="dot" id="dot"></span>
  <h1>DOGZILLA-Lite — __ROBOT_ID__ — JunoClaw Live Viewer</h1>
</header>
<main>
  <section>
    <h2>Robot</h2>
    <div class="robot-wrap"><div id="robot3d"></div></div>
    <div class="robot-hint">drag to orbit &middot; pinch/scroll to zoom</div>
  </section>
  <section>
    <h2>Gaits &amp; Actions</h2>
    <div class="action-row">
      <button class="btn" id="act-stand">Stand</button>
      <button class="btn" id="act-sit">Sit</button>
      <button class="btn" id="act-stretch">Stretch</button>
      <button class="btn" id="act-wave">Wave</button>
      <button class="btn gait-btn" id="act-trot">Trot in place</button>
      <button class="btn stop-btn" id="act-stop">STOP</button>
    </div>
    <div class="dpad-row">
      <div class="dpad">
        <button class="dpad-btn" id="dpad-fwd">&#8593;</button>
        <div class="dpad-mid">
          <button class="dpad-btn" id="dpad-left">&#8592;</button>
          <button class="dpad-btn" id="dpad-right">&#8594;</button>
        </div>
        <button class="dpad-btn" id="dpad-back">&#8595;</button>
      </div>
      <div class="speed-ctrl">
        <label>Speed <span id="speed-val">1.0x</span></label>
        <input type="range" id="speed-slider" min="0.3" max="2" step="0.1" value="1">
      </div>
    </div>
  </section>
  <section>
    <h2>Expression</h2>
    <div class="expr-row" id="expr-row"></div>
  </section>
  <section>
    <h2>Skills — teach once, run anywhere</h2>
    <div class="skill-form">
      <input type="text" id="skill-name" placeholder="skill name (e.g. wave)">
      <input type="text" id="skill-desc" placeholder="description (optional)">
      <select id="skill-license">
        <option value="CC0">CC0</option>
        <option value="MIT">MIT</option>
        <option value="Apache-2.0">Apache-2.0</option>
      </select>
      <button class="btn record" id="record-btn">Start Recording</button>
      <label class="btn" for="import-file" style="margin:0;">Import Skill</label>
      <input type="file" id="import-file" accept="application/json" style="display:none;">
    </div>
    <div id="skill-list"></div>
    <div class="coverage-note" id="coverage-note"></div>
  </section>
  <section>
    <h2>Joints (rad)</h2>
    <div class="grid" id="joints"></div>
  </section>
  <section>
    <h2>IMU</h2>
    <div class="imu-grid" id="imu"></div>
  </section>
</main>
<script>
const JOINTS = ["fl_hip","fl_thigh","fl_calf","fr_hip","fr_thigh","fr_calf",
                "rl_hip","rl_thigh","rl_calf","rr_hip","rr_thigh","rr_calf",
                "arm_base","arm_shoulder","arm_gripper"];
const EXPRESSIONS = ["happy","neutral","alert","confused","sleeping","angry","scared","curious"];

// --- Robot visual: driven by joint state, rendered by the Three.js module below ---
let currentExpression = "neutral";

function getJointValues() {
  const v = {};
  JOINTS.forEach(n => v[n] = parseFloat(sliders[n] ? sliders[n].value : 0));
  return v;
}

function updateRobotView(joints, expression) {
  if (expression) currentExpression = expression;
  if (window.updateRobot3D) window.updateRobot3D(joints, currentExpression);
}

const jointsEl = document.getElementById("joints");
const sliders = {};
JOINTS.forEach(name => {
  const div = document.createElement("div");
  div.className = "joint";
  div.innerHTML = `<label>${name} <span class="val" id="val-${name}">0.00</span></label>
    <input type="range" min="-1.57" max="1.57" step="0.01" value="0" id="slider-${name}">`;
  jointsEl.appendChild(div);
  const slider = div.querySelector("input");
  sliders[name] = slider;
  slider.addEventListener("input", () => {
    document.getElementById(`val-${name}`).textContent = parseFloat(slider.value).toFixed(2);
    updateRobotView(getJointValues());
  });
  slider.addEventListener("change", () => {
    fetch("/robot/joint_command", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({joint: name, position: parseFloat(slider.value)})
    });
  });
});

const exprRow = document.getElementById("expr-row");
EXPRESSIONS.forEach(name => {
  const btn = document.createElement("button");
  btn.textContent = name;
  btn.id = `expr-${name}`;
  btn.addEventListener("click", () => {
    updateRobotView(getJointValues(), name);
    fetch("/robot/expression", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({expression: name, source: "viewer"})
    });
  });
  exprRow.appendChild(btn);
});

const imuEl = document.getElementById("imu");
const IMU_KEYS = ["accel_x","accel_y","accel_z","gyro_x","gyro_y","gyro_z"];
IMU_KEYS.forEach(k => {
  const div = document.createElement("div");
  div.innerHTML = `<span class="k">${k}</span><span id="imu-${k}">0.00</span>`;
  imuEl.appendChild(div);
});

function connect() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${proto}//${location.host}/ws/state`);
  const dot = document.getElementById("dot");
  ws.onopen = () => dot.classList.add("live");
  ws.onclose = () => { dot.classList.remove("live"); setTimeout(connect, 1000); };
  ws.onerror = () => ws.close();
  ws.onmessage = (evt) => {
    const state = JSON.parse(evt.data);
    JOINTS.forEach(name => {
      const v = state.joints[name] ?? 0;
      if (document.activeElement !== sliders[name]) {
        sliders[name].value = v;
        document.getElementById(`val-${name}`).textContent = v.toFixed(2);
      }
    });
    IMU_KEYS.forEach(k => {
      const el = document.getElementById(`imu-${k}`);
      if (el) el.textContent = (state.imu[k] ?? 0).toFixed(2);
    });
    EXPRESSIONS.forEach(name => {
      document.getElementById(`expr-${name}`).classList.toggle("active", state.expression === name);
    });
    updateRobotView(state.joints, state.expression);
  };
}
connect();
updateRobotView(getJointValues(), currentExpression);

// --- Gaits & Actions: pose presets, canned trot gait, D-pad walk/turn ----
const STAND_POSE = {
  fl_hip: 0, fl_thigh: 0.35, fl_calf: -0.65,
  fr_hip: 0, fr_thigh: 0.35, fr_calf: -0.65,
  rl_hip: 0, rl_thigh: -0.35, rl_calf: 0.65,
  rr_hip: 0, rr_thigh: -0.35, rr_calf: 0.65,
  arm_base: 0, arm_shoulder: 0, arm_gripper: 0,
};
const SIT_POSE = {
  fl_hip: 0, fl_thigh: 0.35, fl_calf: -0.65,
  fr_hip: 0, fr_thigh: 0.35, fr_calf: -0.65,
  rl_hip: 0, rl_thigh: -1.0, rl_calf: 1.3,
  rr_hip: 0, rr_thigh: -1.0, rr_calf: 1.3,
  arm_shoulder: 0.3,
};
const STRETCH_POSE = {
  fl_hip: 0, fl_thigh: 0.7, fl_calf: -1.1,
  fr_hip: 0, fr_thigh: 0.7, fr_calf: -1.1,
  rl_hip: 0, rl_thigh: -0.7, rl_calf: 1.1,
  rr_hip: 0, rr_thigh: -0.7, rr_calf: 1.1,
};

let activeLoop = null;
function stopActiveLoop() {
  if (activeLoop) clearInterval(activeLoop.interval);
  activeLoop = null;
  trotBtn.classList.remove("active");
  document.querySelectorAll(".dpad-btn").forEach(b => b.classList.remove("active"));
}

function applyJoints(map) {
  Object.entries(map).forEach(([name, val]) => {
    if (sliders[name]) {
      sliders[name].value = val;
      const lbl = document.getElementById(`val-${name}`);
      if (lbl) lbl.textContent = val.toFixed(2);
    }
  });
  updateRobotView(getJointValues());
  fetch("/robot/joint_commands", {
    method: "POST",
    headers: {"Content-Type": "application/json"},
    body: JSON.stringify({joints: map})
  }).catch(() => {});
}

// Diagonal trot: (fl,rr) in phase, (fr,rl) opposite phase. dirBias flips
// fore/aft swing (forward/back), turnBias biases hip abduction left/right.
function trotFrame(t, turnBias, speed, dirBias) {
  const w = 2 * Math.PI * 1.1 * speed;
  const phases = { fl: 0, rr: 0, fr: Math.PI, rl: Math.PI };
  const out = {};
  Object.entries(phases).forEach(([name, phase]) => {
    const s = Math.sin(w * t + phase);
    const lift = Math.max(0, s);
    out[name + "_thigh"] = 0.35 * s * dirBias;
    out[name + "_calf"] = -0.55 - 0.3 * lift;
    const sideSign = name.includes("l") ? 1 : -1;
    out[name + "_hip"] = turnBias * 0.18 * sideSign;
  });
  return out;
}

function startGaitLoop(dirBias, turnBias) {
  stopActiveLoop();
  const startT = performance.now();
  const interval = setInterval(() => {
    const t = (performance.now() - startT) / 1000;
    const speed = parseFloat(speedSlider.value);
    applyJoints(trotFrame(t, turnBias, speed, dirBias));
  }, 100);
  activeLoop = {interval};
}

function animateToPose(target, duration = 1200) {
  stopActiveLoop();
  const start = getJointValues();
  const steps = Math.max(10, Math.round(duration / 50));
  let i = 0;
  const interval = setInterval(() => {
    i++;
    const f = Math.min(1, i / steps);
    const frame = {};
    Object.keys(target).forEach(k => {
      const a = start[k] ?? 0, b = target[k];
      frame[k] = a + (b - a) * f;
    });
    applyJoints(frame);
    if (f >= 1) { clearInterval(interval); activeLoop = null; }
  }, 50);
  activeLoop = {interval};
}

function waveArm() {
  stopActiveLoop();
  const startT = performance.now();
  const interval = setInterval(() => {
    const t = (performance.now() - startT) / 1000;
    if (t > 2.4) {
      clearInterval(interval);
      activeLoop = null;
      applyJoints({arm_base: 0, arm_shoulder: 0});
      return;
    }
    applyJoints({arm_base: 0.3 * Math.sin(t * 8), arm_shoulder: -0.6 + 0.1 * Math.sin(t * 8)});
  }, 60);
  activeLoop = {interval};
}

document.getElementById("act-stand").addEventListener("click", () => animateToPose(STAND_POSE));
document.getElementById("act-sit").addEventListener("click", () => animateToPose(SIT_POSE));
document.getElementById("act-stretch").addEventListener("click", () => animateToPose(STRETCH_POSE, 900));
document.getElementById("act-wave").addEventListener("click", () => waveArm());
document.getElementById("act-stop").addEventListener("click", () => stopActiveLoop());

const trotBtn = document.getElementById("act-trot");
trotBtn.addEventListener("click", () => {
  if (activeLoop) { stopActiveLoop(); return; }
  startGaitLoop(1, 0);
  trotBtn.classList.add("active");
});

const speedSlider = document.getElementById("speed-slider");
const speedVal = document.getElementById("speed-val");
speedSlider.addEventListener("input", () => speedVal.textContent = parseFloat(speedSlider.value).toFixed(1) + "x");

function bindHold(btn, dirBias, turnBias) {
  const start = (e) => { e.preventDefault(); stopActiveLoop(); startGaitLoop(dirBias, turnBias); btn.classList.add("active"); };
  const stop = () => stopActiveLoop();
  btn.addEventListener("pointerdown", start);
  ["pointerup", "pointerleave", "pointercancel"].forEach(ev => btn.addEventListener(ev, stop));
}
bindHold(document.getElementById("dpad-fwd"), 1, 0);
bindHold(document.getElementById("dpad-back"), -1, 0);
bindHold(document.getElementById("dpad-left"), 0.5, -1);
bindHold(document.getElementById("dpad-right"), 0.5, 1);

// --- Skills: teach once, run anywhere ------------------------------------
let recording = false;
const recordBtn = document.getElementById("record-btn");
const coverageNote = document.getElementById("coverage-note");

recordBtn.addEventListener("click", async () => {
  if (!recording) {
    await fetch("/skills/record/start", {method: "POST"});
    recording = true;
    recordBtn.textContent = "Stop && Save";
    recordBtn.classList.add("active");
  } else {
    const name = document.getElementById("skill-name").value.trim() || `skill_${Date.now()}`;
    const description = document.getElementById("skill-desc").value.trim();
    const license = document.getElementById("skill-license").value;
    await fetch("/skills/record/stop", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name, description, license})
    });
    recording = false;
    recordBtn.textContent = "Start Recording";
    recordBtn.classList.remove("active");
    document.getElementById("skill-name").value = "";
    refreshSkillList();
  }
});

document.getElementById("import-file").addEventListener("change", async (evt) => {
  const file = evt.target.files[0];
  if (!file) return;
  const text = await file.text();
  const skill = JSON.parse(text);
  const res = await fetch("/skills/import", {
    method: "POST",
    headers: {"Content-Type": "application/json"},
    body: text
  });
  const result = await res.json();
  const report = result.retarget_report;
  coverageNote.textContent = `Imported '${result.name}' — coverage ${(report.coverage * 100).toFixed(0)}% ` +
    `(${report.matched_joints.length} joints matched` +
    (report.missing_in_target.length ? `, ${report.missing_in_target.length} not present on this robot` : "") + ")";
  evt.target.value = "";
  refreshSkillList();
});

async function playSkill(name) {
  const res = await fetch(`/skills/${encodeURIComponent(name)}/play`, {method: "POST"});
  const result = await res.json();
  const report = result.retarget_report;
  coverageNote.textContent = `Playing '${name}' — ${result.frames} frames, ${(report.coverage * 100).toFixed(0)}% joint coverage on this robot`;
}

function exportSkill(name) {
  const a = document.createElement("a");
  a.href = `/skills/${encodeURIComponent(name)}/export`;
  a.download = `${name}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
}

async function refreshSkillList() {
  const res = await fetch("/skills");
  const {skills} = await res.json();
  const list = document.getElementById("skill-list");
  list.innerHTML = "";
  if (skills.length === 0) {
    list.innerHTML = '<div class="meta">No skills taught yet — hit Start Recording, pose/drive the robot, then Stop && Save.</div>';
    return;
  }
  skills.forEach(m => {
    const row = document.createElement("div");
    row.className = "skill-row";
    row.innerHTML = `<div><strong>${m.name}</strong> <span class="meta">${m.frame_count} frames · ${m.license} · by ${m.author_robot_id}</span></div>`;
    const actions = document.createElement("div");
    actions.className = "actions";
    const playBtn = document.createElement("button");
    playBtn.className = "btn";
    playBtn.textContent = "Play";
    playBtn.onclick = () => playSkill(m.name);
    const exportBtn = document.createElement("button");
    exportBtn.className = "btn";
    exportBtn.textContent = "Export";
    exportBtn.onclick = () => exportSkill(m.name);
    actions.appendChild(playBtn);
    actions.appendChild(exportBtn);
    row.appendChild(actions);
    list.appendChild(row);
  });
}
refreshSkillList();
</script>
<script type="importmap">
{"imports": {"three": "https://unpkg.com/three@0.160.0/build/three.module.js"}}
</script>
<script type="module">
import * as THREE from "three";
import { OrbitControls } from "https://unpkg.com/three@0.160.0/examples/jsm/controls/OrbitControls.js";

const container = document.getElementById("robot3d");
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0d1117);

const camera = new THREE.PerspectiveCamera(45, container.clientWidth / (container.clientHeight || 320), 0.01, 10);
camera.position.set(0.55, 0.45, 0.6);

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
renderer.setSize(container.clientWidth, container.clientHeight || 320);
container.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set(0, 0.16, 0);
controls.enableDamping = true;
controls.dampingFactor = 0.08;

scene.add(new THREE.AmbientLight(0xffffff, 0.65));
const dirLight = new THREE.DirectionalLight(0xffffff, 0.9);
dirLight.position.set(1, 2, 1);
scene.add(dirLight);

const grid = new THREE.GridHelper(1.0, 10, 0x30363d, 0x21262d);
scene.add(grid);

const BODY_W = 0.20, BODY_H = 0.075, BODY_D = 0.30, STAND_Y = 0.18;
const bodyMat = new THREE.MeshStandardMaterial({ color: 0x21262d, metalness: 0.3, roughness: 0.6 });
const body = new THREE.Mesh(new THREE.BoxGeometry(BODY_W, BODY_H, BODY_D), bodyMat);
body.position.y = STAND_Y;
scene.add(body);

// Real DOGZILLA-Lite face: an IPS display on the front of the body. We
// render it as an actual canvas texture (not just a colored dot) so the
// viewer shows the same eye/mouth shapes the real screen would.
const EXPR_COLOR = {
  happy: "#3fb950", neutral: "#c9d1d9", alert: "#58a6ff", confused: "#d29922",
  sleeping: "#8b949e", angry: "#f85149", scared: "#a371f7", curious: "#e3b341",
};
const faceCanvas = document.createElement("canvas");
faceCanvas.width = 200; faceCanvas.height = 120;
const fctx = faceCanvas.getContext("2d");
const faceTexture = new THREE.CanvasTexture(faceCanvas);
const faceMat = new THREE.MeshBasicMaterial({ map: faceTexture });
// Raised head/camera module at the front-top of the chassis — real
// quadruped kits (DOGZILLA included) mount the face display + camera on a
// stepped-up head block, not flush with the main body panel.
const HEAD_W = BODY_W * 0.72, HEAD_H = BODY_H * 1.1, HEAD_D = BODY_D * 0.2;
const headMat = new THREE.MeshStandardMaterial({ color: 0x2d333b, metalness: 0.35, roughness: 0.55 });
const head = new THREE.Mesh(new THREE.BoxGeometry(HEAD_W, HEAD_H, HEAD_D), headMat);
head.position.set(0, BODY_H / 2 + HEAD_H * 0.3, BODY_D / 2 + HEAD_D * 0.35);
body.add(head);

const lensMat = new THREE.MeshStandardMaterial({ color: 0x0d1117, metalness: 0.6, roughness: 0.2 });
const lens = new THREE.Mesh(new THREE.CylinderGeometry(0.008, 0.008, 0.01, 12), lensMat);
lens.rotation.x = Math.PI / 2;
lens.position.set(0, HEAD_H / 2 - 0.006, HEAD_D / 2 + 0.003);
head.add(lens);

const face = new THREE.Mesh(new THREE.PlaneGeometry(HEAD_W * 0.7, HEAD_H * 0.45), faceMat);
face.position.set(0, -0.006, HEAD_D / 2 + 0.002);
head.add(face);

function drawFace(expression) {
  const color = EXPR_COLOR[expression] || "#c9d1d9";
  fctx.fillStyle = "#05070a";
  fctx.fillRect(0, 0, 200, 120);
  fctx.strokeStyle = color;
  fctx.fillStyle = color;
  fctx.lineWidth = 6;
  fctx.lineCap = "round";
  const cx1 = 70, cx2 = 130, cy = 52;
  if (expression === "sleeping") {
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.moveTo(cx - 16, cy); fctx.lineTo(cx + 16, cy); fctx.stroke(); });
  } else if (expression === "happy") {
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.arc(cx, cy + 10, 16, Math.PI, 0); fctx.stroke(); });
  } else if (expression === "angry") {
    fctx.beginPath(); fctx.moveTo(cx1 - 16, cy - 16); fctx.lineTo(cx1 + 14, cy - 2); fctx.stroke();
    fctx.beginPath(); fctx.moveTo(cx2 + 16, cy - 16); fctx.lineTo(cx2 - 14, cy - 2); fctx.stroke();
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.arc(cx, cy + 8, 11, 0, 7); fctx.fill(); });
  } else if (expression === "scared") {
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.arc(cx, cy, 19, 0, 7); fctx.stroke(); fctx.beginPath(); fctx.arc(cx, cy, 6, 0, 7); fctx.fill(); });
  } else if (expression === "curious") {
    fctx.beginPath(); fctx.arc(cx1, cy, 13, 0, 7); fctx.fill();
    fctx.beginPath(); fctx.moveTo(cx2 - 16, cy - 18); fctx.lineTo(cx2 + 16, cy - 24); fctx.stroke();
    fctx.beginPath(); fctx.arc(cx2, cy, 13, 0, 7); fctx.fill();
  } else if (expression === "confused") {
    fctx.beginPath(); fctx.arc(cx1, cy, 13, 0, 7); fctx.fill();
    fctx.beginPath(); fctx.arc(cx2, cy - 8, 13, 0, 7); fctx.fill();
  } else if (expression === "alert") {
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.arc(cx, cy, 16, 0, 7); fctx.fill(); });
  } else {
    [cx1, cx2].forEach(cx => { fctx.beginPath(); fctx.arc(cx, cy, 12, 0, 7); fctx.fill(); });
  }
  fctx.beginPath();
  if (expression === "happy") {
    fctx.arc(100, 88, 22, 0.15 * Math.PI, 0.85 * Math.PI);
  } else if (expression === "angry" || expression === "scared") {
    fctx.arc(100, 100, 18, 1.15 * Math.PI, 1.85 * Math.PI);
  } else if (expression !== "sleeping") {
    fctx.moveTo(84, 92); fctx.lineTo(116, 92);
  }
  fctx.stroke();
  faceTexture.needsUpdate = true;
}
drawFace("neutral");
let lastExpression = "neutral";

const LEG_LEN = 0.1;
const legMat = new THREE.MeshStandardMaterial({ color: 0x9aa4ad, metalness: 0.6, roughness: 0.35 });
const footMat = new THREE.MeshStandardMaterial({ color: 0x1c1f24, roughness: 0.8 });

function makeSegment(length) {
  const geo = new THREE.CylinderGeometry(0.011, 0.009, length, 10);
  const mesh = new THREE.Mesh(geo, legMat);
  mesh.position.y = -length / 2;
  return mesh;
}

const LEG_ANCHORS = {
  fl: [-BODY_W / 2, -BODY_H / 2, BODY_D / 2 - 0.02],
  fr: [BODY_W / 2, -BODY_H / 2, BODY_D / 2 - 0.02],
  rl: [-BODY_W / 2, -BODY_H / 2, -BODY_D / 2 + 0.02],
  rr: [BODY_W / 2, -BODY_H / 2, -BODY_D / 2 + 0.02],
};
const legs = {};
Object.entries(LEG_ANCHORS).forEach(([name, pos]) => {
  const hipGroup = new THREE.Group();
  hipGroup.position.set(pos[0], pos[1], pos[2]);
  body.add(hipGroup);

  const thighGroup = new THREE.Group();
  hipGroup.add(thighGroup);
  thighGroup.add(makeSegment(LEG_LEN));

  const calfGroup = new THREE.Group();
  calfGroup.position.y = -LEG_LEN;
  thighGroup.add(calfGroup);
  calfGroup.add(makeSegment(LEG_LEN));

  const foot = new THREE.Mesh(new THREE.SphereGeometry(0.013, 10, 10), footMat);
  foot.position.y = -LEG_LEN;
  calfGroup.add(foot);

  legs[name] = { hipGroup, thighGroup, calfGroup };
});

// Arm mounts on top of the head, extending forward/outward (+z) so it
// protrudes clear of the chassis instead of sitting embedded inside it.
const armMat = new THREE.MeshStandardMaterial({ color: 0xe3b341, metalness: 0.3, roughness: 0.45 });
const ARM_SEG = 0.09;
const armBaseGroup = new THREE.Group();
armBaseGroup.position.set(0, HEAD_H / 2, HEAD_D / 2);
head.add(armBaseGroup);

const armShoulderGroup = new THREE.Group();
armBaseGroup.add(armShoulderGroup);
const upperArm = new THREE.Mesh(new THREE.CylinderGeometry(0.01, 0.01, ARM_SEG, 8), armMat);
upperArm.rotation.x = Math.PI / 2;
upperArm.position.z = ARM_SEG / 2;
armShoulderGroup.add(upperArm);

const gripperGroup = new THREE.Group();
gripperGroup.position.z = ARM_SEG;
armShoulderGroup.add(gripperGroup);
const palm = new THREE.Mesh(new THREE.BoxGeometry(0.022, 0.014, 0.018), armMat);
palm.position.z = 0.01;
gripperGroup.add(palm);
const fingerGeo = new THREE.BoxGeometry(0.007, 0.007, 0.035);
const fingerL = new THREE.Mesh(fingerGeo, armMat);
const fingerR = new THREE.Mesh(fingerGeo, armMat);
fingerL.position.set(-0.007, 0, 0.035);
fingerR.position.set(0.007, 0, 0.035);
gripperGroup.add(fingerL, fingerR);

function resize() {
  const w = container.clientWidth, h = container.clientHeight || 320;
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
  renderer.setSize(w, h);
}
window.addEventListener("resize", resize);
resize();

window.updateRobot3D = function(joints, expression) {
  joints = joints || {};
  ["fl", "fr", "rl", "rr"].forEach(name => {
    const leg = legs[name];
    leg.hipGroup.rotation.z = joints[name + "_hip"] || 0;
    leg.thighGroup.rotation.x = joints[name + "_thigh"] || 0;
    leg.calfGroup.rotation.x = joints[name + "_calf"] || 0;
  });
  armBaseGroup.rotation.y = joints.arm_base || 0;
  armShoulderGroup.rotation.x = joints.arm_shoulder || 0;
  const gripperOpen = 0.007 + Math.abs(joints.arm_gripper || 0) * 0.02;
  fingerL.position.x = -gripperOpen;
  fingerR.position.x = gripperOpen;
  if (expression && expression !== lastExpression) {
    lastExpression = expression;
    drawFace(expression);
  }
};

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();
window.updateRobot3D({}, "neutral");
</script>
</body>
</html>
"""
