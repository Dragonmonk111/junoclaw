# Sim2Real RL Pipeline for DOGZILLA-Lite — Scoping

*Matching the Open Duck Mini (Hugging Face / Pollen Robotics) approach: train
locomotion in a physically-accurate simulator, export to ONNX, run on real
servos — instead of the current hand-coded/keyframe-only control.*

## Why this is a different feature from what we have today

| | Today (JunoClaw) | Open Duck Mini | This plan |
|---|---|---|---|
| Gait source | Hand-coded sinusoid (`QuadrupedBackend::apply_gait`) or recorded keyframes (Skills) | Neural policy trained via RL | Neural policy trained via RL |
| Physics | Simplified kinematic approximation, no MuJoCo | MuJoCo + BAM motor identification | MuJoCo + motor identification |
| Robustness | None — open-loop replay | Domain-randomized, disturbance-robust | Domain-randomized |
| Deploy format | Raw joint angles | ONNX policy, real-time inference | ONNX policy, real-time inference |

`@crates/junoclaw-physics/src/worldmodel.rs` (L2 `WorldModel`) is complementary,
not a substitute: it's a fast linear predictor used to *veto* candidate
actions against L1 memory of red verdicts. It can sit on top of a trained
policy as an extra safety gate, same as it does today for hand-coded gaits.

## Reusable pieces (no need to rebuild)

- `QUADRUPED_JOINT_NAMES` (15 DOF) — already the canonical joint schema used
  by the bridge, the viewer, Skills, and `ActionVector`/`StateFeatures`. The
  RL env's action/observation space should mirror this exactly so the L1/L2
  trust stack stays compatible with a trained policy's output.
- `QuadrupedConfig` defaults (mass, leg length, body dims) — a starting point
  for the MJCF `<inertial>`/`<geom>` tags, but currently a single lumped mass
  guess, not per-limb — needs refinement in Phase A.
- `SkillGate` (kinematic clamp) / `SafetyEnvelope` / `TrustLearner` — reused
  unchanged as the runtime safety wrapper around policy output on real
  hardware. A trained policy's joint targets still pass through the same
  invariant checks a recorded skill does today.
- `QuadrupedBackend::apply_gait`'s trot pattern — reusable as a cheap
  **reference motion generator** for an imitation-reward term (same trick
  Open Duck Mini borrows from the Disney BDX paper), instead of building a
  reference-motion tool from scratch.

## Net-new work

### Phase A — Accurate robot model (~1 day, needs the physical unit)
1. Measure real DOGZILLA-Lite: body dims, leg segment lengths, per-limb mass
   (kitchen scale is fine), joint ranges by hand-moving each servo.
2. Hand-build an MJCF (no CAD access — Yahboom's STL/URDF isn't publicly
   pullable, only PDF assembly guides). Good enough for sim2real; revisit if
   a CAD dump ever surfaces.
3. Validate: robot stands under gravity alone in the MuJoCo viewer (no
   controller) without immediately collapsing/exploding — the standard
   sanity check before any training.
4. Update `QuadrupedConfig` defaults in Rust to match the measured reality —
   this also permanently fixes the "match the real body" ask, not just the
   Three.js viewer's cosmetics.

### Phase B — Motor identification (~1 day, needs hardware bridge running)
Per Open Duck Mini's own docs, this is the single biggest lever for sim2real
success. Send known step/sine commands to the real servos via the existing
`/robot/joint_command` endpoint, log commanded vs. actual position, fit
simple 2nd-order servo params (kp, damping, frictionloss) into the MJCF
actuator tags. Blocked until Phase 3 of `PLAN_DOGZILLA_LITE_CM5_DEPLOYMENT.md`
(servo driver) is wired.

### Phase C — Standing policy (~1 day)
Simplest possible RL task (reward = stay upright, penalize falling) to
validate the full training loop end-to-end (env, PPO, ONNX export) before
attempting anything harder. New Python dependency surface: MuJoCo + a
training lib (Brax/JAX or SB3 — SB3 is lower setup friction on Windows).

### Phase D — Walking/trot policy (~2-3 days)
- Reward: forward velocity + upright bonus + energy penalty + imitation
  term against the existing trot reference (reuse, not reinvent)
- Domain randomization: mass ±10%, friction, motor params, sensor noise
- Export trained policy to ONNX

### Phase E — Runtime integration (~1 day)
Small ONNX inference loop added to `server.py`, reading joint/IMU state at
~30-50Hz, calling the policy, posting through the same `/robot/joint_commands`
batch endpoint already built for the gait/D-pad panel — gated through
`SkillGate`/`SafetyEnvelope` before ever reaching real servos. Test in
`--simulate` first. Once working, the viewer's Gaits panel gets an "AI Trot"
toggle alongside the hand-coded one.

### Phase F — Iterate (ongoing)
Refine motor ID and reward shaping using logs from real runs.

## Total estimate
~5-7 focused days, gated by physical-hardware availability for Phases A/B
(measurement + motor ID can't be done without the unit in hand).

## Open question
Phase A/B need the physical robot in hand for measurements/motor ID — these
can't be scaffolded blind. Phase C (standing policy) can start immediately
with placeholder dimensions from `QuadrupedConfig` and be re-tuned once real
measurements land.
