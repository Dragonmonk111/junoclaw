"""Phase G3: train a fall-recovery PPO policy in MuJoCo.

Learns to self-right from random fallen poses (on side, upside down,
tilted). v4 adds stronger explosion penalty (-20x), effort reward for
trying to move, and time penalty for faster recovery. No curriculum —
trains on full tilt range (60-150°) from start.

Usage:
    python train_recovery.py --timesteps 1000000 --n-envs 8
"""

from __future__ import annotations

import argparse
import os

import numpy as np
from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env
from stable_baselines3.common.vec_env import SubprocVecEnv

from env import DogzillaRecoveryEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timesteps", type=int, default=1_000_000)
    parser.add_argument("--n-envs", type=int, default=8)
    parser.add_argument("--check-only", action="store_true", help="Just validate the env and exit")
    parser.add_argument("--tensorboard", action="store_true", help="Enable tensorboard logging")
    parser.add_argument("--resume", default=None, help="Path to a checkpoint .zip to continue training from")
    parser.add_argument("--curriculum", action="store_true", help="Use curriculum: start with easier tilts (30-90 deg)")
    parser.add_argument("--tilt-min", type=float, default=None, help="Custom curriculum tilt min in radians (overrides --curriculum)")
    parser.add_argument("--tilt-max", type=float, default=None, help="Custom curriculum tilt max in radians (overrides --curriculum)")
    parser.add_argument("--lr", type=float, default=None, help="Override learning rate (applies to resumed models too)")
    parser.add_argument("--ent-coef", type=float, default=None, help="Override entropy coefficient for exploration (default 0.0)")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaRecoveryEnv())
        print("Env check passed.")
        return

    if args.tilt_min is not None and args.tilt_max is not None:
        env_kwargs = {"curriculum_tilt_min": args.tilt_min, "curriculum_tilt_max": args.tilt_max}
        print(f"Custom tilt range: {np.degrees(args.tilt_min):.0f}-{np.degrees(args.tilt_max):.0f} deg")
        vec_env = make_vec_env(lambda: DogzillaRecoveryEnv(**env_kwargs), n_envs=args.n_envs, vec_env_cls=SubprocVecEnv)
    elif args.curriculum:
        env_kwargs = {"curriculum_tilt_min": np.pi / 6, "curriculum_tilt_max": np.pi / 2}
        print("Curriculum stage 1: easy tilts 30-90 deg")
        vec_env = make_vec_env(lambda: DogzillaRecoveryEnv(**env_kwargs), n_envs=args.n_envs, vec_env_cls=SubprocVecEnv)
    else:
        vec_env = make_vec_env(lambda: DogzillaRecoveryEnv(), n_envs=args.n_envs, vec_env_cls=SubprocVecEnv)

    ent_coef = args.ent_coef if args.ent_coef is not None else 0.0

    if args.resume:
        model = PPO.load(args.resume, env=vec_env, device="cpu")
        if args.lr is not None:
            model.learning_rate = args.lr
            # Force LR schedule reset so the new rate takes effect
            model._setup_lr_schedule()
        if args.ent_coef is not None:
            model.ent_coef = ent_coef
        print(f"Resumed from {args.resume} (lr={model.learning_rate}, ent_coef={model.ent_coef})")
    else:
        model = PPO(
            "MlpPolicy",
            vec_env,
            policy_kwargs={"net_arch": [256, 256]},
            n_steps=512,
            batch_size=256,
            learning_rate=3e-4,
            gamma=0.98,
            ent_coef=ent_coef,
            verbose=1,
            tensorboard_log=LOG_DIR if args.tensorboard else None,
        )

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_recovery")
    checkpoint_every = max(args.timesteps // 10, 25_000)
    remaining = args.timesteps
    while remaining > 0:
        chunk = min(checkpoint_every, remaining)
        model.learn(total_timesteps=chunk, progress_bar=True, reset_num_timesteps=False)
        remaining -= chunk
        model.save(out_path)
        print(f"Checkpoint saved to {out_path}.zip ({args.timesteps - remaining}/{args.timesteps} steps)")

    print(f"Done. Final checkpoint: {out_path}.zip")
    print(f"Next: python export_onnx.py --checkpoint {out_path}.zip --out checkpoints/policy_recovery.onnx")


if __name__ == "__main__":
    main()
