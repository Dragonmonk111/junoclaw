# Sim2Real Training Roadmap — Post Phase E

*After completing Phases A–E (stand + walk policies trained, ONNX exported,
bridge inference loop wired). This document tracks what to train next and
priorities for improving sim2real transfer.*

## Current State (Sep 2026)

| Policy | Checkpoint | ONNX | Timesteps | Status | Fall Rate | Mean Ep Len |
|--------|-----------|------|-----------|--------|-----------|-------------|
| Stand  | `ppo_stand.zip`  | `policy_stand.onnx`  | 100K   | Trained, exported, sim-tested | — | — |
| Walk   | `ppo_walk.zip`   | `policy_walk.onnx`   | 4M (2M + 2M ext) | Trained, exported, sim-tested, ONNX hash computed | 100% (20/20) | 78.8 steps (best: 159) |
| Turn   | `ppo_turn.zip`   | `policy_turn.onnx`   | 3M (2M walk + 1M turn) | Trained, exported, eval'd | 100% (20/20) | 82 steps (1.6s) |
| Recovery (v1) | `ppo_recovery.zip` | `policy_recovery.onnx` | 500K | Trained, exported, eval'd (buggy env) | 90% (18/20) | 52 steps |
| Recovery (v2) | `ppo_recovery.zip` | `policy_recovery.onnx` | 500K | Trained, exported, eval'd (fixed env) | 95% (19/20) | 50.6 steps |
| Recovery (v3) | `ppo_recovery.zip` | `policy_recovery.onnx` | 1M | Trained, exported, eval'd (curriculum bug) | 95% (19/20) | 52.4 steps |
| Recovery (v4) | `ppo_recovery.zip` | `policy_recovery.onnx` | 1M | Trained, exported, eval'd (stronger penalties, effort reward) | 95% (19/20) | 53.2 steps |
| Speed+Heading | `ppo_speed_heading.zip` | `policy_speed_heading.onnx` | 2M | Trained, exported, eval'd (43-dim, 256x256, from scratch) | 100% (20/20) | 69.9 steps (best: 113) |
| Terrain | `ppo_terrain.zip` | `policy_terrain.onnx` | 2M (warm from walk) | Trained, exported, eval'd (±40% friction, forces, impulses) | 100% (20/20) | 78.4 steps (best: 126) |
| Sit | `ppo_sit.zip` | `policy_sit.onnx` | 500K (warm from stand) | Trained, exported, eval'd (pose matching, 128x128) | 100% (20/20) | 67.5 steps (best: 95) |
| Step | `ppo_step.zip` | `policy_step.onnx` | 2M (warm from terrain) | Trained, exported, eval'd (0.5-2cm step obstacle, 256x256) | 100% (20/20) | 68.8 steps (best: 98) |
| Speed+Heading+Terrain | `ppo_speed_heading_terrain.zip` | `policy_speed_heading_terrain.onnx` | 3M (warm from G8) | Trained, exported, eval'd (43-dim, 256x256, combined) | 100% (20/20) | 102.6 steps (best: 207) |

### Walk Policy Baseline (eval_policy.py, 10 episodes, no domain randomization)
- Fall rate: 100% — robot survives ~82 steps (1.6s at 50Hz) before falling
- Mean upright score: 0.895 (stays mostly upright until it doesn't)
- Mean forward velocity: varies widely (-0.17 to +0.36 m/s, target was 0.08)
- The policy learned a gait but isn't stable long-term — expected for a
  first walk policy with a small MLP (128x128) and simple reward shaping.
  Improvement levers: longer training, curriculum, bigger network, better
  reward shaping (e.g., contact forces, foot air-time reward).

### Walk Policy Extended (eval_policy.py, 20 episodes, after 2M additional steps)
- Fall rate: 100% (20/20) — still falls eventually
- Mean episode length: 78.8 steps (1.6s) — similar mean, but higher variance
- Best episode: 159 steps (3.2s) — significantly longer than any previous
- Mean upright score: 0.898 — consistent with baseline
- Mean forward velocity: -0.01 m/s (near zero — not making forward progress)
- Episodes 2, 8, 13, 18, 19 reached 89+ steps — more episodes showing stability
- ONNX integrity hash: 98c493698fb984129df267e1e6f1e43270018aab8c82acc6f4b5028e81642c9f
- **Key insight**: Extended training improved peak performance (159 vs 82 steps)
  but didn't improve mean. The policy found a better gait mode but can't
  consistently enter it. Needs curriculum learning or reward shaping to
  stabilize the good gait.

### Turn Policy Eval (20 episodes, no domain randomization)
- Fall rate: 100% — same 82-step mean survival as walk baseline
- Mean upright score: 0.903 — slightly better than walk (warm-start helped)
- Mean forward velocity: -0.03 m/s (near zero — expected for turning, not walking forward)
- Yaw error: not yet reported (eval script needs yaw_error in info dict)
- **Key insight**: warm-starting preserved the walk gait. The turn policy
  didn't forget how to walk — it just didn't learn to turn well enough in 1M
  steps. Needs longer training or stronger yaw reward weight.

### Recovery Policy Eval v1 (20 episodes, buggy env — z-axis tilt allowed)
- Success rate: 10% (2/20) with strict success criterion (5 consecutive upright steps)
- Fall rate: 90% — most episodes the robot flails and stays fallen
- Physics explosions in some episodes (height >3m) — ground penetration from
  bad starting poses
- **Root causes identified**:
  1. Tilt axis was 3D random — pure z-axis rotations don't actually tip the robot
  2. Success condition was too loose (single-step upright check allowed false positives)
  3. Starting height 0.05m caused ground penetration for some orientations
- **Fixes applied**:
  1. Tilt axis constrained to x-y plane (always tips the robot sideways)
  2. Success requires 5 consecutive upright steps at correct height (0.08-0.25m)
  3. Starting height raised to 0.08m

### Recovery Policy Eval v2 (20 episodes, fixed env — x-y tilt axis, 0.08m start height)
- Success rate: 5% (1/20) — slightly worse than v1's 10%
- Fall rate: 95%
- Mean upright score: -0.170 (negative — robot is mostly upside down or on side)
- Mean episode length: 50.6 steps (~1s at 50Hz — most episodes terminate quickly)
- Physics explosions still occurring: episodes 1, 2, 8 had heights >2m (max 5.5m!)
- **One genuine success**: episode 13 recovered in 31 steps, upright 0.232, height 0.22m
- **Root cause of remaining issues**:
  1. Reward function still rewards height improvement without cap — explosive jumping
     gets positive reward (episode 1: 1241 reward, height 5.5m, but no success)
  2. No angular velocity penalty — the robot flails wildly without consequence
  3. No contact force penalty — slamming legs into ground is not penalized
  4. 500K steps may be insufficient — the policy hasn't found the recovery strategy
- **Next steps for v3**:
  1. Cap height reward at 0.25m (don't reward going higher than standing)
  2. Add angular velocity penalty: -0.1 * ||gyro||
  3. Add contact force penalty: -0.01 * max(0, contact_force - threshold)
  4. Increase training to 1M steps
  5. Consider curriculum: start with small tilts (10°), increase to 90°

### Recovery Policy Eval v3 (20 episodes, capped height + angular vel penalty + curriculum)
- Success rate: 5% (1/20) — same as v2
- Fall rate: 95%
- Mean upright score: -0.148 (robot mostly stays fallen)
- Mean episode length: 52.4 steps (~1s)
- Mean final height: 0.47m (inflated by 3 explosion episodes)
- Physics explosions still occurring: episodes 1, 2, 8 had heights >2m (max 3.27m)
  - Explosion penalty (-5.0 * (height - 0.30)) reduced severity (3.27m vs 5.5m in v2)
  but didn't eliminate explosions
- **One genuine success**: episode 13 recovered in 34 steps, upright 0.235, height 0.17m
- **Training converged well**: explained_variance 0.83, clip_fraction 0.0002, loss 5.4K
  — the value function learned to predict rewards, but the policy didn't find a good strategy
- **Root cause analysis**:
  1. **Curriculum bug**: training used easy tilts (30-90°) for entire 1M steps but eval
     uses full range (60-150°) — policy never saw hard cases during training
  2. **Explosion penalty too weak**: -5.0 * (height - 0.30) isn't enough to deter the
     policy from exploiting ground penetration contacts
  3. **No "effort" reward**: most episodes the robot lies still (upright negative,
     height ~0.08m) — no incentive to try getting up if it can't do it immediately
  4. **Curriculum needs a callback**: the tilt range is fixed at env creation and
     never progresses — need an SB3 callback that widens the range over training
- **Next steps for v4**:
  1. Remove curriculum (use full tilt range 60-150° from start)
  2. Strengthen explosion penalty: -20.0 * max(0, height - 0.30) (4x stronger)
  3. Add effort reward: +0.1 * |action| (reward trying to move, not just lying still)
  4. Add a "time penalty" -0.01 per step to encourage faster recovery
  5. Keep 1M steps (training converged — the issue is reward design, not training time)

### Recovery Policy Eval v4 (20 episodes, stronger explosion penalty + effort reward + time penalty)
- Success rate: 5% (1/20) — same as v2 and v3
- Fall rate: 95%
- Mean upright score: -0.125 (slightly better than v3's -0.148)
- Mean episode length: 53.2 steps
- Mean final height: 0.55m (inflated by 3 explosion episodes)
- Explosions still occurring (3/20): episodes 1, 2, 8 with heights >2m
  - The -20x explosion penalty made explosions much more costly (ep 1: -5854 reward
  vs v3's -1728), but the policy still finds them preferable to lying still
- **One genuine success**: episode 3 recovered in 56 steps, upright -0.053, height 0.19m
- **Training converged**: explained_variance 0.85, clip_fraction ~0.004
- **Root cause analysis**:
  1. **Recovery from 60-150° tilts is genuinely hard** — the robot starts nearly
     upside down or on its side. Self-righting requires complex contact-rich
     maneuvers that a small MLP struggles to learn from scratch
  2. **Effort reward (0.05) too small** — the time penalty (0.01 * step) accumulates
     faster than the effort reward, so the agent still prefers doing nothing
  3. **Explosions persist despite -20x penalty** — the explosion reward (height >0.3m)
     is a one-time spike; the standing_bonus (2.0/step) is more attractive if the
     robot can briefly reach upright during an explosion
  4. **The fundamental issue**: recovery needs a different RL approach — not just
     reward shaping. Options: (a) imitation learning from a scripted recovery
     trajectory, (b) much larger network (512x512), (c) longer training (5M+ steps),
     (d) simplify the task (start from 30-60° tilts only, not full 60-150°)
- **Next steps for v5** (deferred — recovery is lower priority than walk/turn/speed-heading):
  1. Try imitation learning: script a recovery trajectory and use it as a reference
  2. Simplify: start from 30-60° tilts only (easier self-righting)
  3. Use 512x512 network (4x more capacity)
  4. 5M steps (5x more training time)

### Speed+Heading Policy Eval (G8, 20 episodes, 43-dim obs, 256x256 net, from scratch)
- Fall rate: 100% (20/20) — same as walk baseline
- Mean episode length: 69.9 steps (vs walk's 78.8 — slightly shorter)
- Mean upright score: 0.893 (vs walk's 0.895 — comparable stability)
- Mean forward velocity: 0.05 m/s (more controlled than walk's -0.17 to +0.36 range)
- Mean total reward: 101.74 (positive — speed+heading tracking rewards are working)
- Best episode: 113 steps, fwd_vel 0.26 m/s, upright 0.942
- **Key finding**: G8 maintains walk-level stability while adding commandable
  speed and heading. The 43-dim policy learned to track both commands without
  catastrophic forgetting of the gait. The slightly shorter survival (69.9 vs 78.8)
  is expected — the policy has more to balance (speed + heading + gait).
- **ONNX export**: 1.9 KB, 43-dim input, SHA-256: 667d4aa2...
- **This is the most deployable policy so far** — a single ONNX file that can
  walk at variable speeds and steer, replacing both the walk and turn policies.

Both policies use the same 41-dim observation space and 15-dim action space
defined in `env.py`. Domain randomization is on (±15% mass/friction/gain
perturbation + sensor noise).

---

## Phase F — Iterate on Real Hardware (blocking: needs physical robot)

### F1. Motor Identification (highest impact)
- Send step/sine commands to real servos via `/robot/joint_command`
- Log commanded vs actual position at 50Hz
- Fit 2nd-order servo params (kp, damping, frictionloss) into MJCF actuator tags
- Re-export MJCF, retrain walk policy with identified motor model
- **This is the single biggest lever for sim2real transfer** (per Open Duck Mini docs)

### F2. Real-World Policy Evaluation
- Load `policy_walk.onnx` on the bridge in `--simulate` mode first
- Verify joint trajectories look reasonable (no oscillation, no clamp rejections)
- Switch to real hardware, start with short 5-second runs
- Log joint commands vs actual positions for offline analysis
- Compare sim vs real trajectory divergence

### F3. Reward Shaping from Real Logs
- If the robot falls or stumbles, identify which observation dimensions diverge
- Add domain randomization terms targeting the divergence (e.g., larger friction
  range if foot slips, larger motor delay if servo lag)
- Retrain with wider randomization on the failing dimensions

---

## Phase G — New Policies

### G1. Turn/Steer Policy (priority: high)
- **Why**: Walk policy only goes forward. Steering is needed for useful locomotion.
- **Approach**: Extend `DogzillaWalkEnv` with a steering command in the action space
  or as a parameterized env with a target heading angle.
- **Reward**: Forward velocity projected onto target heading + yaw rate tracking.
- **Estimated effort**: 1 day env + 1 day training (1M timesteps should suffice
  since the walk policy already learned the basic gait — can warm-start from
  `ppo_walk.zip`).
- **New env**: `DogzillaTurnEnv(DogzillaWalkEnv)` — adds heading reward term.

### G2. Terrain-Adaptive Walk (priority: medium)
- **Why**: Real terrain isn't flat. Robot needs to handle carpet, thresholds, slight inclines.
- **Approach**: Add randomized terrain heightfields in MuJoCo (checkerboard bumps,
  inclined planes) to the walk env.
- **Domain randomization**: ground friction [0.3–1.5], incline ±10°, step obstacles
  up to 1cm.
- **Estimated effort**: 2 days env + 2-3 days training (2M timesteps).
- **New env**: `DogzillaTerrainEnv(DogzillaWalkEnv)` — adds terrain generation.

### G3. Recovery / Fall Recovery Policy (priority: medium)
- **Why**: If the robot falls, it should self-right.
- **Approach**: Start from a random non-standing pose (on side, upside down).
  Reward = upright score improvement + height increase. No locomotion term.
- **Estimated effort**: 1 day env + 1 day training (500K timesteps — simpler task).
- **New env**: `DogzillaRecoveryEnv(DogzillaStandEnv)` — randomized fall starts.

### G4. Speed-Adaptive Walk (priority: low)
- **Why**: Current walk has a fixed target velocity (0.08 m/s). Want variable speed.
- **Approach**: Parameterized env with a commanded speed in observation. Train one
  policy that works across 0.0–0.2 m/s.
- **Estimated effort**: 1 day env + 2 days training (2M timesteps).
- Can be combined with G1 (turn) into a single go-anywhere policy.

### G5. Arm-Assisted Balance (priority: low, research)
- **Why**: The arm (3 DOF) is currently stowed. It could assist balance during
  walking, especially on uneven terrain.
- **Approach**: Remove the arm-stowed constraint in the walk env, add a small
  reward for arm-based counterbalance when trunk tilt is detected.
- **Estimated effort**: 2 days (requires careful reward design to avoid the arm
  flailing).

---

## Phase H — Training Infrastructure Improvements

### H1. Curriculum Learning
- Start with easy task (flat ground, no randomization) and gradually increase
  difficulty. SB3 doesn't natively support this — use a callback that swaps env
  parameters at milestone timesteps.
- Priority: medium. Could improve final policy quality by 20-30%.

### H2. Parallel Training with SubprocVecEnv — ✅ DONE
- All training scripts (`train_walk.py`, `train_turn.py`, `train_recovery.py`,
  `train_speed_heading.py`) now use `SubprocVecEnv` instead of `DummyVecEnv`.
- Expected speedup: 4-6x on multi-core machines for future training runs.
- Note: current recovery v3 run started before this change — using DummyVecEnv.

### H3. Tensorboard Logging as Default
- Currently opt-in (`--tensorboard` flag). Make it default so we always have
  training curves for analysis.
- Priority: low (quality of life).

### H4. Evaluation Script — ✅ DONE
- `eval_policy.py` built. Loads SB3 checkpoints or ONNX policies.
- Reports: fall rate, survival rate, mean episode length, total reward,
  upright score, forward velocity, vel tracking, imitation reward, yaw error.
- Supports all envs: stand, walk, turn, recovery.
- **Baseline walk metrics (10 episodes, no domain randomization):**
  - Fall rate: 100% — robot survives ~82 steps (1.6s at 50Hz) before falling
  - Mean upright score: 0.895 (stays mostly upright until it doesn't)
  - Mean forward velocity: varies widely (-0.17 to +0.36 m/s, target was 0.08)
  - The policy learned a gait but isn't stable long-term — expected for a
    first walk policy with a small MLP (128x128) and simple reward shaping.

### H5. Action Smoothing in Bridge — ✅ DONE
- Added EMA (exponential moving average) action smoothing to `start_policy()`.
- Default: 20% new / 80% previous. Prevents first-step jumps that trip safety clamp.
- `prev_targets` initialized from current joint positions → first step delta is zero.
- Configurable via `/robot/policy/start` body: `{"smoothing": 0.2}`.
- Standard sim2real technique — set to 1.0 for raw policy output.

---

## Learning Points (Accumulated)

### From Walk Policy Training (Phase D)
- **Imitation reward works**: The reference trot gait in the reward function
  successfully guides the policy toward a natural-looking gait instead of
  unnatural hopping/dragging. The policy oscillates at ~4 Hz, matching the
  reference gait frequency.
- **Domain randomization is essential**: ±15% perturbation on mass/friction/gains
  produces a policy that generalizes across physics variations. Without it,
  the policy overfits to the sim's exact parameters.
- **Small MLP is sufficient for locomotion**: 128x128 (23K params) is enough
  for a 15-DOF quadruped. Larger networks may help for terrain adaptation
  but aren't needed for flat-ground walking.
- **100% fall rate at 2M steps is not failure**: The policy learned a gait
  (oscillation patterns, forward progress) but lacks long-term stability.
  Improvement levers: longer training (5-10M), curriculum learning, better
  reward shaping (contact forces, foot air-time reward), bigger network.

### From Sim Testing (Phase E)
- **Safety clamp catches pose mismatch**: First sim test rejected at step 0
  because the stand pose thighs were 0.0 rad but the training keyframe used
  0.5 rad. The policy's first action was a large correction that exceeded
  the 0.6 rad/cycle clamp. Fix: always pre-position to the training keyframe.
- **EMA smoothing prevents startup rejection**: Even with correct pose, the
  policy's first inference can produce large joint deltas. EMA smoothing
  (0.2 factor) eliminates this by blending with current positions.
- **Arm joints move during walking**: The policy drives the 3-DOF arm even
  though it wasn't constrained in training. For locomotion-only deployment,
  consider clamping arm joints to zero or adding an arm-stay penalty.

### From Turn/Recovery Env Design (Phase G)
- **Warm-starting is the right approach for turn**: The walk policy already
  knows how to trot. Warm-starting from `ppo_walk.zip` means the turn policy
  only needs to learn yaw control, not locomotion from scratch. ~1M steps
  should suffice vs 2M for the original walk.
- **Recovery is a simpler task than walking**: Self-righting doesn't require
  gait timing or forward velocity — just "get upright from a fallen state."
  500K steps and 200-step episodes (4s) should be sufficient.
- **Same observation space = drop-in compatibility**: Both new envs use the
  same 41-dim observation as the walk env. Trained policies work with the
  existing bridge inference loop without any changes.

### From Turn/Recovery Eval (Phase G eval)
- **Warm-starting preserves walk stability**: Turn policy maintained 0.903
  upright score (vs walk's 0.895) and identical 82-step survival. The policy
  didn't catastrophically forget walking — it just didn't learn turning yet.
  1M steps was enough to preserve, not enough to add new capability.
- **Tilt axis matters for recovery env**: A 3D random tilt axis produces
  ~33% of episodes where the robot just spins (z-axis rotation) instead of
  falling sideways. Constraining to x-y plane ensures every episode is a
  genuine fall recovery challenge.
- **Physics explosions from ground penetration**: Starting a tilted robot at
  0.05m height causes some orientations to penetrate the ground plane,
  producing explosive contact forces (height >3m). Fix: raise to 0.08m and
  let MuJoCo's contact solver handle the initial settling.
- **Single-step success check is too loose**: A robot flipping through upright
  while airborne can trigger a false "success" if the check is just
  `upright > 0.7 and height > 0.12`. Fix: require 5 consecutive upright
  steps at bounded height (0.08-0.25m) to confirm stable self-righting.
- **Recovery needs more training or better reward shaping**: 500K steps with
  the fixed env produced 10% success. The reward function (3.0 * upright +
  5.0 * height_improvement + 2.0 * upright_improvement) may need tuning —
  the height improvement term can reward explosive jumping instead of
  controlled standing. Consider adding a stability term (penalize angular
  velocity) and a contact-force term (penalize slamming legs into ground).

### From Recovery v3 Training (Sep 2)
- **Value function convergence ≠ policy success**: Recovery v3 had excellent
  training metrics (explained_variance 0.83, clip_fraction 0.0002) but still
  only 5% success rate. The value function learned to predict the reward
  landscape, but the policy settled into a local optimum of "lying still"
  because that minimizes penalties without requiring effort.
- **Curriculum without progression is harmful**: Setting easy tilt range
  (30-90°) for the entire training run meant the policy never encountered
  the hard cases (90-150°) that appear in evaluation. A proper curriculum
  needs an SB3 callback that progressively widens the tilt range over
  training milestones.
- **Explosion penalty needs to be overwhelming**: -5.0 * (height - 0.30)
  reduced explosion severity (3.27m vs 5.5m) but didn't eliminate it.
  v4 uses -20.0 * (height - 0.30) — the penalty must be so severe that
  any explosion episode is a catastrophic negative return.
- **"Lying still" is a local optimum**: Without an effort reward, the
  policy learns that doing nothing avoids energy penalties and angular
  velocity penalties. Adding +0.05 * mean(|action|) gives the agent a
  small reward for trying, breaking the "do nothing" equilibrium.
- **Larger network may help for recovery**: Recovery is a more complex
  motor skill than walking (non-linear dynamics, contact-rich). v4 uses
  256x256 (92K params) vs v3's 128x128 (23K params).

### From G8 Speed+Heading Env Design (Sep 2)
- **Observation space change breaks warm-starting**: Can't load a 41-dim
  walk policy into a 43-dim env — SB3 checks observation space on load.
  Must train from scratch or manually pad the first layer weights.
- **43-dim obs is the right design for deployment**: Adding commanded_speed
  and commanded_heading as observation dims is cleaner than action-space
  commands — the policy learns to map state+command → joint targets in
  one pass, no separate controller needed.
- **G8 training converges fast**: explained_variance reached 0.773 by 729K
  steps (from scratch), suggesting the speed+heading task is not much
  harder than walk alone — the imitation reward provides a strong learning
  signal that anchors the gait.

### From Recovery v4 and G8 Eval (Sep 2)
- **Recovery is a fundamentally harder problem than locomotion**: After 4
  iterations (v1-v4) with different reward shapes, network sizes, and
  curriculum strategies, recovery stays at 5% success. The task requires
  contact-rich whole-body coordination that a small MLP can't learn from
  sparse reward signals. Imitation learning or a different RL approach
  (e.g., SAC with replay, or curriculum with proper progression callbacks)
  is needed — more reward shaping won't fix this.
- **Explosion penalty arms race is unwinnable**: Going from -5x to -20x
  reduced but didn't eliminate explosions. The policy finds any positive
  reward signal (standing_bonus during a flip) worth the penalty. The fix
  is to remove the standing_bonus entirely and only reward sustained upright
  (5+ consecutive steps), not transient upright during explosions.
- **G8 is the most deployable policy**: A single 43-dim ONNX file (1.9 KB)
  that walks at variable speeds and steers, replacing both walk and turn
  policies. The 256x256 network from scratch learned gait + speed tracking
  + heading tracking in 2M steps. This is the policy to deploy on hardware
  first.
- **From-scratch > warm-start for new observation dims**: G8 trained from
  scratch (ev=0.773 at 729K) outperformed the failed warm-start attempt.
  When the observation space changes, the first-layer weight matrix has
  wrong dimensions — padding with zeros doesn't preserve learned features
  meaningfully. Better to train from scratch with a larger network.

---

## Recommended Execution Order (Updated Sep 2)

1. ~~**H4** (eval script)~~ — ✅ Done
2. ~~**H5** (action smoothing)~~ — ✅ Done
3. ~~**G1** (turn/steer)~~ — ✅ Trained (1M steps), exported, eval'd (100% fall, 82-step survival, upright 0.903)
4. ~~**G3** (recovery v1)~~ — ✅ Trained (500K steps), exported, eval'd (10% success, env bugs found & fixed)
5. ~~**G3v2** (recovery retrain)~~ — ✅ Trained (500K steps), exported, eval'd (5% success, reward shaping needs work)
6. ~~**Walk2** (extended walk training)~~ — ✅ Trained (2M additional), eval'd (100% fall, best ep 159 steps, ONNX hash computed)
7. ~~**H2** (SubprocVecEnv)~~ — ✅ Done — all training scripts updated
8. ~~**ONNX hash** (OpenGradient action item)~~ — ✅ Done — export_onnx.py computes SHA-256
9. ~~**G3v3** (recovery reward fix)~~ — ✅ Trained (1M steps, curriculum), eval'd (5% success, curriculum bug identified)
10. ~~**G8** (commandable speed+heading)~~ — ✅ Trained (2M steps, 43-dim, 256x256), exported, eval'd (100% fall, 70-step survival, upright 0.893, most deployable policy)
11. ~~**G3v4** (recovery full-range fix)~~ — ✅ Trained (1M steps, no curriculum, -20x explosion, effort reward), eval'd (5% success, recovery deferred)
12. **F1** (motor ID) — needs physical robot, biggest sim2real lever
13. **F2** (real eval) — first real deployment
14. ~~**G2** (terrain)~~ — ✅ Trained (2M steps, warm from walk, 256x256), exported, eval'd (78.4 steps, upright 0.902, terrain-robust)
15. **HF Hub** (RoboNet action item) — add --push flag to export_onnx.py
16. **G3v5** (recovery rethink) — imitation learning or simplified tilts, deferred
17. **G5** (arm balance) — research, defer
18. ~~**G6** (sit/lie down)~~ — ✅ Trained (500K steps, warm from stand, 128x128), exported, eval'd (67.5 steps, upright 0.902, partial sit transition)
19. ~~**G7** (step climbing)~~ — ✅ Trained (2M steps, warm from terrain, 256x256), exported, eval'd (68.8 steps, upright 0.852, best ep 98)
20. ~~**G8+G2** (speed+heading+terrain)~~ — ✅ Trained (3M steps, warm from G8, 43-dim, 256x256), exported, eval'd (102.6 steps, best 207, upright 0.880 — BEST POLICY EVER, 30% better than walk baseline)

Items 12-13 are blocked on physical hardware. Items 14-15 can be done
in sim. Recovery (16) is deferred until we rethink the approach.

---

## Pre-Hardware Training Plan (Sep 2)

All training that can be completed in sim before the physical DOGZILLA
unit arrives. Listed in priority order.

### Priority 1: G2 Terrain (DONE)
- **Env**: `DogzillaTerrainEnv` — extends walk with ±40% friction variation,
  random external trunk forces, periodic lateral impulses
- **Training**: 2M steps, warm-started from `ppo_walk.zip`, 256x256 net
- **Why**: Sim2real robustness — real ground is never perfectly flat. This
  is the highest-impact sim training before hardware.
- **Results**: 78.4 steps mean ep len (vs walk's 78.8 on flat), upright 0.902
  (vs walk's 0.895), best ep 126 steps. **Terrain-robust without stability loss.**
- **ONNX**: 1.8 KB, 41-dim, SHA-256: 775e4df2...

### Priority 2: G6 Sit/Lie Down (DONE)
- **Env**: `DogzillaSitEnv` — target pose reward (match sitting keyframe at 0.08m)
- **Training**: 500K steps, warm-started from `ppo_stand.zip`, 128x128 net
- **Why**: Power saving, safe shutdown, deployment readiness
- **Results**: 67.5 steps mean ep len, upright 0.902, reward 54.85. Policy
  learned partial sit transition (heights 0.11-0.34m, target 0.08m). Full
  sit pose not reached but stability maintained during lowering.
- **ONNX**: 2.2 KB, 41-dim, SHA-256: c79ce62b...
- **Next**: More training (1M+) or easier target pose for full sit

### Priority 3: G7 Step Climbing (DONE)
- **Env**: `DogzillaStepEnv` — separate MJCF with 0.5-2cm box obstacle, randomized per episode
- **Training**: 2M steps, warm-started from `ppo_terrain.zip`, 256x256 net
- **Why**: Real environments have thresholds, carpet edges, small steps
- **Results**: 68.8 steps mean ep len (vs terrain's 78.4), upright 0.852, best ep 98.
  Step obstacle adds ~10 steps of difficulty vs flat terrain. Policy adapts.
- **ONNX**: 1.8 KB, 41-dim, SHA-256: 57495f45...

### Priority 4: G8+G2 Combined Speed+Heading+Terrain (DONE)
- **Env**: `DogzillaSpeedHeadingTerrainEnv` — merges G8's 43-dim
  commandable obs with G2's terrain perturbations (±40% friction, forces)
- **Training**: 3M steps, warm-started from `ppo_speed_heading.zip`, 256x256 net
- **Why**: The ultimate pre-hardware policy — commandable speed, heading,
  AND terrain-robust. Single ONNX file for deployment.
- **Results**: **102.6 steps mean ep len** (best: 207), upright 0.880, reward 140.9.
  30% better than walk baseline (78.8), 31% better than terrain (78.4).
  Best episode survived 207 steps (4.1s at 50Hz) — first policy to break 200.
  The combination of speed+heading commands with terrain perturbations
  produced a more robust gait than any individual task training.
- **ONNX**: 1.9 KB, 43-dim, SHA-256: 389b4a8c...

### Priority 5: HF Hub Integration (infrastructure)
- **Task**: Add `--push` flag to `export_onnx.py` for HuggingFace Hub upload
- **Why**: Policy distribution pipeline — needed for Builder tier of
  subscription ladder
- **Effort**: 2 hours
- **No training needed, just code**

### Deferred (needs hardware or rethink)
- **G3v5 Recovery**: Needs imitation learning approach, not more reward shaping
- **G5 Arm Balance**: Research phase, low priority
- **F1 Motor ID**: Needs physical robot
- **F2 Real Eval**: Needs physical robot

---

## Phase I — New Skill Ideas (Sep 1)

### G6. Sit / Lie Down Policy (priority: medium)
- **Why**: Robot should be able to transition from standing to a resting pose
  and back. Useful for power saving and safe shutdown.
- **Approach**: Target pose reward (match a sitting keyframe) + smooth
  transition reward. Can be a simple trajectory, not full RL — may be better
  as a demonstration skill captured through the browser viewer.
- **Estimated effort**: 0.5 day (likely a Skill, not an RL policy).

### G7. Step Climbing (priority: medium)
- **Why**: Real environments have thresholds, small steps, carpet edges.
- **Approach**: Add 1-2cm step obstacles to the walk env. Reward for
  maintaining height and forward progress over the obstacle.
- **Estimated effort**: 1 day env + 2 days training (2M timesteps).
- **New env**: `DogzillaStepEnv(DogzillaWalkEnv)` — adds step geometry.

### G8. Commandable Speed + Heading (priority: high, merges G1+G4) — ENV BUILT
- **Why**: A single policy that accepts [speed, heading] commands is more
  useful than separate walk/turn policies. This is what you'd actually
  deploy on a real robot.
- **Approach**: Extend observation to 43 dims (add commanded_speed and
  commanded_heading). Train one policy across speed range [0, 0.2] m/s and
  heading range [-90°, 90°].
- **Trade-off**: Changes the observation space → needs bridge update too.
  But produces the most useful single policy.
- **Estimated effort**: 2 days env + 3 days training (3M timesteps).
- **Status**: `DogzillaSpeedHeadingEnv` built and validated. Training script
  `train_speed_heading.py` created with SubprocVecEnv and 256x256 network.
  Can warm-start from `ppo_walk.zip` (will adapt obs from 41→43 dims).
- **Next**: Start training after recovery v3 completes.

---

## Phase J — Competitive Landscape Learnings (Sep 1)

Researched the decentralized/verifiable robotics landscape after Grok analysis.
Key projects and what we should learn from each:

### From OpenMind (OM1 + FABRIC)
- **Skill chips framing**: Make ONNX policies as easy to install as OM1 skill chips.
  Their app store model (1,000+ developers) is the adoption playbook.
- **Hardware breadth**: OM1 runs on Unitree Go2/G1, TurtleBot4, Ubtech Yanshee.
  We need to support at least one more robot platform beyond Dogzilla.
- **Go runtime for edge**: OM1 migrated from Python to Go for lower latency
  and smaller footprint. Worth studying for our bridge.
- **Action item**: Package ONNX policies as "skill chips" with metadata
  (robot type, joint schema, observation format) for easy discovery.

### From peaq
- **Omnichain identity**: Machine identity shouldn't be locked to one chain.
  Consider IBC-enabled skill registry so skills are discoverable cross-chain.
- **Universal Machine Functions**: Their pattern (ID, Access, Storage, Payment,
  Verification, Time) is a clean abstraction. We have ID and Verification;
  Payment and Time are gaps.
- **Machine credit ratings**: Maps to our trust scoring but more economically
  developed. Consider staking-based skill quality signals.
- **Action item**: Explore IBC packet for skill provenance (verify a skill
  on Juno, use it on another Cosmos chain).

### From OpenGradient
- **Model integrity verification**: Hash the ONNX graph and commit it on-chain
  so any robot can verify "this is the exact policy that was trained and
  registered." We have ONNX export but no cryptographic binding to the
  training record.
- **TEE attestation for inference**: Their TEE-verified inference is production-
  ready. We should add TEE attestation to our bridge inference loop so the
  runtime can prove "this exact ONNX model produced this exact action."
- **x402 payment protocol**: Per-inference settlement is elegant. Could enable
  skill marketplace economics (pay per skill execution).
- **Action item**: Add ONNX model hash to skill-registry contract's
  PublishSkill message. This is a small change with high trust value.
- **Status**: ✅ DONE — `export_onnx.py` now computes SHA-256 of the ONNX
  model and prints it. The skill-registry contract already has a `skill_hash`
  field in `PublishSkill` that stores this hash on-chain.

### From RODEO (ICRA 2026)
- **Economic autonomy loop**: Robot earns tokens for tasks, reinvests in
  charging. This is the right framing for our skill marketplace — a robot
  that buys a skill should be able to earn from using it.
- **Verification oracle**: Their emulation-based task verification complements
  our model-based SkillGate. Consider adding a post-execution verification
  step (did the skill achieve its stated outcome?).
- **Academic validation**: ICRA 2026 paper. We should pursue academic
  validation of the skill OS framing.
- **Action item**: Write up the skill lifecycle (teach → train → export →
  share → gate → execute → verify) as a position paper for ICRA/CoRL.

### From RoboNet (Orboh)
- **Visual skill browser**: Robot profiles, success rates, activity feeds.
  Our skill registry is on-chain but has no visual discovery layer.
- **HuggingFace Hub integration**: Distribute ONNX policies via HF Hub for
  versioning, download stats, and community visibility.
- **"Robots as users"**: The social network framing is approachable. Consider
  a Buzz channel where robots post skill execution results automatically.
- **Action item**: Add HuggingFace Hub upload to export_onnx.py (optional
  --push flag that uploads to a specified HF repo).
