#!/usr/bin/env python3
"""MuJoCo Simulation HTTP Server — exposes the Dogzilla recovery env to MCP tools.

Run: python sim_server.py [--port 8765]

Endpoints:
  GET  /info       — server status, model info
  POST /reset      — reset env with optional tilt params → {observation, info}
  POST /step       — step with action vector → {observation, reward, terminated, truncated, info}
  GET  /observe    — current observation without stepping → {observation, info}
  POST /eval       — evaluate a policy over N episodes → {results}

This is a thin wrapper around DogzillaRecoveryEnv. The heavy logic (reward,
termination, observation construction) stays in env.py.
"""

import argparse
import json
import os
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler

import numpy as np
import mujoco

# Ensure env.py is importable
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from env import DogzillaRecoveryEnv, STAND_HEIGHT  # noqa: E402

# Global env instance (recreated on reset with different tilt params)
_env: DogzillaRecoveryEnv | None = None
_env_params: dict = {}


def get_or_create_env(params: dict) -> DogzillaRecoveryEnv:
    global _env, _env_params
    tilt_min = params.get("tilt_min")
    tilt_max = params.get("tilt_max")
    if tilt_min is not None:
        tilt_min = float(tilt_min)
    if tilt_max is not None:
        tilt_max = float(tilt_max)

    # Recreate if params changed or no env yet
    if _env is None or _env_params.get("tilt_min") != tilt_min or _env_params.get("tilt_max") != tilt_max:
        kwargs = {}
        if tilt_min is not None and tilt_max is not None:
            kwargs["curriculum_tilt_min"] = tilt_min
            kwargs["curriculum_tilt_max"] = tilt_max
        _env = DogzillaRecoveryEnv(**kwargs)
        _env_params = {"tilt_min": tilt_min, "tilt_max": tilt_max}

    return _env


def obs_to_list(obs: np.ndarray) -> list:
    return np.asarray(obs, dtype=np.float64).tolist()


class SimHandler(BaseHTTPRequestHandler):
    def _send_json(self, data: dict, status: int = 200):
        body = json.dumps(data, default=str).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _read_body(self) -> dict:
        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length))

    def do_GET(self):
        if self.path == "/info":
            self._send_json({
                "ok": True,
                "model": "dogzilla_lite",
                "stand_height": STAND_HEIGHT,
                "action_dim": 15,
                "observation_dim": 41,
                "env_loaded": _env is not None,
                "env_params": _env_params,
            })
        elif self.path == "/observe":
            if _env is None:
                self._send_json({"ok": False, "error": "No env — call /reset first"})
                return
            obs = _env._get_obs()
            info = {
                "upright": float(_env._upright_score()),
                "height": float(_env.data.qpos[2]),
                "step_count": _env._step_count,
            }
            self._send_json({"ok": True, "observation": obs_to_list(obs), "info": info})
        else:
            self._send_json({"ok": False, "error": f"Unknown path: {self.path}"}, 404)

    def do_POST(self):
        try:
            body = self._read_body()
        except json.JSONDecodeError as e:
            self._send_json({"ok": False, "error": f"Invalid JSON: {e}"}, 400)
            return

        if self.path == "/reset":
            try:
                env = get_or_create_env(body)
                seed = body.get("seed")
                kwargs = {}
                if seed is not None:
                    kwargs["seed"] = int(seed)

                # If tilt_degrees is specified, override the random tilt
                tilt_deg = body.get("tilt_degrees")
                if tilt_deg is not None:
                    tilt_rad = np.radians(float(tilt_deg))
                    obs, info = env.reset(**kwargs)
                    # Override the quaternion to the specific tilt
                    env.data.qpos[3:7] = [np.cos(tilt_rad / 2), 0, np.sin(tilt_rad / 2), 0]
                    mujoco.mj_forward(env.model, env.data)
                    obs = env._get_obs()
                    u = float(env._upright_score())
                    env._initial_upright = u
                else:
                    obs, info = env.reset(**kwargs)
                    u = float(env._upright_score())

                self._send_json({
                    "ok": True,
                    "observation": obs_to_list(obs),
                    "info": {
                        "upright": u,
                        "height": float(env.data.qpos[2]),
                        "initial_upright": float(env._initial_upright),
                        "step_count": 0,
                    },
                })
            except Exception as e:
                self._send_json({"ok": False, "error": str(e)}, 500)

        elif self.path == "/step":
            if _env is None:
                self._send_json({"ok": False, "error": "No env — call /reset first"})
                return
            try:
                action = body.get("action")
                if action is None:
                    # Use zero action (neutral pose)
                    action = np.zeros(15)
                else:
                    action = np.array(action, dtype=np.float32)
                    if len(action) != 15:
                        self._send_json({"ok": False, "error": f"Action must be 15-dim, got {len(action)}"}, 400)
                        return

                obs, reward, terminated, truncated, info = _env.step(action)
                u = float(_env._upright_score())
                self._send_json({
                    "ok": True,
                    "observation": obs_to_list(obs),
                    "reward": float(reward),
                    "terminated": bool(terminated),
                    "truncated": bool(truncated),
                    "info": {
                        "upright": u,
                        "height": float(_env.data.qpos[2]),
                        "step_count": _env._step_count,
                        "upright_steps": _env._upright_steps,
                        "success": bool(terminated and _env._upright_steps >= 5),
                    },
                })
            except Exception as e:
                self._send_json({"ok": False, "error": str(e)}, 500)

        elif self.path == "/eval":
            try:
                from stable_baselines3 import PPO

                checkpoint = body.get("checkpoint", "checkpoints/ppo_recovery.zip")
                tilt_min = body.get("tilt_min", 60)
                tilt_max = body.get("tilt_max", 150)
                n_episodes = int(body.get("n_episodes", 20))

                model = PPO.load(checkpoint, device="cpu")
                tilt_min_rad = np.radians(float(tilt_min))
                tilt_max_rad = np.radians(float(tilt_max))
                env = DogzillaRecoveryEnv(
                    curriculum_tilt_min=tilt_min_rad,
                    curriculum_tilt_max=tilt_max_rad,
                )

                successes = 0
                best_uprights = []
                ep_lengths = []
                for i in range(n_episodes):
                    obs, _ = env.reset(seed=i)
                    best_u = -1
                    ep_len = 0
                    for t in range(200):
                        action, _ = model.predict(obs, deterministic=True)
                        obs, reward, terminated, truncated, info = env.step(action)
                        u = info.get("upright", 0)
                        best_u = max(best_u, u)
                        ep_len = t + 1
                        if terminated or truncated:
                            break
                    if env._upright_steps >= 5:
                        successes += 1
                    best_uprights.append(best_u)
                    ep_lengths.append(ep_len)

                self._send_json({
                    "ok": True,
                    "results": {
                        "n_episodes": n_episodes,
                        "successes": successes,
                        "success_rate": successes / n_episodes,
                        "tilt_range": [tilt_min, tilt_max],
                        "best_uprights": [round(u, 3) for u in best_uprights],
                        "ep_lengths": ep_lengths,
                        "mean_ep_length": sum(ep_lengths) / len(ep_lengths),
                        "mean_best_upright": sum(best_uprights) / len(best_uprights),
                    },
                })
            except Exception as e:
                self._send_json({"ok": False, "error": str(e)}, 500)

        else:
            self._send_json({"ok": False, "error": f"Unknown path: {self.path}"}, 404)

    def log_message(self, format, *args):
        # Suppress default logging; uncomment for debug
        # print(f"[sim_server] {args[0]}")
        pass


def main():
    parser = argparse.ArgumentParser(description="MuJoCo Sim Server for MCP tools")
    parser.add_argument("--port", type=int, default=8765, help="Port to listen on")
    args = parser.parse_args()

    server = HTTPServer(("127.0.0.1", args.port), SimHandler)
    print(f"[sim_server] listening on http://127.0.0.1:{args.port}")
    print(f"[sim_server] model: dogzilla_lite, action_dim=15, obs_dim=41")
    print(f"[sim_server] endpoints: GET /info, POST /reset, POST /step, GET /observe, POST /eval")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[sim_server] shutting down")
        server.server_close()


if __name__ == "__main__":
    main()
