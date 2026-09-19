"""Phase C: train a standing-only PPO policy in MuJoCo.

This validates the full pipeline (env -> PPO -> checkpoint -> ONNX export)
end-to-end before attempting locomotion (Phase D). See
drafts/PLAN_SIM2REAL_RL_PIPELINE.md.

Usage:
    python train_standing.py --timesteps 100000
"""

from __future__ import annotations

import argparse
import os

from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env

from env import DogzillaStandEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timesteps", type=int, default=100_000)
    parser.add_argument("--n-envs", type=int, default=4)
    parser.add_argument("--check-only", action="store_true", help="Just validate the env and exit")
    parser.add_argument("--tensorboard", action="store_true", help="Enable tensorboard logging (requires `pip install tensorboard`)")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaStandEnv())
        print("Env check passed.")
        return

    vec_env = make_vec_env(lambda: DogzillaStandEnv(), n_envs=args.n_envs)

    model = PPO(
        "MlpPolicy",
        vec_env,
        policy_kwargs={"net_arch": [64, 64]},
        n_steps=512,
        batch_size=256,
        learning_rate=3e-4,
        gamma=0.98,
        verbose=1,
        tensorboard_log=LOG_DIR if args.tensorboard else None,
    )

    model.learn(total_timesteps=args.timesteps, progress_bar=True)

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_stand")
    model.save(out_path)
    print(f"Saved checkpoint to {out_path}.zip")
    print("Next: python export_onnx.py")


if __name__ == "__main__":
    main()
