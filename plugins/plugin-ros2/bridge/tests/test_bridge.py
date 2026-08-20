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
