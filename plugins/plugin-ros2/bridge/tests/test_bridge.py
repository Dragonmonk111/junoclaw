"""Tests for the JunoClaw ROS2 bridge — run without ROS2 using simulation mode."""

import pytest
from httpx import AsyncClient, ASGITransport

from junoclaw_ros2_bridge.server import create_app


@pytest.fixture
def app():
    return create_app(robot_id="test-bot-01", simulate=True)


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


async def test_health(client):
    resp = await client.get("/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["robot_id"] == "test-bot-01"
    assert data["ros2_connected"] is False  # simulate mode


async def test_root(client):
    resp = await client.get("/")
    assert resp.status_code == 200
    data = resp.json()
    assert data["service"] == "junoclaw-ros2-bridge"
    assert "endpoints" in data


async def test_simulate_intent(client):
    resp = await client.post(
        "/intent/simulate",
        params={"action": "navigate", "target_x": 10.0, "target_y": 20.0},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "test-bot-01"
    assert data["action"] == "navigate"
    assert data["params"]["target_x"] == 10.0
    assert data["controller_timestamp"] > 0
    assert len(data["sensor_snapshot"]) > 0


async def test_simulate_batch_ok(client):
    resp = await client.post(
        "/rosbag/simulate",
        params={"cycle_count": 100, "violate": False},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "test-bot-01"
    assert data["cycle_count"] == 100
    assert data["all_invariants_maintained"] is True
    assert len(data["violated_invariants"]) == 0
    assert len(data["merkle_root"]) == 64  # SHA-256 hex
    assert len(data["cycles"]) == 100


async def test_simulate_batch_violation(client):
    resp = await client.post(
        "/rosbag/simulate",
        params={"cycle_count": 100, "violate": True},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["all_invariants_maintained"] is False
    assert "max_speed" in data["violated_invariants"]
    assert "min_collision_distance" in data["violated_invariants"]


async def test_get_intent_not_found(client):
    resp = await client.get("/intent/nonexistent")
    assert resp.status_code == 404


async def test_get_batch_not_found(client):
    resp = await client.get("/rosbag/nonexistent")
    assert resp.status_code == 404


async def test_register_robot(client):
    resp = await client.post("/robot/register")
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "test-bot-01"
    assert data["status"] == "registration_pending"


async def test_merkle_root_deterministic():
    from junoclaw_ros2_bridge.server import Ros2Bridge

    bridge = Ros2Bridge(robot_id="test", simulate=True)
    batch1 = bridge.generate_simulated_batch(cycle_count=100, violate=False)
    batch2 = bridge.generate_simulated_batch(cycle_count=100, violate=False)
    assert batch1.merkle_root == batch2.merkle_root


async def test_merkle_root_changes_on_violation():
    from junoclaw_ros2_bridge.server import Ros2Bridge

    bridge = Ros2Bridge(robot_id="test", simulate=True)
    batch_ok = bridge.generate_simulated_batch(cycle_count=100, violate=False)
    batch_bad = bridge.generate_simulated_batch(cycle_count=100, violate=True)
    assert batch_ok.merkle_root != batch_bad.merkle_root


@pytest.fixture
def quad_app():
    return create_app(robot_id="dogzilla-s2-001", simulate=True, robot_type="quadruped")


@pytest.fixture
async def quad_client(quad_app):
    transport = ASGITransport(app=quad_app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac


async def test_quadruped_health(quad_client):
    resp = await quad_client.get("/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "dogzilla-s2-001"
    assert data["ros2_connected"] is False


async def test_quadruped_simulate_intent(quad_client):
    resp = await quad_client.post(
        "/intent/simulate",
        params={"action": "gait_trot", "target_x": 3.0, "target_y": 2.0},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "dogzilla-s2-001"
    assert data["action"] == "gait_trot"
    assert len(data["sensor_snapshot"]) > 0


async def test_quadruped_simulate_batch_ok(quad_client):
    resp = await quad_client.post(
        "/rosbag/simulate",
        params={"cycle_count": 500, "violate": False},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_id"] == "dogzilla-s2-001"
    assert data["cycle_count"] == 500
    assert data["all_invariants_maintained"] is True
    assert len(data["merkle_root"]) == 64
    assert len(data["cycles"]) == 500
    # Check quadruped-specific sensor readings
    first_cycle = data["cycles"][0]
    assert "imu_accel_z" in first_cycle["sensor_readings"]
    assert "num_contacts" in first_cycle["sensor_readings"]
    # Check 15-DOF joint outputs (12 leg + 3 arm for DOGZILLA-Lite)
    assert len(first_cycle["control_outputs"]) == 15
    assert "fl_hip" in first_cycle["control_outputs"]
    assert "rr_calf" in first_cycle["control_outputs"]
    assert "arm_gripper" in first_cycle["control_outputs"]


async def test_quadruped_simulate_batch_violation(quad_client):
    resp = await quad_client.post(
        "/rosbag/simulate",
        params={"cycle_count": 500, "violate": True},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["all_invariants_maintained"] is False
    assert "max_tilt" in data["violated_invariants"]
    assert "max_speed" in data["violated_invariants"]


async def test_quadruped_root_shows_type(quad_client):
    resp = await quad_client.get("/")
    assert resp.status_code == 200
    data = resp.json()
    assert data["robot_type"] == "quadruped"


async def test_quadruped_merkle_deterministic():
    from junoclaw_ros2_bridge.server import Ros2Bridge

    bridge = Ros2Bridge(robot_id="dogzilla-test", simulate=True, robot_type="quadruped")
    batch1 = bridge.generate_simulated_batch(cycle_count=100, violate=False)
    batch2 = bridge.generate_simulated_batch(cycle_count=100, violate=False)
    assert batch1.merkle_root == batch2.merkle_root


async def test_quadruped_merkle_differs_from_wheeled():
    from junoclaw_ros2_bridge.server import Ros2Bridge

    quad_bridge = Ros2Bridge(robot_id="r1", simulate=True, robot_type="quadruped")
    wheeled_bridge = Ros2Bridge(robot_id="r1", simulate=True, robot_type="wheeled")
    quad_batch = quad_bridge.generate_simulated_batch(cycle_count=100, violate=False)
    wheeled_batch = wheeled_bridge.generate_simulated_batch(cycle_count=100, violate=False)
    assert quad_batch.merkle_root != wheeled_batch.merkle_root


async def test_expression_valid(client):
    resp = await client.post(
        "/robot/expression",
        json={"expression": "happy", "source": "junoclaw-trust-layer"},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["expression"] == "happy"
    assert data["source"] == "junoclaw-trust-layer"


async def test_expression_invalid(client):
    resp = await client.post(
        "/robot/expression",
        json={"expression": "sad", "source": "test"},
    )
    assert resp.status_code == 400
    data = resp.json()
    assert "error" in data


async def test_expression_quadruped(quad_client):
    resp = await quad_client.post(
        "/robot/expression",
        json={"expression": "alert", "source": "truth-market-verdict"},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["expression"] == "alert"
    assert data["robot_id"] == "dogzilla-s2-001"
