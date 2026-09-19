"""Quick test: run ONNX policy directly in MuJoCo sim to see if it walks.

Usage:
    python test_policy_in_sim.py --policy checkpoints/policy_speed_heading_terrain.onnx --steps 200
"""

from __future__ import annotations

import argparse
import os
import numpy as np
import mujoco

try:
    import onnxruntime as ort
except ImportError:
    ort = None

from env import (
    DogzillaSpeedHeadingTerrainEnv,
    DogzillaWalkEnv,
    DogzillaSpeedHeadingEnv,
    make_walk_env,
    make_speed_heading_env,
    make_speed_heading_terrain_env,
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", required=True, help="Path to ONNX policy")
    parser.add_argument("--steps", type=int, default=200)
    parser.add_argument("--speed", type=float, default=0.08)
    parser.add_argument("--heading", type=float, default=0.0)
    args = parser.parse_args()

    # Pick env based on obs dim
    sess = ort.InferenceSession(args.policy)
    obs_dim = sess.get_inputs()[0].shape[1]
    print(f"Policy obs dim: {obs_dim}")

    if obs_dim == 43:
        env = make_speed_heading_terrain_env()
    elif obs_dim == 41:
        env = make_walk_env()
    else:
        print(f"Unknown obs dim {obs_dim}")
        return

    obs, info = env.reset()
    print(f"Env reset. Obs shape: {obs.shape}")

    # Override commanded speed/heading if 43-dim
    if obs_dim == 43 and hasattr(env, '_commanded_speed'):
        env._commanded_speed = args.speed
        env._commanded_heading = args.heading + env._initial_heading
        obs = env._get_obs()

    action_history = []
    vel_history = []

    for step in range(args.steps):
        input_name = sess.get_inputs()[0].name
        action = sess.run(None, {input_name: obs.reshape(1, -1)})[0][0]
        action = np.clip(action, -1.0, 1.0)

        action_history.append(action.copy())

        obs, reward, terminated, truncated, info = env.step(action)

        vel_history.append(info.get("forward_vel", 0.0))

        if step < 10 or step % 25 == 0:
            act_str = np.array2string(action, precision=3, separator=',', suppress_small=True)
            print(
                f"  step {step:4d} | action=[{act_str}] | "
                f"vel={info.get('forward_vel', 0.0):.4f} | "
                f"upright={info.get('upright', 0.0):.3f} | "
                f"height={info.get('height', 0.0):.3f} | "
                f"imitation={info.get('imitation_reward', 0.0):.3f}"
            )

        if terminated:
            print(f"  FELL at step {step}")
            break
        if truncated:
            print(f"  Truncated at step {step}")
            break

    # Analysis
    actions = np.array(action_history)
    vels = np.array(vel_history)

    print(f"\n{'='*60}")
    print("ANALYSIS")
    print(f"{'='*60}")

    # Per-leg action variation over time (std dev)
    leg_names = ["FL", "FR", "RL", "RR", "ARM"]
    for i, name in enumerate(leg_names):
        leg_actions = actions[:, i*3:(i+1)*3]
        std = np.std(leg_actions, axis=0)
        mean = np.mean(leg_actions, axis=0)
        print(f"  {name}: mean={np.array2string(mean, precision=3, separator=',')} std={np.array2string(std, precision=3, separator=',')}")

    # Check for oscillation (direction reversals in action)
    print(f"\nOscillation check (action reversals per joint):")
    for j in range(15):
        vals = actions[:, j]
        if len(vals) >= 3:
            reversals = sum(
                1 for k in range(2, len(vals))
                if (vals[k] - vals[k-1]) * (vals[k-1] - vals[k-2]) < 0
            )
            print(f"  joint {j:2d} ({['fl_hip','fl_thigh','fl_calf','fr_hip','fr_thigh','fr_calf','rl_hip','rl_thigh','rl_calf','rr_hip','rr_thigh','rr_calf','arm_b','arm_s','arm_g'][j]}): "
                  f"{reversals} reversals, range={vals.max()-vals.min():.4f}")

    print(f"\nForward velocity: mean={np.mean(vels):.4f}, max={np.max(vels):.4f}, min={np.min(vels):.4f}")
    print(f"Final position x: {env.data.qpos[0]:.4f}")
    print(f"{'='*60}")


if __name__ == "__main__":
    main()
