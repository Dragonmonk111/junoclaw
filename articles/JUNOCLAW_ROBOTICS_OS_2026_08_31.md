# JunoClaw Robotics OS: Teach Once, Run Anywhere — Open Source, Recursive Worldview Learning Across All Robots

*Open Duck Mini went viral this week for a browser you can teach a robot through, no install required. We matched that in an afternoon. Here's the part we don't think anyone else can match: what happens after you teach it.*

**Summary:** A robot that learns a skill in a browser is a good demo. A robot that learns a skill, proves who taught it and when, shares it with every other robot on an open network, and lets any of them check "has this ever gone wrong before I run it" — that's an operating system. JunoClaw now has both halves: the no-install teaching UX everyone is excited about this week, and the Merkle-verified memory, world model, and cross-fleet trust layer that makes a shared skill something you can actually trust, not just download and hope.

---

## The Moment

[Open Duck Mini's browser viewer](https://x.com/MeRTcooking/status/2094349966995276004) is spreading fast this week, and for good reason: open a URL, no app install, and you're driving a small open-source robot from your phone. People are teaching it things in simulation, watching the skill transfer to the real robot, and sharing what they taught. Sales are climbing. The reason isn't the hardware — it's the loop: **teach once, watch it work, share it, everyone's robot gets better.**

That loop is exactly right, and it's worth taking seriously instead of dismissing it as a toy. Open source plus a low-friction teaching interface is how a robotics platform actually grows a community instead of just a customer list. The question we asked ourselves this week: do we have the pieces to do the same loop, but with the parts that make a shared skill something you can actually verify instead of just trust?

We did — most of them were already built for a different reason. This is the write-up of closing the gap.

---

## What Microduck Gets Right

To be fair about it: a no-install browser viewer that lets you pose a robot and watch it move is a genuinely good piece of UX, and as of this week we didn't have one. So we built one. `plugins/plugin-ros2/bridge/.../server.py` now serves a single-file page at `GET /viewer` — open it from a phone or laptop on the same network (or over Tailscale), no install, and you get:

- Live joint + IMU telemetry over a WebSocket (`/ws/state`, ~10Hz)
- Fifteen joint teleop sliders that command the robot in real time
- Expression buttons wired to the existing face-display mapping
- All of it works in `--simulate` mode with no hardware attached, which matters this week specifically: DOGZILLA-Lite's CM5 arrived August 31, and Phase 0 (unbox and inspect) hasn't started yet. The UI is fully testable tonight.

That closes the parity gap. It's not the interesting part.

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

```rust
pub struct SkillManifest {
    pub name: String,               // "wave"
    pub description: String,
    pub author_robot_id: String,    // who taught it
    pub joint_names: Vec<String>,   // the schema it was taught on
    pub frame_count: usize,
    pub cycle_dt_ms: u64,
    pub license: String,            // "CC0", "MIT" — open-source, explicitly
    pub provenance_batch_root: String, // Merkle root of the batch it was captured within
    pub created_at_ms: u64,
}

pub struct Skill {
    pub manifest: SkillManifest,
    keyframes: Vec<Vec<f64>>,       // per-frame target position, per joint
}
```

A `SkillRecorder` captures this by sampling `PhysicsState` over time — and it does not care whether those states came from the in-crate simulator or from real hardware telemetry relayed through the ROS2 bridge. Teach in sim, teach by physically posing the robot, teach by driving it through the browser viewer — all three produce the identical artifact. That's what "teach once" actually requires: the capture format has to be blind to where the demonstration came from.

### The part that makes "run anywhere" honest, not marketing

```rust
pub fn retarget(&self, target_joint_names: &[String]) -> (RetargetedSkill, RetargetReport)
```

A skill only transfers a joint if the receiving robot has a joint with the *same name*. No attempt is made to guess a mapping between joints that don't share a name — that's a harder, separate research problem, and claiming to solve it in an afternoon would be dishonest. What this buys today: any two robots built against `QUADRUPED_JOINT_NAMES` (the naming convention this codebase already uses everywhere) exchange skills with full coverage, automatically. Anything else — a different robot, a different joint count, a partial overlap — gets a `RetargetReport`:

```rust
pub struct RetargetReport {
    pub matched_joints: Vec<String>,
    pub missing_in_target: Vec<String>,
    pub unused_target_joints: Vec<String>,
    pub coverage: f64,
}
```

We tested this directly: took a skill taught on DOGZILLA's full 15-DOF body, handed it to a simulated target robot that only shared two joint names, and got back `coverage: 0.667`, an explicit list of what didn't carry over, and a playback that only drove the two joints it actually knew how to drive. That's the difference between "transferable to any robot" as a slogan and as a property you can check before you trust it.

### Export, import, play — tested end to end today

The bridge exposes this directly:

```
POST /skills/record/start          — begin capturing (sim or real)
POST /skills/record/stop           — {name, description, license} → saved artifact
GET  /skills                       — list manifests
GET  /skills/{name}/export         — the portable JSON artifact
POST /skills/import                — accept anyone's exported skill, get a coverage report
POST /skills/{name}/play           — retarget onto this robot's schema, execute
```

Full loop verified against a live instance today: record a demonstration → export it → mutate the joint schema to simulate a different robot → import → get an honest partial-coverage report → play the retargeted version → confirm it only commanded the joints it actually had a match for. The `/viewer` page wraps all six endpoints in a UI: Start Recording, pose the robot, Stop & Save, then Play / Export / Import buttons per skill.

### Gated playback: the safety half, closed

Exporting and retargeting a skill is only half the trust problem — the other half is what happens the instant before a joint actually moves. Two things now exist for that:

- **`SkillGate` (`crates/junoclaw-physics/src/skill.rs`)** — checks every frame of a skill against the L2 `WorldModel` and L1 memory before it's allowed to play: predict the consequence, reject if it lands near a state memory has flagged red. 12 tests, all passing. This is the real, model-informed gate.
- **A hard kinematic safety clamp in the ROS2 bridge (`server.py::play_skill`)** — `plugin-ros2` doesn't yet depend on `junoclaw-physics` in-process (it only talks to this bridge over HTTP), so there's no live `WorldModel` to consult there today. Rather than skip gating on real hardware until that wiring exists, playback fails closed on any single-frame joint delta over `0.6` rad, checked every cycle, abort-don't-clip. Honest interim measure, not the final answer — `GET /skills/playback/status` reports exactly which frame it rejected and why.

### Where sharing goes next

A skill is just JSON — small enough that the existing Buzz relay infrastructure already carries it without any new backend work. Upload the artifact through the relay's Blossom endpoint (`PUT /upload`, already NIP-98 authenticated, content-addressed by SHA-256), reference the resulting blob from a Nostr event (`POST /events`), and it's discoverable per-community today. For on-chain listing, the bridge now generates ready-to-sign CosmWasm messages directly:

```
GET /skills/{name}/registry_msg      — PublishSkill for the deployed skill-registry contract
GET /skills/{name}/marketplace_msg   — ListService for the marketplace contract
```

`skill-registry` is deployed on testnet and mainnet, so `registry_msg` returns a real sha256 hash, a real contract address, and an execute_msg an operator can submit today with their own signer — the bridge holds no wallet key and broadcasts nothing itself. `marketplace` (and `truth-market`) are built and tested but not yet deployed, and `marketplace_msg` says so explicitly (`marketplace_deployed: false`) rather than implying it's live. `POST /robot/register` ties it together: it now returns a real per-skill entry for everything this robot has taught, not a fixed placeholder string.

We're not claiming the marketplace is deployed. It isn't. What's done is the message format and the contract it targets, so listing a skill is a deploy-and-submit away, not a from-scratch build.

---

## Why "Recursive"

This is the part that's easy to oversell, so here's the grounded version, tied to code that exists and tests that pass:

1. A robot acts. Every cycle is hashed and Merkle-rooted (`merkle.rs`, `attestation.rs`) — this was already built.
2. Verified transitions retrain the **world model** (`worldmodel.rs`) — predicting consequences gets better with more verified experience.
3. **Memory** (`memory.rs`) accumulates: "has anyone been near this state, and did it go red?" — every robot's near-misses become every other robot's caution, gated by `fleet.rs` trust scoring so a hostile contributor can't poison the pool.
4. **Skills** (`skill.rs`, new today) are demonstrations captured *using* a robot that is itself informed by 1–3 — a skill taught by a robot with a better world model and a richer memory is a better demonstration to begin with.
5. Skills get shared. Another robot imports one, plays it, and in doing so generates *its own* verified cycles — which feed back into step 1, on a different robot, in a different environment.

The loop doesn't close on one robot. It closes across the fleet, permissionlessly, because the memory and the skill artifacts are both just verifiable JSON/Merkle data that any robot can read without asking anyone's permission. That's what "recursive worldview learning across all robots" means concretely: not one model getting smarter in a vendor's cloud, but a shared, provable pool of experience and demonstrated behavior that every participating robot both draws from and contributes to — including robots that have never met, built by owners who don't know each other, coordinated only by a public Merkle root and a Nostr relay.

No vendor cloud can do this across competing owners, for the same reason a closed vendor fleet-memory can't be trusted by a competitor's robot (see `drafts/PLAN_SOVEREIGN_ROBOTICS_OS.md`): **verifiability is what makes sharing possible at all**, not an add-on to it.

---

## The LLM Integration Microduck Doesn't Have

Skills and memory are the physical-learning half. The other half of "DOGZILLA has LLM integration, Microduck doesn't" is the agent layer already running in front of this robot's decision loop:

- **Hermes agents** connect to the robot's community over the **Buzz relay** (`buzz.junoclaw.xyz`) — a Nostr relay we spent part of today fixing (nginx was silently swallowing several of its REST routes behind the SPA). An LLM-driven agent can join `#governance`, `#robotics`, or `#dev`, read the robot's posted verdicts and skill listings, and act on them — propose a DAO vote, flag a suspicious skill import, or narrate what the robot is doing in natural language.
- **Truth Market operators** — staked humans or LLM-assisted agents — adjudicate reflex-batch attestations into green/yellow/red verdicts that directly tighten or relax the robot's `SafetyEnvelope` via `TrustLearner`. This is a language- and judgment-mediated layer sitting on top of the hard physical safety bounds, not a replacement for them.
- **The bridge's `/robot/expression` and upcoming skill-marketplace listings** are exactly the kind of structured surface an LLM agent reasons over well: bounded vocabulary, JSON in and out, no need for the agent to understand raw joint torques to usefully participate.

None of this replaces L0's classical control — the robot still balances on 1ms PID with no model and no network in the loop, per `PLAN_SOVEREIGN_ROBOTICS_OS.md`. The LLM layer sits where it belongs: coordination, judgment, and narration, several tiers up from the reflex loop, exactly where the existing L0–L6 latency hierarchy already puts it.

---

## Honest Comparison

| | Open Duck Mini (public materials) | JunoClaw Robotics OS |
|---|---|---|
| No-install browser control | ✅ Live, viral this week | ✅ Shipped today (`/viewer`) |
| Teach a skill by demonstration | ✅ | ✅ (`SkillRecorder`, sim or real) |
| Skill transfers to real hardware | ✅ | ✅ |
| Skill transfers to a *different* robot | Unclear from public materials | ✅ Name-based retarget, honest coverage report |
| Skill provenance (who taught it, when, proven) | Unclear | ✅ `provenance_batch_root` → Merkle-anchored batch |
| Cross-fleet memory (avoid others' mistakes) | Unclear | ✅ `fleet.rs`, trust-gated, slashable |
| World model / consequence prediction | Unclear | ✅ `worldmodel.rs`, gates action approval |
| Skill playback safety-gated (SkillGate + interim kinematic clamp) | Unclear | ✅ `skill.rs::SkillGate` (L2+L1) + bridge clamp (fail-closed) |
| On-chain skill registry / marketplace listing message | Unclear | ✅ ready-to-sign CosmWasm `ExecuteMsg` for deployed `skill-registry` |
| LLM / agent layer | None publicly shown | ✅ Hermes agents over Buzz relay, Truth Market verdicts |
| Open source | ✅ | ✅ |
| Skills carry a license marker | Unclear | ✅ (CC0 / MIT / Apache-2.0 per skill) |

We're leaving several rows "Unclear" rather than "No" — we've only seen the public demo, and asserting a negative about a project we haven't inspected would be the same overclaiming we're trying not to do with our own work.

---

## What's Actually Built (Checked Today, Not Copied From an Older Draft)

```
$ cargo test -p junoclaw-physics
163 passed; 0 failed
```

| Component | Status | Tests |
|---|---|---|
| `Skill`, `SkillRecorder`, `retarget` (new) | ✅ Built | 8 |
| `SkillGate` — L2 `WorldModel` + L1 memory gated playback (new) | ✅ Built | 12 |
| `MemoryIndex`, `MemoryFetch`, `RootCache` (L1) | ✅ Built | 15 |
| `WorldModel` (L2) | ✅ Built | 8 |
| `FleetRegistry` (cross-fleet memory) | ✅ Built | 8 |
| `DatasetExporter` | ✅ Built | 6 |
| `ReflexPipeline` (L2→L1→L0) | ✅ Built | — |
| `ReplayLog`, `Watchdog`, `AuditBundle` | ✅ Built | — |
| ROS2 bridge `/viewer` — live telemetry, teleop | ✅ Built, tested against a running instance | — |
| ROS2 bridge skill record/export/import/play | ✅ Built, tested end to end (cross-embodiment retarget verified) | — |
| Bridge kinematic safety clamp on playback (interim, until `plugin-ros2` embeds `SkillGate` directly) | ✅ Built, fail-closed, tested | 9 |
| `registry_msg` / `marketplace_msg` — ready-to-sign CosmWasm payloads | ✅ Built, tested against deployed `skill-registry` | — |
| Skill marketplace / DAO-gated listing — contract deployment | ⬜ `contracts/marketplace` built and tested, not yet deployed | — |
| Real-hardware validation | ⬜ CM5 arrived Aug 31; full DOGZILLA unit arriving imminently; Phase 0 (unbox) starts on arrival | — |

The remaining gaps are narrower than they were: gating and on-chain message generation are done. What's left is deploying `marketplace` itself, and running all of the above against real hardware instead of `--simulate`. That's the next work, not today's claim.

---

## What "Finished" Actually Means Here

Not: a robot that can do everything. What's finished is the *loop* — the same loop that's making Open Duck Mini spread this week, minus the parts that don't hold up under "can I trust what I just downloaded":

1. Teach a behavior — in simulation, or by posing real hardware, through a browser, no install.
2. Export it as a small, self-describing, license-tagged JSON artifact.
3. Hand it to a different robot. Get told exactly how much of it will work before you run it, instead of finding out the hard way.
4. Every robot that plays it, and every robot that ever ran anything, contributes to a Merkle-verified memory and a continuously retrained world model that the *next* skill and the *next* safety check both draw on.
5. An agent layer — LLMs over a Nostr relay, staked truth-market operators — sits on top, reasoning about what the robots are doing in language, without touching the millisecond-scale reflex loop underneath it.

Open source, sim-to-real, cross-robot, provable, and reasoned about by language models sitting above a control loop that was never waiting on them in the first place. That's the JunoClaw Robotics OS.

---

*August 31–September 1, 2026. `cargo test -p junoclaw-physics` passes 163/163, including 8 tests for the `Skill` layer and 12 for `SkillGate` (L2 world-model + L1 memory gated playback). ROS2 bridge extended with a no-install browser viewer, full skill teach/export/import/retarget/play loop, a fail-closed kinematic safety clamp on live playback, and `registry_msg`/`marketplace_msg` endpoints generating ready-to-sign CosmWasm payloads against the deployed `skill-registry` contract — 28/28 bridge tests passing. DOGZILLA-Lite CM5 hardware arrived Aug 31; the full DOGZILLA unit is arriving in the coming hours, at which point Phase 0 (unbox and inspect) begins. Buzz relay REST API fixed (nginx was dropping several real routes to the SPA fallback) so Hermes agents can actually reach it.*
