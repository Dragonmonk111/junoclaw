"""Phase G8: train a commandable speed + heading PPO policy in MuJoCo.

The most useful single policy for real deployment — accepts [speed, heading]
commands as part of the 43-dim observation. Warm-starts from the walk policy
(ppo_walk.zip) so the agent doesn't relearn the basic gait.

Observation: 43-dim (41 base + commanded_speed + commanded_heading)
Action: 15-dim joint targets in [-1, 1]

Usage:
    python train_speed_heading.py --timesteps 2000000 --n-envs 8 --resume checkpoints/ppo_walk.zip
"""

from __future__ import annotations

import argparse
import os

from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env
from stable_baselines3.common.vec_env import SubprocVecEnv

from env import DogzillaSpeedHeadingEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timesteps", type=int, default=2_000_000)
    parser.add_argument("--n-envs", type=int, default=8)
    parser.add_argument("--check-only", action="store_true", help="Just validate the env and exit")
    parser.add_argument("--resume", default=os.path.join(CHECKPOINT_DIR, "ppo_walk.zip"),
                        help="Checkpoint to warm-start from (default: ppo_walk.zip)")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaSpeedHeadingEnv())
        print("Env check passed.")
        return

    vec_env = make_vec_env(lambda: DogzillaSpeedHeadingEnv(), n_envs=args.n_envs, vec_env_cls=SubprocVecEnv)

    if args.resume and os.path.exists(args.resume):
        try:
            model = PPO.load(args.resume, env=vec_env, device="cpu")
            print(f"Warm-started from {args.resume}")
        except ValueError as e:
            if "Observation spaces do not match" in str(e):
                print(f"Obs space mismatch (expected: 43-dim, checkpoint: 41-dim). Training from scratch.")
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
            else:
                raise
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

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_speed_heading")
    checkpoint_every = max(args.timesteps // 10, 50_000)
    remaining = args.timesteps
    while remaining > 0:
        chunk = min(checkpoint_every, remaining)
        model.learn(total_timesteps=chunk, progress_bar=True, reset_num_timesteps=False)
        remaining -= chunk
        model.save(out_path)
        print(f"Checkpoint saved to {out_path}.zip ({args.timesteps - remaining}/{args.timesteps} steps)")

    print(f"Done. Final checkpoint: {out_path}.zip")
    print(f"Next: python export_onnx.py --checkpoint {out_path}.zip --out checkpoints/policy_speed_heading.onnx")


if __name__ == "__main__":
    main()
