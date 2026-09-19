# DOGZILLA-Lite Sim2Real Pipeline (Phase C scaffold)

Standing-policy RL pipeline: MuJoCo -> PPO (Stable-Baselines3) -> ONNX.
See `../drafts/PLAN_SIM2REAL_RL_PIPELINE.md` for the full phased plan.

## Status

**Pipeline validated end-to-end, policy NOT yet trained to convergence.**

A 20k-timestep smoke test confirms the full loop runs without errors:
`env.py` loads cleanly, PPO trains, checkpoint saves, ONNX export succeeds,
and the exported graph passes `onnx.checker.check_model`. That's all a
smoke test proves — 20k steps is nowhere near enough for the robot to learn
to stand (`ep_len_mean` was still ~49/500 steps at the end, i.e. falling
almost immediately). Real training needs far more timesteps.

## Files

- `models/dogzilla_lite.xml` — MJCF model. **Dimensions are placeholders**
  from `QuadrupedConfig` in the Rust crate, not measured from the real
  robot (Phase A of the plan). Swap in real numbers once measured.
- `env.py` — `DogzillaStandEnv`, a Gymnasium env. Joint order matches
  `QUADRUPED_JOINT_NAMES` in `crates/junoclaw-physics/src/simulator.rs`.
- `train_standing.py` — PPO training script.
- `export_onnx.py` — exports a trained checkpoint to ONNX for the future
  real-time runtime (Phase E, not built yet).

## Usage

```powershell
pip install -r requirements.txt

# Sanity check the env loads and spaces are valid (no training)
python train_standing.py --check-only

# Real training run — 20k was a smoke test; expect to need 500k-2M+
# timesteps for a standing policy to actually converge. On this machine
# (CPU only, ~500 fps with 4 parallel envs) that's roughly 15-70 minutes
# per million timesteps. Watch ep_len_mean climb toward 500 (max episode
# length) as a sign it's learning to stay up.
python train_standing.py --timesteps 1000000 --n-envs 8

# Export the trained checkpoint
python export_onnx.py --checkpoint checkpoints/ppo_stand.zip

# Optional: watch it in the MuJoCo viewer once trained (needs a GUI session)
python -c "
from stable_baselines3 import PPO
from env import DogzillaStandEnv
model = PPO.load('checkpoints/ppo_stand.zip')
env = DogzillaStandEnv(render_mode='human')
obs, _ = env.reset()
for _ in range(2000):
    action, _ = model.predict(obs, deterministic=True)
    obs, reward, terminated, truncated, info = env.step(action)
    env.render()
    if terminated or truncated:
        obs, _ = env.reset()
"
```

## Known gaps before this is real sim2real (not just a working demo)

1. **Model accuracy (Phase A)** — dimensions/masses are placeholders, not
   measured. Standing in sim proves nothing about standing in reality until
   these are real.
2. **Motor identification (Phase B)** — the MJCF actuators use guessed PD
   gains (`kp="6"`/`"10"`), not identified from real servo step responses.
   This is the single biggest sim2real risk per Open Duck Mini's own
   findings — an inaccurate motor model produces policies that look great
   in sim and fail immediately on hardware.
3. **No domain randomization yet** — Phase C is intentionally not robust;
   Phase D adds randomized mass/friction/motor params.
4. **No locomotion reward yet** — this env only rewards standing still.
   Walking (Phase D) needs a velocity-tracking + imitation reward against
   the existing `QuadrupedBackend::apply_gait` trot as a reference motion.
5. **Runtime integration (Phase E) — wired, not validated on hardware.**
   `plugins/plugin-ros2/bridge` now has `/robot/policy/{load,start,stop,status}`
   endpoints that load an ONNX policy and run closed-loop inference against
   live telemetry, gated by the same fail-closed `MAX_JOINT_DELTA_PER_CYCLE_RAD`
   clamp `play_skill` already uses (not the real `SkillGate`/`WorldModel` —
   `plugin-ros2` still doesn't depend on `junoclaw-physics` in-process; see
   the comment above `PolicyRuntime` in `server.py` for why). Two honest gaps
   in the observation the bridge builds vs. what the policy was trained on:
   joint velocity is a finite-difference estimate in simulate mode (real
   `/joint_states` velocities are used when ROS2-connected), and trunk height
   has no sensor at all — it's hardcoded to the sim's 0.16m standing height.
   Tested against a random-weight ONNX graph in
   `plugins/plugin-ros2/bridge/tests/test_bridge.py`, not yet against a
   converged policy on real hardware.
