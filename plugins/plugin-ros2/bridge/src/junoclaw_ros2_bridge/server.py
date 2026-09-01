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
        self._latest_imu: dict[str, float] = {}
        self._latest_expression: str = "neutral"
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
</style>
</head>
<body>
<header>
  <span class="dot" id="dot"></span>
  <h1>DOGZILLA-Lite — __ROBOT_ID__ — JunoClaw Live Viewer</h1>
</header>
<main>
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
  };
}
connect();

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
</body>
</html>
"""
