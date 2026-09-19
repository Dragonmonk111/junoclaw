"""Live simulate-mode test of the trained walk ONNX policy against the
ROS2 bridge. Starts the bridge in simulate mode, pre-positions the robot
to a standing pose, loads the real policy_walk.onnx, runs the inference
loop for N steps, and logs joint trajectories + safety clamp outcomes.

Also includes an --unpack mode that inspects the ONNX model internals
(input/output shapes, node count, operator types, parameter count).

Usage:
    # Run the policy in simulate mode for 200 steps
    python test_policy_sim.py --steps 200

    # Inspect the ONNX model without running inference
    python test_policy_sim.py --unpack

    # Both
    python test_policy_sim.py --unpack --steps 200
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
import time

# Add bridge src to path
BRIDGE_SRC = os.path.join(
    os.path.dirname(__file__), "..", "plugins", "plugin-ros2", "bridge", "src"
)
sys.path.insert(0, BRIDGE_SRC)

ONNX_PATH = os.path.join(os.path.dirname(__file__), "checkpoints", "policy_walk.onnx")

# Standing pose — matches MJCF keyframe "stand" qpos exactly:
#   hip=0, thigh=0.5, calf=-0.9, arm=0
# The policy was trained from this pose; setting thighs to 0.0 instead
# of 0.5 puts the observation far out of distribution and the first
# inference produces a huge action that trips the safety clamp.
STAND_POSE = {
    "fl_hip": 0.0, "fl_thigh": 0.5, "fl_calf": -0.9,
    "fr_hip": 0.0, "fr_thigh": 0.5, "fr_calf": -0.9,
    "rl_hip": 0.0, "rl_thigh": 0.5, "rl_calf": -0.9,
    "rr_hip": 0.0, "rr_thigh": 0.5, "rr_calf": -0.9,
    "arm_base": 0.0, "arm_shoulder": 0.0, "arm_gripper": 0.0,
}

QUADRUPED_JOINTS = [
    "fl_hip", "fl_thigh", "fl_calf",
    "fr_hip", "fr_thigh", "fr_calf",
    "rl_hip", "rl_thigh", "rl_calf",
    "rr_hip", "rr_thigh", "rr_calf",
    "arm_base", "arm_shoulder", "arm_gripper",
]


def unpack_onnx(onnx_path: str) -> None:
    """Inspect ONNX model internals: I/O shapes, graph nodes, op types,
    parameter count, and a per-layer weight summary.
    """
    print(f"\n{'='*60}")
    print(f"ONNX Model Inspection: {onnx_path}")
    print(f"{'='*60}")

    import onnx

    model = onnx.load(onnx_path)
    graph = model.graph

    # File size
    fsize = os.path.getsize(onnx_path)
    data_file = onnx_path + ".data"
    data_size = os.path.getsize(data_file) if os.path.exists(data_file) else 0
    print(f"\nFile sizes:")
    print(f"  .onnx (graph):  {fsize:>10,} bytes")
    print(f"  .onnx.data:     {data_size:>10,} bytes")
    print(f"  Total:          {fsize + data_size:>10,} bytes")

    # Inputs
    print(f"\nInputs ({len(graph.input)}):")
    for inp in graph.input:
        dims = [d.dim_value if d.dim_value else d.dim_param for d in inp.type.tensor_type.shape.dim]
        print(f"  {inp.name}: shape={dims}, dtype={inp.type.tensor_type.elem_type}")

    # Outputs
    print(f"\nOutputs ({len(graph.output)}):")
    for out in graph.output:
        dims = [d.dim_value if d.dim_value else d.dim_param for d in out.type.tensor_type.shape.dim]
        print(f"  {out.name}: shape={dims}, dtype={out.type.tensor_type.elem_type}")

    # Node statistics
    op_counts: dict[str, int] = {}
    for node in graph.node:
        op_counts[node.op_type] = op_counts.get(node.op_type, 0) + 1

    print(f"\nGraph nodes: {len(graph.node)} total")
    for op_type, count in sorted(op_counts.items(), key=lambda x: -x[1]):
        print(f"  {op_type:20s}: {count}")

    # Initializers (weights/biases)
    total_params = 0
    print(f"\nInitializers (weights/biases): {len(graph.initializer)}")
    for init in graph.initializer:
        numel = 1
        for d in init.dims:
            numel *= d
        total_params += numel
        print(f"  {init.name:40s}: shape={list(init.dims)}, numel={numel:,}")
    print(f"  Total parameters: {total_params:,}")

    # Opset
    print(f"\nOpset imports:")
    for opset in model.opset_import:
        print(f"  domain='{opset.domain or 'ai.onnx'}', version={opset.version}")

    print(f"{'='*60}\n")


async def run_policy_test(onnx_path: str, steps: int, hz: float) -> None:
    """Start bridge in simulate mode, load policy, run inference loop,
    and log joint trajectories.
    """
    from junoclaw_ros2_bridge.server import create_app
    from fastapi.testclient import TestClient

    print(f"\n{'='*60}")
    print(f"Simulate-Mode Policy Test")
    print(f"  ONNX: {onnx_path}")
    print(f"  Steps: {steps}")
    print(f"  Hz: {hz}")
    print(f"{'='*60}\n")

    app = create_app(
        robot_id="test-robot",
        simulate=True,
        robot_type="quadruped",
    )

    trajectory_log: list[dict] = []

    with TestClient(app) as client:
        # 1. Health check
        r = client.get("/health")
        print(f"Health: {r.json()}")

        # 2. Pre-position to standing pose via batch endpoint
        print("\nPre-positioning to standing pose (thigh=0.5, calf=-0.9)...")
        r = client.post("/robot/joint_commands", json={"joints": STAND_POSE})
        print(f"  Batch set response: {r.status_code} {r.json()}")

        # 3. Verify state via WebSocket
        import websockets
        import threading
        import queue

        state_q: queue.Queue = queue.Queue()

        def _ws_listener():
            loop = asyncio.new_event_loop()
            loop.run_until_complete(_ws_recv(state_q))

        async def _ws_recv(q: queue.Queue):
            import urllib.parse
            ws_url = "ws://127.0.0.1:8080/ws/state"
            try:
                async with websockets.connect(ws_url) as ws:
                    while True:
                        msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                        q.put(json.loads(msg))
                        break  # just need one snapshot
            except Exception as e:
                q.put({"error": str(e)})

        # Can't easily run WS in same thread as TestClient — use app's bridge directly
        # Access bridge via the app's internal state
        # The bridge is captured in closures; we'll just use joint_commands
        # and trust the internal state tracking.
        # Instead, let's read state via the WebSocket sync client
        try:
            with client.websocket_connect("/ws/state") as ws:
                state = ws.receive_json()
                joints = state.get("joints", {})
                print(f"\nRobot state after pre-positioning (via WS):")
                for jname in QUADRUPED_JOINTS:
                    print(f"  {jname:>16}: {joints.get(jname, 0.0):>8.4f}")
        except Exception as e:
            print(f"  (WS state read failed: {e})")

        # 4. Load the ONNX policy
        print(f"\nLoading ONNX policy: {onnx_path}")
        r = client.post("/robot/policy/load", json={"onnx_path": onnx_path})
        print(f"  Load response: {r.status_code} {r.json()}")
        if r.status_code != 200:
            print("  FAILED to load policy.")
            return

        # 5. Start the policy loop
        print(f"\nStarting policy inference at {hz} Hz...")
        r = client.post("/robot/policy/start", json={"hz": hz, "smoothing": 0.2})
        print(f"  Start response: {r.status_code} {r.json()}")

        # 6. Run for the requested number of steps, logging trajectories
        period = 1.0 / hz
        print(f"\nRunning {steps} steps (logging key joints)...")
        print(f"{'step':>5} | {'fl_thigh':>10} | {'fr_thigh':>10} | {'rl_thigh':>10} | {'rr_thigh':>10} | {'fl_calf':>10} | status")
        print("-" * 90)

        for i in range(steps):
            await asyncio.sleep(period)

            # Check policy status
            r = client.get("/robot/policy/status")
            status = r.json()

            if status.get("status") == "rejected":
                print(f"\n  *** SAFETY CLAMP REJECTED at step {status.get('rejected_at_step')} ***")
                print(f"  Reason: {status.get('reason')}")
                print(f"  Steps run before rejection: {status.get('steps_run')}")
                break

            # Read state via WebSocket
            try:
                with client.websocket_connect("/ws/state") as ws:
                    state = ws.receive_json()
                    joints = state.get("joints", {})
            except Exception:
                joints = {}

            # Log key joints
            if i < 30 or i % 50 == 0:
                vals = [joints.get(jn, 0.0) for jn in ["fl_thigh", "fr_thigh", "rl_thigh", "rr_thigh", "fl_calf"]]
                print(f"{i:>5} | {vals[0]:>10.4f} | {vals[1]:>10.4f} | {vals[2]:>10.4f} | {vals[3]:>10.4f} | {vals[4]:>10.4f} | {status.get('status', '?')}")

                snapshot = {
                    "step": i,
                    "joint_positions": dict(joints),
                    "policy_status": dict(status),
                }
                trajectory_log.append(snapshot)

        # 7. Stop the policy
        print(f"\nStopping policy...")
        r = client.post("/robot/policy/stop")
        print(f"  Stop response: {r.status_code} {r.json()}")

        # 8. Final status
        r = client.get("/robot/policy/status")
        final_status = r.json()
        print(f"\nFinal policy status:")
        print(f"  {json.dumps(final_status, indent=2)}")

    # 9. Save trajectory log
    log_path = os.path.join(os.path.dirname(__file__), "test_policy_sim_log.json")
    with open(log_path, "w") as f:
        json.dump(trajectory_log, f, indent=2)
    print(f"\nTrajectory log saved to: {log_path}")

    # 10. Summary analysis
    if trajectory_log:
        print(f"\n{'='*60}")
        print("Trajectory Analysis Summary")
        print(f"{'='*60}")
        first = trajectory_log[0]["joint_positions"]
        last = trajectory_log[-1]["joint_positions"]
        print(f"\nJoint position drift (first -> last logged step):")
        for jname in QUADRUPED_JOINTS:
            v0 = first.get(jname, 0.0)
            v1 = last.get(jname, 0.0)
            drift = v1 - v0
            marker = " ***" if abs(drift) > 0.3 else ""
            print(f"  {jname:>16}: {v0:>8.4f} -> {v1:>8.4f} (drift={drift:+.4f}){marker}")

        # Check for oscillation (direction reversals in thigh joints)
        thigh_joints = ["fl_thigh", "fr_thigh", "rl_thigh", "rr_thigh"]
        print(f"\nOscillation check (thigh joints — direction reversals):")
        for jname in thigh_joints:
            values = [snap["joint_positions"].get(jname, 0.0) for snap in trajectory_log]
            if len(values) >= 3:
                reversals = sum(
                    1 for k in range(2, len(values))
                    if (values[k] - values[k-1]) * (values[k-1] - values[k-2]) < 0
                )
                vrange = max(values) - min(values) if values else 0.0
                print(f"  {jname}: {reversals} reversals, range={vrange:.4f} rad, over {len(values)} samples")

        # Check if policy is producing motion (not frozen)
        all_values = []
        for snap in trajectory_log:
            for jname in thigh_joints:
                all_values.append(snap["joint_positions"].get(jname, 0.0))
        if all_values:
            total_motion = max(all_values) - min(all_values)
            print(f"\nOverall thigh motion range: {total_motion:.4f} rad")
            if total_motion < 0.01:
                print("  WARNING: Policy appears frozen (no motion detected)")
            elif total_motion > 1.0:
                print("  NOTE: Large motion range — verify this is expected gait behavior")

    print(f"\n{'='*60}\n")


def main():
    parser = argparse.ArgumentParser(description="Test walk ONNX policy in simulate mode")
    parser.add_argument("--onnx", default=ONNX_PATH, help="Path to ONNX policy file")
    parser.add_argument("--steps", type=int, default=200, help="Number of inference steps to run")
    parser.add_argument("--hz", type=float, default=30.0, help="Inference frequency")
    parser.add_argument("--unpack", action="store_true", help="Inspect ONNX model internals and exit")
    args = parser.parse_args()

    if args.unpack:
        unpack_onnx(args.onnx)
        if args.steps == 0:
            return

    if args.steps > 0:
        asyncio.run(run_policy_test(args.onnx, args.steps, args.hz))


if __name__ == "__main__":
    main()
