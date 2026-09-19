"""Track-2: train a smoother walk policy for cleaner hardware locomotion.

Warm-starts from the existing speed+heading+terrain policy and trains with
gentler gait parameters (2.5 Hz cadence, 0.18 rad lift) plus an action
smoothness reward. The result should be a less jerky, more natural walk
that performs better on real servos.

Usage:
    python train_smooth_walk.py --timesteps 2000000 --n-envs 8 \
        --resume checkpoints/ppo_speed_heading_terrain.zip

    # Check the env is valid first:
    python train_smooth_walk.py --check-only
"""

from __future__ import annotations

import argparse
import os

from stable_baselines3 import PPO
from stable_baselines3.common.env_util import make_vec_env
from stable_baselines3.common.env_checker import check_env
from stable_baselines3.common.vec_env import SubprocVecEnv

from env import DogzillaSmoothWalkEnv

CHECKPOINT_DIR = os.path.join(os.path.dirname(__file__), "checkpoints")
LOG_DIR = os.path.join(os.path.dirname(__file__), "logs")


def main():
    parser = argparse.ArgumentParser(description="Track-2: smoother walk training")
    parser.add_argument("--timesteps", type=int, default=2_000_000,
                        help="Total training timesteps (default 2M)")
    parser.add_argument("--n-envs", type=int, default=8,
                        help="Number of parallel envs (default 8)")
    parser.add_argument("--check-only", action="store_true",
                        help="Just validate the env and exit")
    parser.add_argument("--resume", default=None,
                        help="Checkpoint to warm-start from (e.g. ppo_speed_heading_terrain.zip)")
    parser.add_argument("--roughness", type=float, default=0.008,
                        help="Terrain roughness (default 0.008, gentler than base 0.015)")
    parser.add_argument("--lr", type=float, default=1e-4,
                        help="Learning rate (default 1e-4, lower than base 3e-4 for fine-tuning)")
    args = parser.parse_args()

    os.makedirs(CHECKPOINT_DIR, exist_ok=True)
    os.makedirs(LOG_DIR, exist_ok=True)

    if args.check_only:
        check_env(DogzillaSmoothWalkEnv(terrain_roughness=args.roughness))
        print("Smooth walk env check passed.")
        print(f"  Gait: {_SMOOTH_GAIT_HZ} Hz, lift={_SMOOTH_LIFT_ANGLE} rad, stride={_SMOOTH_STRIDE} rad")
        return

    vec_env = make_vec_env(
        lambda: DogzillaSmoothWalkEnv(terrain_roughness=args.roughness),
        n_envs=args.n_envs,
        vec_env_cls=SubprocVecEnv,
    )

    if args.resume and os.path.exists(args.resume):
        model = PPO.load(args.resume, env=vec_env, device="cpu")
        print(f"Warm-started from {args.resume}")
        print(f"  LR={args.lr} (fine-tuning mode)")
        # Override learning rate for fine-tuning
        model.learning_rate = args.lr
    else:
        print("Training from scratch (43-dim obs, 256x256, smooth gait).")
        model = PPO(
            "MlpPolicy",
            vec_env,
            policy_kwargs={"net_arch": [256, 256]},
            n_steps=512,
            batch_size=256,
            learning_rate=args.lr,
            gamma=0.99,
            verbose=1,
        )

    out_path = os.path.join(CHECKPOINT_DIR, "ppo_smooth_walk")
    checkpoint_every = max(args.timesteps // 10, 25_000)
    remaining = args.timesteps
    while remaining > 0:
        chunk = min(checkpoint_every, remaining)
        model.learn(total_timesteps=chunk, progress_bar=True, reset_num_timesteps=False)
        remaining -= chunk
        model.save(out_path)
        print(f"Checkpoint saved to {out_path}.zip ({args.timesteps - remaining}/{args.timesteps} steps)")

    print(f"\nDone. Final checkpoint: {out_path}.zip")
    print(f"Next: python export_onnx.py --checkpoint {out_path}.zip --out checkpoints/policy_smooth_walk.onnx")
    print(f"Then copy to Pi: scp checkpoints/policy_smooth_walk.onnx pi@10.42.0.1:~/")


if __name__ == "__main__":
    # Import the smooth gait constants for the --check-only summary
    from env import _SMOOTH_GAIT_HZ, _SMOOTH_LIFT_ANGLE, _SMOOTH_STRIDE
    main()
