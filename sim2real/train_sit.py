"""Phase G6: train a sit/lie-down PPO policy in MuJoCo.

The robot starts standing and must smoothly transition to a folded-leg
sitting pose at half height, then hold it. Uses the same 41-dim obs
and 15-dim action as the stand env.

Usage:
    python train_sit.py --timesteps 500000 --n-envs 8 --resume checkpoints/ppo_stand.zip
"""

from __future__ import annotations

import argparse
import os

from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env
from stable_baselines3.common.vec_env import SubprocVecEnv

from env import DogzillaSitEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timesteps", type=int, default=500_000)
    parser.add_argument("--n-envs", type=int, default=8)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--resume", default=None, help="Path to a checkpoint .zip to warm-start from")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaSitEnv())
        print("Env check passed.")
        return

    vec_env = make_vec_env(
        lambda: DogzillaSitEnv(),
        n_envs=args.n_envs,
        vec_env_cls=SubprocVecEnv,
    )

    if args.resume and os.path.exists(args.resume):
        model = PPO.load(args.resume, env=vec_env, device="cpu")
        print(f"Warm-started from {args.resume}")
    else:
        print("Training from scratch.")
        model = PPO(
            "MlpPolicy",
            vec_env,
            policy_kwargs={"net_arch": [128, 128]},
            n_steps=512,
            batch_size=256,
            learning_rate=3e-4,
            gamma=0.99,
            verbose=1,
        )

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_sit")
    checkpoint_every = max(args.timesteps // 10, 25_000)
    remaining = args.timesteps
    while remaining > 0:
        chunk = min(checkpoint_every, remaining)
        model.learn(total_timesteps=chunk, progress_bar=True, reset_num_timesteps=False)
        remaining -= chunk
        model.save(out_path)
        print(f"Checkpoint saved to {out_path}.zip ({args.timesteps - remaining}/{args.timesteps} steps)")

    print(f"Done. Final checkpoint: {out_path}.zip")
    print(f"Next: python export_onnx.py --checkpoint {out_path}.zip --out checkpoints/policy_sit.onnx")


if __name__ == "__main__":
    main()
