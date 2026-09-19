"""Phase G2: train a terrain-adaptive walk PPO policy in MuJoCo.

Warm-starts from the walk policy (ppo_walk.zip) and trains on the
DogzillaTerrainEnv which adds friction variations, random external
forces, and lateral impulses to simulate uneven ground.

Usage:
    python train_terrain.py --timesteps 2000000 --n-envs 8 --resume checkpoints/ppo_walk.zip
"""

from __future__ import annotations

import argparse
import os

from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env
from stable_baselines3.common.vec_env import SubprocVecEnv

from env import DogzillaTerrainEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timesteps", type=int, default=2_000_000)
    parser.add_argument("--n-envs", type=int, default=8)
    parser.add_argument("--check-only", action="store_true", help="Just validate the env and exit")
    parser.add_argument("--resume", default=None, help="Path to a checkpoint .zip to warm-start from")
    parser.add_argument("--roughness", type=float, default=0.015, help="Terrain perturbation magnitude")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaTerrainEnv(terrain_roughness=args.roughness))
        print("Env check passed.")
        return

    vec_env = make_vec_env(
        lambda: DogzillaTerrainEnv(terrain_roughness=args.roughness),
        n_envs=args.n_envs,
        vec_env_cls=SubprocVecEnv,
    )

    if args.resume and os.path.exists(args.resume):
        model = PPO.load(args.resume, env=vec_env, device="cpu")
        print(f"Warm-started from {args.resume}")
    else:
        print(f"No checkpoint found at {args.resume}, training from scratch.")
        model = PPO(
            "MlpPolicy",
            vec_env,
            policy_kwargs={"net_arch": [256, 256]},
            n_steps=512,
            batch_size=256,
            learning_rate=3e-4,
            gamma=0.99,
            verbose=1,
        )

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_terrain")
    checkpoint_every = max(args.timesteps // 10, 25_000)
    remaining = args.timesteps
    while remaining > 0:
        chunk = min(checkpoint_every, remaining)
        model.learn(total_timesteps=chunk, progress_bar=True, reset_num_timesteps=False)
        remaining -= chunk
        model.save(out_path)
        print(f"Checkpoint saved to {out_path}.zip ({args.timesteps - remaining}/{args.timesteps} steps)")

    print(f"Done. Final checkpoint: {out_path}.zip")
    print(f"Next: python export_onnx.py --checkpoint {out_path}.zip --out checkpoints/policy_terrain.onnx")


if __name__ == "__main__":
    main()
