"""Evaluate a trained SB3 PPO policy (or ONNX policy) in the MuJoCo sim env.

Runs N episodes, reports: mean episode length, mean total reward, fall rate,
mean forward velocity, mean upright score, mean imitation reward.

Usage:
    # Evaluate the walk policy
    python eval_policy.py --checkpoint checkpoints/ppo_walk.zip --env walk --episodes 20

    # Evaluate the turn policy
    python eval_policy.py --checkpoint checkpoints/ppo_turn.zip --env turn --episodes 20

    # Evaluate the recovery policy
    python eval_policy.py --checkpoint checkpoints/ppo_recovery.zip --env recovery --episodes 20

    # Evaluate an ONNX policy directly (no SB3 needed)
    python eval_policy.py --onnx checkpoints/policy_walk.onnx --env walk --episodes 20
"""

from __future__ import annotations

import argparse
import os
import sys

import numpy as np

from env import (
    DogzillaStandEnv,
    DogzillaWalkEnv,
    DogzillaTurnEnv,
    DogzillaRecoveryEnv,
    DogzillaSpeedHeadingEnv,
    DogzillaTerrainEnv,
    DogzillaSitEnv,
    DogzillaStepEnv,
    DogzillaSpeedHeadingTerrainEnv,
)

ENV_REGISTRY = {
    "stand": DogzillaStandEnv,
    "walk": DogzillaWalkEnv,
    "turn": DogzillaTurnEnv,
    "recovery": DogzillaRecoveryEnv,
    "speed_heading": DogzillaSpeedHeadingEnv,
    "terrain": DogzillaTerrainEnv,
    "sit": DogzillaSitEnv,
    "step": DogzillaStepEnv,
    "speed_heading_terrain": DogzillaSpeedHeadingTerrainEnv,
}


def evaluate_sb3(checkpoint: str, env_class, n_episodes: int, seed: int = 42):
    from stable_baselines3 import PPO

    env = env_class(randomize=False)
    model = PPO.load(checkpoint, env=env, device="cpu")

    results = []
    for ep in range(n_episodes):
        obs, info = env.reset(seed=seed + ep)
        total_reward = 0.0
        steps = 0
        terminated = False
        truncated = False
        ep_infos = []

        while not (terminated or truncated):
            action, _ = model.predict(obs, deterministic=True)
            obs, reward, terminated, truncated, info = env.step(action)
            total_reward += reward
            steps += 1
            ep_infos.append(info)

        results.append({
            "episode": ep,
            "steps": steps,
            "total_reward": total_reward,
            "fell": terminated and not ep_infos[-1].get("success", False) if ep_infos else terminated,
            "success": ep_infos[-1].get("success", False) if ep_infos else False,
            "final_upright": ep_infos[-1].get("upright", 0.0) if ep_infos else 0.0,
            "final_height": ep_infos[-1].get("height", 0.0) if ep_infos else 0.0,
            "mean_forward_vel": np.mean([i.get("forward_vel", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_upright": np.mean([i.get("upright", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_vel_tracking": np.mean([i.get("vel_tracking", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_imitation": np.mean([i.get("imitation_reward", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_yaw_error": np.mean([i.get("yaw_error", 0.0) for i in ep_infos]) if ep_infos else 0.0,
        })

    env.close()
    return results


def evaluate_onnx(onnx_path: str, env_class, n_episodes: int, seed: int = 42):
    import onnxruntime as ort

    env = env_class(randomize=False)
    session = ort.InferenceSession(onnx_path)
    input_name = session.get_inputs()[0].name

    results = []
    for ep in range(n_episodes):
        obs, info = env.reset(seed=seed + ep)
        total_reward = 0.0
        steps = 0
        terminated = False
        truncated = False
        ep_infos = []

        while not (terminated or truncated):
            obs_arr = np.asarray([obs], dtype=np.float32)
            action = session.run(None, {input_name: obs_arr})[0][0]
            obs, reward, terminated, truncated, info = env.step(action)
            total_reward += reward
            steps += 1
            ep_infos.append(info)

        results.append({
            "episode": ep,
            "steps": steps,
            "total_reward": total_reward,
            "fell": terminated and not ep_infos[-1].get("success", False) if ep_infos else terminated,
            "success": ep_infos[-1].get("success", False) if ep_infos else False,
            "final_upright": ep_infos[-1].get("upright", 0.0) if ep_infos else 0.0,
            "final_height": ep_infos[-1].get("height", 0.0) if ep_infos else 0.0,
            "mean_forward_vel": np.mean([i.get("forward_vel", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_upright": np.mean([i.get("upright", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_vel_tracking": np.mean([i.get("vel_tracking", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_imitation": np.mean([i.get("imitation_reward", 0.0) for i in ep_infos]) if ep_infos else 0.0,
            "mean_yaw_error": np.mean([i.get("yaw_error", 0.0) for i in ep_infos]) if ep_infos else 0.0,
        })

    env.close()
    return results


def print_report(results: list[dict], env_name: str):
    n = len(results)
    fell = sum(1 for r in results if r["fell"])
    succeeded = sum(1 for r in results if r.get("success", False))
    survived = n - fell

    print(f"\n{'='*60}")
    print(f"Evaluation Report: {env_name} ({n} episodes)")
    print(f"{'='*60}")
    print(f"  Fall rate:          {fell}/{n} ({100*fell/n:.1f}%)")
    print(f"  Survival rate:      {survived}/{n} ({100*survived/n:.1f}%)")
    if any("success" in r for r in results):
        print(f"  Success rate:       {succeeded}/{n} ({100*succeeded/n:.1f}%)")
    print(f"  Mean episode length: {np.mean([r['steps'] for r in results]):.1f} steps")
    print(f"  Mean total reward:   {np.mean([r['total_reward'] for r in results]):.2f}")
    print(f"  Mean upright score:  {np.mean([r['mean_upright'] for r in results]):.3f}")
    print(f"  Mean final height:   {np.mean([r['final_height'] for r in results]):.4f} m")

    if any("forward_vel" in r and r["mean_forward_vel"] != 0.0 for r in results):
        print(f"  Mean forward vel:    {np.mean([r['mean_forward_vel'] for r in results]):.4f} m/s")
    if any("vel_tracking" in r and r["mean_vel_tracking"] != 0.0 for r in results):
        print(f"  Mean vel tracking:   {np.mean([r['mean_vel_tracking'] for r in results]):.3f}")
    if any("imitation_reward" in r and r["mean_imitation"] != 0.0 for r in results):
        print(f"  Mean imitation:      {np.mean([r['mean_imitation'] for r in results]):.3f}")
    if any("yaw_error" in r and r["mean_yaw_error"] != 0.0 for r in results):
        print(f"  Mean yaw error:      {np.mean([r['mean_yaw_error'] for r in results]):.3f} rad")

    has_success = any("success" in r for r in results)
    if has_success:
        print(f"\n  Per-episode summary:")
        print(f"  {'ep':>3} | {'steps':>5} | {'reward':>8} | {'success':>7} | {'upright':>8} | {'height':>8} | {'fwd_vel':>8}")
        print(f"  {'-'*60}")
        for r in results:
            print(f"  {r['episode']:>3} | {r['steps']:>5} | {r['total_reward']:>8.2f} | {str(r.get('success', False)):>7} | {r['mean_upright']:>8.3f} | {r['final_height']:>8.4f} | {r['mean_forward_vel']:>8.4f}")
    else:
        print(f"\n  Per-episode summary:")
        print(f"  {'ep':>3} | {'steps':>5} | {'reward':>8} | {'fell':>5} | {'upright':>8} | {'height':>8} | {'fwd_vel':>8}")
        print(f"  {'-'*55}")
        for r in results:
            print(f"  {r['episode']:>3} | {r['steps']:>5} | {r['total_reward']:>8.2f} | {str(r['fell']):>5} | {r['mean_upright']:>8.3f} | {r['final_height']:>8.4f} | {r['mean_forward_vel']:>8.4f}")

    print(f"{'='*60}\n")


def main():
    parser = argparse.ArgumentParser(description="Evaluate a trained policy in MuJoCo sim")
    parser.add_argument("--checkpoint", default=None, help="Path to SB3 PPO .zip checkpoint")
    parser.add_argument("--onnx", default=None, help="Path to ONNX policy (alternative to --checkpoint)")
    parser.add_argument("--env", required=True, choices=list(ENV_REGISTRY.keys()), help="Environment to evaluate in")
    parser.add_argument("--episodes", type=int, default=20, help="Number of evaluation episodes")
    parser.add_argument("--seed", type=int, default=42, help="Base random seed")
    args = parser.parse_args()

    env_class = ENV_REGISTRY[args.env]

    if args.onnx:
        print(f"Evaluating ONNX policy: {args.onnx}")
        results = evaluate_onnx(args.onnx, env_class, args.episodes, args.seed)
    elif args.checkpoint:
        print(f"Evaluating SB3 checkpoint: {args.checkpoint}")
        results = evaluate_sb3(args.checkpoint, env_class, args.episodes, args.seed)
    else:
        print("Error: must provide either --checkpoint or --onnx")
        sys.exit(1)

    print_report(results, args.env)


if __name__ == "__main__":
    main()
