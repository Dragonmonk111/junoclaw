"""Export a trained SB3 PPO policy to ONNX for the real-time runtime
(Phase E — wired into the bridge's server.py, gated through SkillGate/
SafetyEnvelope before ever reaching real servos).

SB3 doesn't export ONNX directly, so we wrap the actor network (feature
extractor -> mlp_extractor.policy_net -> action_net) in a plain
torch.nn.Module and export that. Output is the *mean* action (deterministic
policy) — matches how the real robot should run inference (no exploration
noise on hardware).

Usage:
    python export_onnx.py --checkpoint checkpoints/ppo_stand.zip
"""

from __future__ import annotations

import argparse
import hashlib
import os

import torch
from stable_baselines3 import PPO

from env import QUADRUPED_JOINT_NAMES


class OnnxPolicyWrapper(torch.nn.Module):
    """Deterministic actor-only wrapper: obs -> action_mean in [-1, 1]."""

    def __init__(self, sb3_policy):
        super().__init__()
        self.features_extractor = sb3_policy.features_extractor
        self.mlp_extractor = sb3_policy.mlp_extractor
        self.action_net = sb3_policy.action_net

    def forward(self, obs: torch.Tensor) -> torch.Tensor:
        features = self.features_extractor(obs)
        latent_pi, _ = self.mlp_extractor(features)
        mean_actions = self.action_net(latent_pi)
        return torch.tanh(mean_actions)  # squash to [-1, 1] like the env expects


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--checkpoint", default=os.path.join(os.path.dirname(__file__), "checkpoints", "ppo_stand.zip"))
    parser.add_argument("--out", default=os.path.join(os.path.dirname(__file__), "checkpoints", "policy_stand.onnx"))
    args = parser.parse_args()

    model = PPO.load(args.checkpoint, device="cpu")
    wrapper = OnnxPolicyWrapper(model.policy).eval()

    obs_dim = model.observation_space.shape[0]
    dummy_input = torch.zeros(1, obs_dim)

    torch.onnx.export(
        wrapper,
        dummy_input,
        args.out,
        input_names=["observation"],
        output_names=["action"],
        dynamic_axes={"observation": {0: "batch"}, "action": {0: "batch"}},
        opset_version=17,
    )
    print(f"Exported ONNX policy to {args.out}")
    print(f"Input: observation (batch, {obs_dim}) — see env.py _get_obs() for layout")
    print(f"Output: action (batch, {len(QUADRUPED_JOINT_NAMES)}) in [-1, 1], "
          f"map via env._action_to_joint_targets() joint order: {QUADRUPED_JOINT_NAMES}")

    # Compute SHA-256 integrity hash for on-chain verification
    with open(args.out, "rb") as f:
        onnx_bytes = f.read()
    model_hash = hashlib.sha256(onnx_bytes).hexdigest()
    model_size = len(onnx_bytes)
    print(f"\nModel integrity hash (SHA-256): {model_hash}")
    print(f"Model size: {model_size} bytes ({model_size / 1024:.1f} KB)")
    print(f"\nTo register on-chain, include this hash in the PublishSkill message:")
    print(f'  {{"name": "<skill_name>", "onnx_hash": "{model_hash}", ...}}')


if __name__ == "__main__":
    main()
