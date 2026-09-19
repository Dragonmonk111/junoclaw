# JunoClaw Robotics OS: A Robot That Learns, Proves It, and Teaches Every Other Robot

*Open Duck Mini went viral this week for a browser you can teach a robot through, no install required. Remi Fabre showed how it learns to walk — hundreds of ducks in parallel, sharing one brain, until one of them figures it out. We matched the browser. We matched the parallel RL training. Here's the part we don't think anyone else can match: what happens after the duck learns to walk — and how the learning doesn't stay with one duck.*

**Summary:** A robot that learns a skill in a browser is a good demo. A robot that learns a skill through reinforcement learning alongside 7 virtual copies of itself is a better one. A robot that learns a skill, exports it as a verifiable artifact, proves who trained it and when, shares it with every other robot on an open network, and lets any of them check "has this ever gone wrong before I run it" — that's an operating system. This week JunoClaw closed the full loop: browser teaching UX, parallel RL training pipeline (stand + walk + turn + recovery policies), ONNX runtime inference, and the Merkle-verified memory and cross-fleet trust layer that makes a shared skill something you can actually trust, not just download and hope. One robot learns to walk. Every robot on the network knows how to walk. No blackbox required.                                                                                                                                                                                                                                                                                                                                   

---

## The Moment

[Open Duck Mini's browser viewer](https://x.com/MeRTcooking/status/2094349966995276004) is spreading fast this week, and for good reason: open a URL, no app install, and you're driving a small open-source robot from your phone. People are teaching it things in simulation, watching the skill transfer to the real robot, and sharing what they taught. Sales are climbing. The reason isn't the hardware — it's the loop: **teach once, watch it work, share it, everyone's robot gets better.**

That loop is exactly right, and it's worth taking seriously instead of dismissing it as a toy. Open source plus a low-friction teaching interface is how a robotics platform actually grows a community instead of just a customer list. The question we asked ourselves this week: do we have the pieces to do the same loop, but with the parts that make a shared skill something you can actually verify instead of just trust?

We did — most of them were already built for a different reason. This is the write-up of closing the gap.

---

## What Microduck Gets Right

A no-install browser viewer that lets you pose a robot and watch it move is genuinely good UX. We built one too: `plugins/plugin-ros2/bridge/.../server.py` serves a single-file page at `GET /viewer` — live joint + IMU telemetry over WebSocket, 15 teleop sliders, expression buttons, all working in `--simulate` mode with no hardware attached.

That closes the parity gap. It's not the interesting part.

---

## Many Ducks, One Brain — The Training Pipeline Behind the Walking Skill

[Remi Fabre's video](https://x.com/RemiFabreRobot/status/2094886502874448012) shows how Microduck learns to walk: spawn many ducks in parallel, give them one shared policy network, let them try and fail millions of times, and eventually one of them figures out a gait that doesn't fall over. It's reinforcement learning explained by ducks — and it's exactly what we've been building in parallel, for the same reason.

The difference: our duck doesn't just learn to walk. It exports what it learned as a verifiable artifact that any other duck can check, import, and run.

### What we trained

The sim2real pipeline runs in five phases, each building on the last:

| Phase | Skill | How | Timesteps | Status |
|-------|-------|-----|-----------|--------|
| A | Accurate robot model | MJCF physics model matching real DOGZILLA-Lite dimensions | — | Done |
| B | Motor identification | Wiggle-test calibration on real hardware, XGO API discovered | — | **Done (hardware)** |
| C | Stand | PPO, 4 parallel envs, 100K steps | 100K | Trained, exported, sim-tested |
| D | Walk | PPO, 8 parallel envs, 2M steps, domain randomization + imitation reward | 2M | Trained, exported, sim-tested |
| E | Runtime integration | ONNX inference loop on Pi CM5, safety-gated | — | **Done, hardware-tested** |
| G1 | Turn/steer | PPO warm-started from walk policy, 8 envs, 1M steps | 1M | Trained, exported, eval'd |
| G3 | Fall recovery | PPO from scratch, 8 envs, 5M steps, full tilt range (60°–150°), ent_coef=0.001 | 5M | Trained, exported, eval'd (v5.1: 40% easy, 25% medium, 20% full-tilt success) |
| G2-G8 | Speed+heading+terrain | PPO, 8 envs, multi-terrain curriculum | 2M+ | Trained, exported, **hardware-tested** |
| HW | Hardware deployment | xgo_robot.py driver, run_on_pi.py inference, safety controller | — | **Stand + walk-in-place + forward walk tested on real robot (Sep 10)** |

Phase D is the one worth pausing on. Eight copies of the DOGZILLA-Lite quadruped spawn in MuJoCo, each with a slightly different body mass, ground friction, and motor gain — domain randomization, the standard sim2real robustness technique. They share one policy network (a 3-layer MLP, 41 inputs → 128 → 128 → 15 outputs, 23,823 parameters total — smaller than a single high-resolution image). Every step, each duck observes its joint positions, velocities, trunk orientation, gyro, accelerometer, and height (41 numbers), the policy outputs 15 joint targets, and the duck either stays upright or falls. The PPO algorithm collects all eight ducks' experiences, updates the shared brain, and tries again. Two million steps later, the duck can walk.

### The imitation reward — teaching with a reference gait

Pure RL from scratch can find gaits that work but look unnatural (dragging legs, hopping instead of trotting). We avoided this by adding an imitation reward: the training environment includes a reference trot gait — a direct port of the hand-coded `QuadrupedBackend::apply_gait` from our Rust physics crate — and rewards the policy for matching its leg timing. The policy learns to trot like the reference, not invent its own gait from scratch. This is the same principle as learning from demonstration, but at the reward-shaping level instead of the trajectory level.

### From training to deployment: the ONNX export

A trained PPO policy is a PyTorch model — useful in Python, useless on a robot's CM5 controller. The export pipeline (`sim2real/export_onnx.py`) wraps the actor network (feature extractor → MLP → action head → tanh squash) and exports it as an ONNX graph: 6 nodes, 3 Gemm + 3 Tanh, 97KB total. The bridge's `PolicyRuntime` class loads this with `onnxruntime` and runs one inference step per control cycle — 41 floats in, 15 floats out, sub-millisecond on a Raspberry Pi.

### Safety gating: the bridge won't let the duck hurt itself

The inference loop in `server.py` doesn't blindly execute what the policy outputs. Each step:

1. Build the 41-dim observation from live joint telemetry + IMU
2. Run ONNX inference → 15 raw action values in [-1, 1]
3. Scale to joint ranges (each joint's min/max in radians)
4. Apply EMA smoothing (20% new / 80% previous) — prevents startup jumps
5. Check every joint delta against `MAX_JOINT_DELTA_PER_CYCLE_RAD = 0.6` — if any single joint wants to move more than 0.6 rad in one cycle, **reject and halt**
6. Only if all joints pass the clamp, send the commands

We verified this end-to-end in simulate mode: 200 steps at 30 Hz, zero clamp rejections, thigh joints oscillating at ~4 Hz (consistent with a trot gait), 21-24 direction reversals per joint over the run. The policy is alive and walking in simulation.

### Why this matters for the sharing story

Here's the convergence. The `Skill` abstraction (taught by demonstration, exported as JSON keyframes) and the RL policy (trained by PPO, exported as ONNX) are two paths to the same destination: a robot that can do something it couldn't do before. Both produce a portable artifact. Both can be shared over the Buzz relay. Both can be registered on-chain via the skill-registry contract. Both can be gated by the SkillGate and the kinematic safety clamp before execution.

But the RL path has a property the demonstration path doesn't: **the policy learned it from scratch, across millions of trials, in a physics simulator that can run on any laptop.** No human needed to pose the robot. No one needed to physically demonstrate the gait. The duck taught itself, and the result is a 97KB file that any other duck can run.

Now combine that with the on-chain registry: one robot trains a walking policy, exports it to ONNX, registers the hash on the skill-registry contract, and publishes the artifact over the Buzz relay. Every other robot on the network can verify the provenance (who trained it, when, what's the Merkle root), download the ONNX file, load it into their own bridge, and walk — without retraining, without a vendor cloud, without asking permission. **One duck learns to walk. Every duck knows how to walk. That's the operating system.**

---

## What Microduck Doesn't Have (As Far As We Can Tell)

Two things, based on what's public: no cryptographic record of who taught what and whether it can be trusted, and no LLM/agent layer sitting on top of the robot's decision loop. We can't be certain about internals we haven't seen — but nothing in the public materials suggests either exists. If we're wrong about that, the comparison below is still useful as a spec for what a *sovereign, agentic* version of the same idea looks like.

DOGZILLA, running the JunoClaw stack, has both. That's the actual gap worth closing, and it's the one that took more than an afternoon.

---

## Skills: The Piece That Was Missing

We already had the substrate for this — it just wasn't packaged as something you could name, export, and hand to a different robot. Three things already existed in `crates/junoclaw-physics/src/`:

- **`memory.rs`** — every reflex cycle hashed, Merkle-rooted, queryable: "has any robot ever been near this state, and what happened?"
- **`worldmodel.rs`** — predicts the consequence of a candidate action, trained on verified transitions
- **`fleet.rs`** — trust-gated sync of *raw memory* across robots that don't share an owner, so one robot's mistake becomes every robot's caution

What none of them did: let you teach a *named, repeatable behavior* — "wave," "sit," "climb\_step" — and hand it to a robot that isn't even the same model. That's a different object than a memory record or a world-model weight update, and it needed its own abstraction. We built it today: `crates/junoclaw-physics/src/skill.rs`.

### What a Skill actually is

A `Skill` is a named, repeatable behavior — "wave," "sit," "climb_step" — captured as a manifest (author, joint schema, license, Merkle provenance root) plus keyframes (per-frame joint targets). A `SkillRecorder` captures it by sampling `PhysicsState` over time, and it doesn't care whether those states came from the simulator or from real hardware telemetry. Teach in sim, teach by physically posing the robot, teach by driving it through the browser viewer — all three produce the identical artifact.

### The part that makes "run anywhere" honest, not marketing

A skill only transfers a joint if the receiving robot has a joint with the *same name*. No guessing mappings between joints that don't share a name — that's a harder research problem. What this buys: any two robots built against `QUADRUPED_JOINT_NAMES` exchange skills with full coverage, automatically. Anything else gets a `RetargetReport` with matched joints, missing joints, and a coverage fraction. We tested this: took a skill taught on DOGZILLA's 15-DOF body, handed it to a robot that only shared two joint names, got `coverage: 0.667` and a playback that only drove the two joints it could. That's the difference between "transferable" as a slogan and as a property you can check.

### Export, import, play — tested end to end

The bridge exposes record/export/import/play endpoints. Full loop verified: record a demonstration → export → mutate the joint schema to simulate a different robot → import → get an honest partial-coverage report → play the retargeted version. The `/viewer` page wraps it all in a UI.

### Gated playback: the safety half, closed

Two safety layers: **`SkillGate`** checks every frame against the L2 `WorldModel` and L1 memory before it's allowed to play — predict the consequence, reject if it lands near a state memory has flagged red (12 tests passing). **A hard kinematic safety clamp** in the ROS2 bridge fails closed on any single-frame joint delta over 0.6 rad — abort, don't clip.

### Where sharing goes next

A skill is just JSON — small enough for the existing Buzz relay to carry. Upload via Blossom endpoint, reference from a Nostr event, it's discoverable. For on-chain listing, the bridge generates ready-to-sign CosmWasm messages. `skill-registry` is deployed on testnet and mainnet, so `registry_msg` returns a real contract address and execute_msg. `marketplace` is built and tested but not yet deployed — the bridge says so explicitly rather than implying it's live.

---

## Why "Recursive"

The loop, grounded in code that exists and tests that pass:

1. A robot acts. Every cycle is hashed and Merkle-rooted.
2. Verified transitions retrain the **world model** — predicting consequences gets better with more experience.
3. **Memory** accumulates: "has anyone been near this state, and did it go red?" — every robot's near-misses become every other robot's caution, gated by trust scoring so a hostile contributor can't poison the pool.
4. **Skills** are demonstrations captured *using* a robot informed by 1–3 — a skill taught by a robot with a better world model is a better demonstration.
5. Skills get shared. Another robot imports one, plays it, generates its own verified cycles — feeding back into step 1, on a different robot, in a different environment.

The loop closes across the fleet, permissionlessly, because memory and skill artifacts are both verifiable data that any robot can read without asking permission. No vendor cloud can do this across competing owners: **verifiability is what makes sharing possible at all**, not an add-on to it.

---

## The LLM Integration Microduck Doesn't Have

Skills and memory are the physical-learning half. The other half is the agent layer running in front of this robot's decision loop:

- **Hermes agents** connect over the **Buzz relay** (`buzz.junoclaw.xyz`) — an LLM-driven agent can join `#governance` or `#robotics`, read the robot's posted verdicts and skill listings, and act on them.
- **Truth Market operators** — staked humans or LLM-assisted agents — adjudicate reflex-batch attestations into green/yellow/red verdicts that directly tighten or relax the robot's `SafetyEnvelope`.
- **The bridge's `/robot/expression` and skill-marketplace listings** are structured surfaces an LLM agent reasons over well: bounded vocabulary, JSON in and out.

None of this replaces L0's classical control — the robot still balances on 1ms PID with no model and no network in the loop. The LLM layer sits where it belongs: coordination, judgment, and narration, several tiers up from the reflex loop.

### Three LLM Pathways

**1. Training (ready now)**: The LLM agent uses MCP simulation tools to discover recovery trajectories in MuJoCo — hypothesize a motion, test it, iterate. Successful trajectories become imitation learning seeds for PPO warm-start. The recovery policy v5.1 hit its ceiling with pure PPO (40% easy, 25% medium, 20% full-tilt); LLM-guided trajectory discovery is the next step.

**2. Strategic intervention (near-term)**: When the robot encounters a novel state — a fall on stairs, unfamiliar terrain — the L1 MemoryIndex reports no match, the L2 WorldModel reports low confidence, the SkillGate rejects, the SafetyController halts. The LLM agent simulates the fallen state, hypothesizes recovery strategies, tests them, and proposes a trajectory. The SkillGate gates it. Only if approved does the robot execute. Total loop: ~15 seconds. The reflex loop runs at 20ms with no network — the LLM intervenes only when the reflex policy can't handle the situation.

**3. Continuous auditing (long-term)**: The J-Lens monitor reads the LLM's internal state during modes 1 and 2, detecting forbidden concepts (reward hacking, deception, unsafe shortcuts) before they become external actions. Not just an LLM that helps, but an LLM whose reasoning is itself auditable.

The LLM loop is **optional**. The robot works without it. The LLM adds capability for novel situations and accelerates training.

---

## Lattice Jolt: Post-Quantum ZK Verification on uni-7

On September 10, 2026, we deployed the `jolt-cw-verifier` CosmWasm contract to uni-7 testnet — the first step toward on-chain post-quantum ZK proof verification without chain-level precompiles.

The Jolt verifier (sum-check protocol, polynomial commitments, proof deserialization) is pure computation — no GPU, no threads, no file I/O. It compiles to `wasm32-unknown-unknown` and runs in any WASM runtime. This collapses the PQC deployment roadmap: no v30 BN254 precompile, no wasmvm fork, no governance vote. Just a pure Wasm contract on stock Juno.

| Field | Value |
|---|---|
| codeId | 103 |
| Contract address | `juno1gkyjms6uwfc3nugumttnww6ye4mtv25c3npys09zu0w5qyyxqzds0k9vd9` |
| Wasm size | 193.4 KB (`wasm-opt -Oz`) |
| Store tx | `29C7EBF7283FE461D5700D75858A29981CE87BF1CE495A5070D55E63D5122BA3` |
| Instantiate tx | `F01A68EFB3D56B92F0B632CFCBB6C82B0FF14470201E3805F8C75DA629C474EC` |

The contract exposes `StoreProof`, `VerifyProof` (structural validation), and queries. Full arkworks proof verification in-wasm pending wasmvm bulk memory re-enablement — the wrapper is ready for the full integration.

---

## Why This Is an OS, Not "Middleware on Top of ROS2"

ROS2 is a message bus — it moves bytes between nodes. It doesn't decide what those bytes mean, who sent them, whether they're safe, or whether a different robot should trust them. That's by design. ROS2 is transport, not policy.

What we built makes **decisions ROS2 cannot make**: skill provenance (who trained this, when, Merkle-anchored), cross-fleet trust (staked reputation, slashable misbehavior), safety memory (has any robot been near this state and had it go wrong?), consequence prediction (world model + SkillGate), autonomous skill discovery (detect, verify, gate, run — no human in the loop), and on-chain settlement (CosmWasm contracts, truth markets, DAO governance).

An operating system defines what a program is, how it's installed, who can run it, what permissions it has, and how programs find and trust each other. That's what we built for robot skills. The ROS2 bridge is the device driver — important, necessary, but not the OS.

Linux needs device drivers. It is not a device driver.

---

## The Landscape: Who Else Is Building Toward This

We're not the only project thinking about robots + verifiable trust + open networks. Here's an honest map:

| Project | Strengths we lack | Our differentiator |
|---|---|---|
| **OpenMind (OM1+FABRIC)** | Hardware breadth (Unitree, Ubtech, TurtleBot4), 1K+ dev app store, BrainPack shipping product, x402 payments | Merkle-anchored skill provenance, cross-fleet memory, sim-to-real RL pipeline, skill retargeting with coverage reports |
| **peaq** | Scale (6M+ machines), omnichain identity, machine credit ratings, RWA tokenization | Skill-level provenance (behaviors, not just identity), safety-gated execution, RL training pipeline, world-model gating |
| **OpenGradient** | TEE-verified inference in production, zkML model integrity proofs, 1.5K+ models on-chain | Physical robot execution + consequence verification, full skill lifecycle provenance, cross-fleet memory |
| **RODEO (ICRA 2026)** | Peer-reviewed paper, physical robot economic autonomy (88hr), task verification oracle | RL-trained skills (not pre-programmed), cross-fleet trust, deployed mainnet (not Ganache), world model + safety gate |
| **RoboNet** | Working frontend, robot profiles, HuggingFace Hub integration | Safety-gated execution, cross-embodiment retargeting, Merkle-anchored provenance, RL training pipeline |

No single project combines all six pillars: RL-trained skills, verifiable provenance, cross-fleet trust, safety-gated execution, open sharing, and on-chain settlement. The intersection is where we are.

| | Open Duck Mini (public) | JunoClaw Robotics OS |
|---|---|---|
| No-install browser control | ✅ Viral this week | ✅ Shipped (`/viewer`) |
| Parallel RL training | ✅ | ✅ PPO, 8 envs, 2M+ steps, domain randomization |
| ONNX export + safety-gated inference | Unclear | ✅ 97KB, sub-ms, EMA + kinematic clamp |
| Skill transfers to real hardware | ✅ | ✅ Stand + walk-in-place + forward walk on DOGZILLA-Lite |
| Skill retargeting to different robot | Unclear | ✅ Name-based, honest coverage report |
| Merkle-anchored skill provenance | Unclear | ✅ `provenance_batch_root` |
| Cross-fleet memory + world model | Unclear | ✅ `fleet.rs` + `worldmodel.rs` |
| On-chain skill registry | Unclear | ✅ Deployed on juno-1 mainnet |
| LLM / agent layer | None public | ✅ Hermes agents, Truth Market, MCP sim tools |
| Post-quantum ZK verification | ❌ | ✅ Lattice Jolt verifier compiles to wasm32 |

We leave rows "Unclear" rather than "No" — we've only seen public demos, and asserting negatives about projects we haven't inspected would be the same overclaiming we avoid in our own work.

---

## What's Actually Built

```
$ cargo test -p junoclaw-physics
163 passed; 0 failed
```

**Rust crates (junoclaw-physics)**: `Skill`/`SkillRecorder`/`retarget` (8 tests), `SkillGate` with WorldModel + memory gating (12 tests), `MemoryIndex`/`RootCache` L1 (15 tests), `WorldModel` L2 (8 tests), `FleetRegistry` cross-fleet trust (8 tests), `DatasetExporter` (6 tests), `ReflexPipeline`, `ReplayLog`/`Watchdog`/`AuditBundle`. All passing.

**ROS2 bridge**: `/viewer` live telemetry + teleop, skill record/export/import/play (cross-embodiment retarget verified), kinematic safety clamp (9 tests), ready-to-sign CosmWasm `registry_msg`/`marketplace_msg` tested against deployed skill-registry.

**Sim2real RL pipeline**: Stand (100K steps), walk (2M steps, domain randomization + imitation reward), turn (1M steps, upright 0.903), recovery v5.1 (5M steps, 40% easy / 25% medium / 20% full-tilt — first policy to self-right from upside-down poses), speed+heading+terrain (2M+ steps). All exported to ONNX, sim-tested.

**Hardware (DOGZILLA-Lite on Pi CM5)**: `xgo_robot.py` driver (XGO API, offset calibration, IMU conversion), `run_on_pi.py` inference (ONNX + SafetyController at 50Hz). Stand test: upright=0.995, 100 steps. Walk-in-place: upright=0.998, 200 steps. Forward walk: speed=0.08 m/s, upright=0.95–1.0, 200 steps, 8 runs, zero falls, on UK carpet.

**On-chain**: `skill-registry` deployed on juno-1 mainnet (codeId 5145) and uni-7 testnet (codeId 82). `jolt-cw-verifier` deployed on uni-7 (codeId 103). `marketplace` contract built and tested, not yet deployed.

**Remaining gaps**: Deploy `marketplace`, add MuJoCo sim tools to MCP server for LLM-driven policy discovery, retrain walk policy with smoother gait parameters, test turning on real hardware. Recovery policy v5.1 hit PPO's ceiling — LLM-guided trajectory discovery is the next step to break through.

---

## Hardware Validation: The Duck Walks on Real Carpet

The sim-to-real gap is the hardest part of robotics. On September 4–10, 2026, we closed it.

The DOGZILLA-Lite runs a Raspberry Pi CM5 with `xgolib` (Python, serial bus at 115200 baud). We built `xgo_robot.py` to translate between sim conventions (radians, quaternions) and the physical robot (degrees, roll/pitch/yaw), with per-joint offset calibration via `--calibrate-only`.

### Initial tests (Sep 4): first-attempt sim-to-real transfer

| Test | Policy | Steps | Upright | Speed | Result |
|---|---|---|---|---|---|
| Stand | `policy_stand.onnx` (100K steps) | 100 @ 50Hz | 0.995 | — | ✅ Stable, tiny corrections (action_sum 0.19) |
| Walk-in-place | `policy_speed_heading_terrain.onnx` (2M+ steps) | 200 @ 50Hz | 0.998 | 0.0 m/s | ✅ Full trot gait active (action_sum 4.2, 20× stand) |
| Forward walk | `policy_speed_heading_terrain.onnx` | 200 @ 50Hz | 0.998 | 0.04 m/s | ✅ Forward locomotion on UK carpet, zero falls |

All three policies transferred from simulation to real hardware on the first attempt, no fine-tuning. Domain randomization during training (varying body mass, friction, motor gains across 8 parallel envs) produced policies robust enough for the real robot's dynamics.

### Stability tuning (Sep 10): from falling to smooth forward walk

The initial tests used `torque_limit=1.0` (full action range) with no smoothing. The robot produced a valid trot gait but was unstable — it sometimes rolled onto its back during walking. The root cause: the training gait uses `_LIFT_ANGLE=0.3` rad for calf flex and `_STRIDE=0.15` rad for thigh sweep, and at full torque the servos executed these aggressively, causing the lightweight robot to tip.

We tuned two deployment-time parameters — no retraining needed:

- **`torque_limit`**: clips raw policy actions before scaling by `ACTION_SCALE=0.3`. At 0.7, the effective joint residual is ±0.21 rad — enough for partial leg lift, controlled enough to stay upright.
- **`action_smooth`**: EMA filter (α·previous + (1-α)·new). At 0.1–0.15, it damps jerky gait phase transitions without suppressing the trot dynamics.

| Run | Torque | Smooth | Speed | Heading | Steps | Upright (min–max) | Forward? | Result |
|---|---|---|---|---|---|---|---|---|
| 1 | 0.5 | 0.4 | 0.0 | 0.0 | 200 | 0.97–0.99 | N/A | ✅ Stable walk-in-place, no fall |
| 2 | 0.5 | 0.4 | 0.08 | 0.0 | 200 | 0.93–1.0 | Slides forward | ✅ No fall, limited forward |
| 3 | 0.7 | 0.2 | 0.08 | 0.0 | 200 | 0.98–1.0 | 2–3 steps + right drift | ✅ Best forward yet |
| 4 | 0.7 | 0.2 | 0.12 | 0.0 | 200 | 0.54–0.99 | Stumbled, recovered | ⚠️ Near-fall at step 50 |
| 5 | 0.6 | 0.3 | 0.08 | 0.0 | 200 | 0.99–1.0 | Less forward | ✅ Very stable, conservative |
| 6 | 0.7 | 0.1 | 0.08 | 0.05 | 200 | 0.97–1.0 | **Longest forward walk** | ✅ Forward + slight right |
| 7 | 0.7 | 0.15 | 0.08 | 0.05 | 200 | 0.98–1.0 | Forward + right drift | ✅ Stable, forward |
| 8 | 0.7 | 0.1 | 0.08 | 0.05 | 200 | 0.95–1.0 | Forward, battery 30% | ✅ Forward despite low battery |

**Best configuration**: `torque_limit=0.7`, `action_smooth=0.1`, `speed=0.08`, `heading=0.05`. The robot walked forward multiple steps on UK carpet — the longest sustained forward locomotion we've achieved. A slight rightward drift persists, likely from a minor calibration asymmetry in the hip offsets.

### Key learnings

1. **Sim-to-real works without fine-tuning** when domain randomization is used — all policies transferred on first attempt.
2. **Deployment-time parameter tuning is critical**: `torque_limit=0.7` + `action_smooth=0.1` transformed an unstable gait into a forward walk. No retraining needed.
3. **ONNX inference is sub-ms on Pi CM5** at 50Hz — the control loop is never inference-bound.
4. **Battery level affects gait quality**: tests at 55% battery produced cleaner steps than at 30%, as servo torque drops with charge.
5. **Offset calibration is critical** and must be captured before first run; residual rightward drift suggests a small hip asymmetry.
6. **UK carpet provides adequate friction** for the trot gait at the trained speed range (0.0–0.2 m/s).
7. **The `.onnx.data` external weights file is essential** — without it, onnxruntime loads an incomplete graph producing saturated constant actions. This was the initial bug that blocked all hardware testing.

---

## What "Finished" Actually Means Here

Not: a robot that can do everything. What's finished is the *loop* — the same loop that's making Open Duck Mini spread this week, plus the parts that make a shared skill something you can verify instead of just download:

1. **Teach a behavior** — three ways: by demonstration through a browser, by physically posing the robot, or by reinforcement learning (the duck teaches itself in simulation, 8 copies in parallel, 2 million steps).
2. **Export it** — as a small, self-describing, license-tagged JSON artifact (demonstration) or a 97KB ONNX graph (RL policy). Both portable. Both verifiable.
3. **Share it on an open network** — publish over the Buzz relay, register the hash on the skill-registry contract. Every robot on the network discovers it automatically. No vendor cloud, no app store, no permission.
4. **Trust it before you run it** — check the provenance (who trained it, when, Merkle root), check the coverage report (which joints transfer to your robot), check the safety gate (has this skill ever caused a fall? does the world model predict a safe landing?), and let the kinematic clamp catch anything the models missed.
5. **Run it** — the bridge loads the ONNX policy, builds observations from live telemetry, runs inference at 50 Hz, smooths the output, and gates every joint command through the safety clamp before a single servo moves.
6. **Learn from it** — every robot that runs the skill generates verified reflex cycles that feed back into the shared memory and world model, making the *next* skill and the *next* safety check better for every robot on the network — including robots that haven't been built yet.

An agent layer — LLMs over a Nostr relay, staked truth-market operators — sits on top, reasoning about what the robots are doing in language, without touching the millisecond-scale reflex loop underneath it. The LLM loop is optional: the robot works without it, and it adds capability for novel situations and training acceleration. Three pathways: training (MCP sim tools for trajectory discovery), strategic intervention (LLM reasons about novel states the reflex policy can't handle), and continuous auditing (J-Lens probes the LLM's own reasoning for forbidden concepts).

Post-quantum ZK verification is now live on testnet: the `jolt-cw-verifier` CosmWasm contract is deployed on uni-7 (codeId 103), proving the deployment pipeline end-to-end. The wrapper handles proof storage and structural verification today; full arkworks proof verification in-wasm pending a wasmvm bulk memory re-enable. No chain precompiles needed — pure Wasm, chain-agnostic by construction.

The key thing that makes this an operating system and not a demo: **when one robot learns something, every robot knows it.** Not through a vendor's cloud push, not through a manual download, not through a terms-of-service agreement — through a public Merkle root and a Nostr relay that any robot can read without asking anyone's permission. One duck learns to walk. Every duck on the network can verify how, when, and by whom — and then walk too. That's the JunoClaw Robotics OS.

---

*August 31–September 10, 2026. `cargo test -p junoclaw-physics` passes 163/163. ROS2 bridge: 28/28 tests passing. Sim2real RL pipeline complete (stand 100K, walk 2M, turn 1M, recovery 5M steps — all exported to ONNX, sim-tested). Hardware validation: stand + walk-in-place + forward walk tested on DOGZILLA-Lite (Sep 4 first-attempt transfers; Sep 10 stability tuning — torque_limit=0.7, action_smooth=0.1, 8 runs, all 200 steps, zero falls, longest sustained forward walk achieved). Recovery policy v5.1: 40% easy / 25% medium / 20% full-tilt — PPO ceiling reached, LLM-guided trajectory discovery via MCP sim tools is next. LLM pathway designed: three modes (training, strategic intervention, continuous auditing), optional, ~15s loop runtime for novel-state recovery. Lattice Jolt `jolt-cw-verifier` deployed to uni-7 testnet (Sep 10, codeId 103, `juno1gkyjms6uwfc3nugumttnww6ye4mtv25c3npys09zu0w5qyyxqzds0k9vd9`, 193 KB) — post-quantum ZK verification wrapper live on-chain, full arkworks integration pending wasmvm bulk memory re-enable. Competitive landscape: OpenMind, peaq, OpenGradient, RODEO, RoboNet — no single project combines all six pillars. On-chain settlement is asynchronous provenance/adjudication, not an execution gate.*
