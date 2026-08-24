"""Bridge server — FastAPI app exposing ROS2 data to JunoClaw plugin-ros2."""

import asyncio
import hashlib
import json
import time
import uuid
from typing import Any, Optional

from pydantic import BaseModel, Field

try:
    from fastapi import FastAPI, HTTPException, Request
    from fastapi.responses import JSONResponse
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
    async def register_robot():
        """Register the robot in the JunoClaw skill-registry via marketplace."""
        return {
            "robot_id": bridge.robot_id,
            "status": "registration_pending",
            "message": "Create skill-registry entry with robotics capability + marketplace listing gated by Truth Market",
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
            ],
            "robot_type": bridge.robot_type,
        }

    app.state.bridge = bridge
    return app
