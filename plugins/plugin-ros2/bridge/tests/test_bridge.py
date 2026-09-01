"""Tests for the JunoClaw ROS2 bridge — run without ROS2 using simulation mode."""

import asyncio
import hashlib
import json

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
    assert data["status"] == "no_skills_taught_yet"
    assert data["skill_registry"]["deployed"] is True
    assert data["skill_registry"]["skills"] == []
    assert data["marketplace"]["deployed"] is False


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


def _inject_skill(app, name, joint_names, keyframes, cycle_dt_ms=10):
    """Directly install a skill artifact on the bridge, bypassing live
    recording, so playback/registry tests can use precise keyframes."""
    bridge = app.state.bridge
    bridge.skills[name] = {
        "manifest": {
            "name": name,
            "description": "test skill",
            "author_robot_id": bridge.robot_id,
            "joint_names": joint_names,
            "frame_count": len(keyframes),
            "cycle_dt_ms": cycle_dt_ms,
            "license": "CC0",
            "provenance_batch_root": "",
            "created_at_ms": 0,
        },
        "keyframes": keyframes,
    }


async def test_skill_record_export_roundtrip(client, app):
    resp = await client.post("/skills/record/start", json={"cycle_dt_ms": 10})
    assert resp.status_code == 200
    await asyncio.sleep(0.05)
    resp = await client.post(
        "/skills/record/stop",
        json={"name": "wave", "description": "test wave", "license": "CC0", "cycle_dt_ms": 10},
    )
    assert resp.status_code == 200
    manifest = resp.json()
    assert manifest["name"] == "wave"
    assert manifest["frame_count"] > 0

    resp = await client.get("/skills")
    assert any(s["name"] == "wave" for s in resp.json()["skills"])

    resp = await client.get("/skills/wave/export")
    assert resp.status_code == 200
    exported = resp.json()
    assert exported["manifest"]["name"] == "wave"

    resp = await client.get("/skills/nonexistent/export")
    assert resp.status_code == 404


async def test_skill_import_retarget_report(client):
    skill = {
        "manifest": {
            "name": "imported-skill",
            "description": "",
            "author_robot_id": "other-bot",
            "joint_names": ["fl_hip", "made_up_joint"],
            "frame_count": 1,
            "cycle_dt_ms": 100,
            "license": "CC0",
            "provenance_batch_root": "",
            "created_at_ms": 0,
        },
        "keyframes": [[0.2, 0.3]],
    }
    resp = await client.post("/skills/import", json=skill)
    assert resp.status_code == 200
    data = resp.json()
    assert data["name"] == "imported-skill"
    report = data["retarget_report"]
    assert "fl_hip" in report["matched_joints"]
    assert "made_up_joint" in report["missing_in_target"]
    assert 0.0 < report["coverage"] < 1.0


async def test_skill_play_completes_within_clamp(client, app):
    _inject_skill(app, "safe-move", ["fl_hip"], [[0.3], [0.6]], cycle_dt_ms=10)
    resp = await client.post("/skills/safe-move/play")
    assert resp.status_code == 200
    assert resp.json()["status"] == "playing"

    await asyncio.sleep(0.2)
    resp = await client.get("/skills/playback/status")
    status = resp.json()
    assert status["name"] == "safe-move"
    assert status["status"] == "completed"
    assert status["frames_executed"] == 2
    assert status["rejected_at_frame"] is None


async def test_skill_play_rejects_over_clamp(client, app):
    _inject_skill(app, "unsafe-jump", ["fl_hip"], [[1.5]], cycle_dt_ms=10)
    resp = await client.post("/skills/unsafe-jump/play")
    assert resp.status_code == 200

    await asyncio.sleep(0.1)
    resp = await client.get("/skills/playback/status")
    status = resp.json()
    assert status["name"] == "unsafe-jump"
    assert status["status"] == "rejected"
    assert status["rejected_at_frame"] == 0
    assert "safety clamp" in status["reason"]
    assert status["frames_executed"] == 0


async def test_skill_play_not_found(client):
    resp = await client.post("/skills/nonexistent/play")
    assert resp.status_code == 404


async def test_skill_registry_msg(client, app):
    _inject_skill(app, "bow", ["fl_hip"], [[0.1]])
    resp = await client.get("/skills/bow/registry_msg")
    assert resp.status_code == 200
    data = resp.json()
    msg = data["execute_msg"]["publish_skill"]
    assert msg["dapp_name"] == "bow"
    assert msg["chain_id"] == "juno-1"
    assert "testnet" in data["contract_addresses"]
    assert "mainnet" in data["contract_addresses"]

    expected_hash = hashlib.sha256(
        json.dumps(app.state.bridge.skills["bow"], sort_keys=True).encode()
    ).hexdigest()
    assert msg["skill_hash"] == expected_hash
    assert msg["skill_uri"].endswith("/skills/bow/export")

    resp = await client.get("/skills/nonexistent/registry_msg")
    assert resp.status_code == 404


async def test_skill_marketplace_msg(client, app):
    _inject_skill(app, "spin", ["fl_hip"], [[0.1]])
    resp = await client.get("/skills/spin/marketplace_msg?price_ujuno=1000000&description=spin+in+place")
    assert resp.status_code == 200
    data = resp.json()
    assert data["marketplace_deployed"] is False
    msg = data["execute_msg"]["list_service"]
    assert msg["skill_ref"] == "spin"
    assert msg["price"] == "1000000"
    assert msg["description"] == "spin in place"

    resp = await client.get("/skills/nonexistent/marketplace_msg")
    assert resp.status_code == 404


async def test_register_robot_with_taught_skill(client, app):
    _inject_skill(app, "sit", ["fl_hip"], [[0.1]])
    resp = await client.post("/robot/register")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ready_to_register"
    entries = data["skill_registry"]["skills"]
    assert len(entries) == 1
    assert entries[0]["dapp_name"] == "sit"
    assert "registry_msg_url" in entries[0]
    assert "marketplace_msg_url" in entries[0]
    assert data["marketplace"]["deployed"] is False
