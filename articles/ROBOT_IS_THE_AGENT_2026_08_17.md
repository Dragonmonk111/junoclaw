# The Robot Is the Agent

*August 17, 2026 — JunoClaw was built as an agent trust OS. It turns out a robot was never a special case — it's just another fleet member emitting intents into the same gate. Seven things that were true in the code before they were true in the pitch deck, now verified, built, and deployed on uni-7.*

---

## TL;DR

JunoClaw was built as an agent trust OS. It turns out a robot was never a special case — it's just another fleet member emitting intents into the same gate. This piece works through seven things that were true in the code before they were true in the pitch deck: robots are agent-fleet extensions (with a typed `IntentMessage` schema that makes the reflex-tier / intent-tier split real), the skill-registry and the marketplace are two halves of the same discovery/pay loop (now enforced on-chain), the whole stack is open-source-only by *necessity* not ideology, the plugin system was already robotics-shaped (and now has a ROS2 adapter that converts action server output into gate-auditable messages), the Truth Market's operator model had a sharp low-participation risk that is now patched at both layers, operators can now self-report fingerprints for diversity detection, and the Robot Ops UI reads live chain state — honestly showing zero operators rather than masking the bootstrap gap with a simulation.

---

## The robot is the agent

Nothing in JunoClaw's coordination crate or `agent-company` contract special-cases "robot" vs "software agent." The trust stack — J-Lens gate → BFT consensus → Truth Market → Juno settlement — audits *messages*, not a fixed notion of what emitted them. A robot's controller pushing an intent through the pipeline is architecturally indistinguishable from any other fleet member. This was true by construction, not by retrofit.

There's a useful split here between *reflex-tier* and *intent-tier* decisions. Reflex-tier is the sub-100ms loop — sensor fusion, balance, collision avoidance — that lives entirely on the robot's controller and never touches the chain. Intent-tier is the higher-order "what should I do next" decision that *does* get audited: a combat robot choosing to engage a target, a delivery robot choosing a route through disputed terrain, a surgical assistant choosing a tool. The intent is the message; the gate audits the message; the Truth Market settles the outcome.

The reflex/intent split is now real in the code, not just architectural framing. The coordination crate's `MultiOperatorGate` processes arbitrary `AgentMessage` payloads — it doesn't care whether the sender is a Python script or a robot controller. But there's now a typed schema for robot intent-tier decisions: `IntentMessage`, defined in `junoclaw-coordination/src/message.rs`. An `IntentMessage` carries `robot_id`, `action` (e.g. "engage", "navigate"), structured `params`, a `sensor_snapshot_hash` (SHA-256 of the robot's sensor state at decision time), `controller_timestamp`, optional `rationale`, and optional `execution_proof_ref` (rosbag path or action server result ID). It encodes into an `AgentMessage`'s content field via `into_agent_message()` — the gate hashes and audits it, the Truth Market settles the outcome.

What's *not* built yet is the HTTP bridge to a real ROS2 action server — the plugin that converts live ROS2 output into `IntentMessage` payloads (see `plugin-ros2` below). The reflex-tier (sub-100ms sensor fusion, balance, collision avoidance) stays on the robot's controller and never becomes an `IntentMessage`. Only intent-tier decisions that need on-chain audit are wrapped. The Commonware BFT layer (~300ms finality) is fast enough to consume intent-tier decisions within a robot's decision cycle. The robot decides its next move mid-fight in milliseconds; the chain decides whether that move was honest in seconds. Those are different timescales solving different problems, and the architecture now has the typed schema to match.

---

## Reflex vs intent: what the code actually does

The split is concrete. Here's what stays on the controller (reflex-tier) and what becomes a message (intent-tier), in three real-world scenarios.

### Scenario 1: Combat robot — "engage target"

**Reflex-tier (on controller, never touches chain):** The robot's IMU detects a 15° tilt as it crests a rubble pile. The balance controller adjusts motor torque in 8ms. Lidar detects an obstacle at 0.3m — the collision avoidance module triggers a lateral dodge in 12ms. These are sensor-fusion loops running at 100-1000Hz. They never become `IntentMessage`s. They are not auditable decisions; they are physics.

**Intent-tier (becomes a message, audited on-chain):** The robot's target classifier identifies a hostile at 30m bearing 045°. The robot's decision module evaluates: target confirmed, weapon system armed, no civilians in blast radius. It emits an intent to engage. This is a *choice* — not a reflex — and it's the kind of choice that needs an audit trail. The `plugin-ros2` adapter wraps it:

```rust
IntentMessage {
    robot_id: "combat-bot-7",
    action: "engage",
    params: json!({"target_id": "T-0042", "weapon": "primary", "bearing": 45}),
    sensor_snapshot_hash: "a3f2e1...",  // SHA-256 of lidar+thermal+audio at decision time
    controller_timestamp: 1723910500000,
    rationale: Some("Hostile confirmed, no civilians in blast radius"),
    execution_proof_ref: Some("rosbag_2026_08_17_001"),
}
```

This `IntentMessage` is encoded into an `AgentMessage.content` field. The J-Lens gate audits it: was the decision to engage consistent with the robot's declared operating parameters? Did the sensor snapshot support the target classification? The Truth Market settles: multiple operators evaluate the intent, consensus determines whether it was honest, and the outcome is recorded on-chain.

### Scenario 2: Delivery robot — "route through disputed terrain"

**Reflex-tier:** The robot's wheel encoders detect slip on a gravel surface. Traction control adjusts torque distribution in 5ms. A pedestrian steps into the path — emergency brake fires in 20ms. These are safety-critical reflexes. They happen faster than a human can blink, and they should never wait for chain finality.

**Intent-tier:** The robot reaches a junction. Route A is shorter but crosses a sector with recent GPS spoofing reports. Route B is 400m longer but stays in verified territory. The robot's navigation module chooses Route B. This is a *strategic decision* — it's auditable, it's debatable, and if the robot later deviates from the chosen route, that deviation is evidence. The intent becomes a message:

```rust
IntentMessage {
    robot_id: "delivery-bot-3",
    action: "navigate",
    params: json!({"route": "B", "destination": [37.7749, -122.4194], "reason": "gps_spoofing_risk_sector_A"}),
    sensor_snapshot_hash: "b7c4d9...",
    controller_timestamp: 1723910600000,
    rationale: Some("Route A has GPS spoofing risk; Route B is 400m longer but verified"),
    execution_proof_ref: None,
}
```

### Scenario 3: Surgical assistant — "select tool"

**Reflex-tier:** The robot's force sensor detects resistance exceeding 2N during a guided incision. It backs off in 3ms. The camera stabilization gimbal compensates for surgeon hand tremor at 200Hz. These are hardware control loops — they live and die in the controller's real-time stack.

**Intent-tier:** The robot's vision system identifies the next anatomical structure. It recommends switching from scalpel to cauterizer. This is a *treatment decision* — it has medical, legal, and ethical implications. The surgeon approves; the robot executes. But the recommendation itself is the auditable intent:

```rust
IntentMessage {
    robot_id: "surgical-assist-1",
    action: "pick_tool",
    params: json!({"current": "scalpel", "recommended": "cauterizer", "structure": "blood_vessel_L4"}),
    sensor_snapshot_hash: "c9e8a1...",
    controller_timestamp: 1723910700000,
    rationale: Some("Vision system identifies blood vessel; cauterizer reduces bleeding risk"),
    execution_proof_ref: Some("action_result_8842"),
}
```

### The code boundary

The split is enforced by *what becomes a message*. The `IntentMessage` struct has fields for `action`, `params`, `sensor_snapshot_hash`, and `rationale` — these are the auditable components of a decision. The reflex-tier has no struct, no message, no on-chain representation. It's the absence of a message. The robot's controller runs its real-time loops; when it reaches a decision that needs an audit trail, `plugin-ros2`'s `emit_intent` action wraps it:

```rust
// In plugin-ros2's execute() — the conversion point
let intent = self.build_intent_message(
    "engage",                          // action
    json!({"target_id": "T-0042"}),    // params
    &sensor_snapshot,                   // raw sensor bytes
    controller_timestamp,               // robot's clock
    Some("Hostile confirmed".into()),   // rationale
    Some("rosbag_001".into()),          // proof ref
);
// Wraps into AgentMessage — ready for the gate
let agent_msg = self.wrap_intent_into_agent_message(intent, from, timestamp)?;
```

The gate doesn't know it's a robot. The Truth Market doesn't know it's a robot. The settlement contract doesn't know it's a robot. They see an `AgentMessage` with a content hash and an encoded payload. The `IntentMessage` schema is for the *producer* (the robot plugin) and the *consumer* (the relayer or auditor who decodes the content to evaluate the intent). The trust stack in between is hardware-agnostic by design.

### Is anyone else doing this?

No. The closest analogues are:

- **Ocean Protocol** — data marketplace with on-chain settlement, but no robotics, no BFT gate, no intent schema.
- **Fetch.ai** — agent economy framework, but no typed intent/reflex split, no Truth Market, no on-chain verdict settlement.
- **Chainlink** — decentralized oracle consensus, but no robotics, no agent-fleet model, no intent-tier auditing.
- **ROS2 + rosbag** — industry-standard logging and replay, but no cryptographic anchoring to a trust gate, no economic settlement on decision honesty.

What doesn't exist anywhere else is the **junction**: robot intent → typed schema → BFT gate → Truth Market settlement → on-chain economics. That chain is in this codebase, deployed on uni-7, today. The reflex/intent split isn't a whitepaper framing — it's a `struct` with fields and a conversion function. The plugin isn't a spec — it's a crate that compiles and returns encoded `AgentMessage`s. The Truth Market isn't a design doc — it's a CosmWasm contract at `juno1rsf3...` with a `min_operators` floor and fingerprint diversity detection.

First to ship a trust OS where a robot is just another fleet member emitting intents into the same gate as software agents. Not first to imagine it — first to deploy it.

---

## The reflex-tier gap: you can't gate physics, but you can prove it was safe

The intent-tier asks "was this decision honest?" in real time. The reflex-tier can't ask that — it runs at 100-1000Hz, and a ~300ms BFT consensus cycle would arrive after the robot has already crashed, fallen, or collided. The split is a physics constraint, not a design choice.

But this leaves a gap: if everything below 100ms is unauditable, how do you trust the robot at all? A malicious operator could classify everything as "reflex" and never emit an auditable message. A safety violation in the reflex layer — a collision, a force overload, a tilt beyond bounds — would have no on-chain trace.

The answer is **not** real-time gating (impossible) but **post-hoc attestation with enforcement teeth**. Three layers, all now in the codebase:

### Layer 1: SafetyEnvelope — governance-controlled operating parameters

```rust
SafetyEnvelope {
    robot_id: "surgical-bot-1",
    max_speed: 0.5,           // m/s
    max_force: 10.0,          // Newtons
    min_collision_distance: 0.05,  // meters
    max_tilt_degrees: 15.0,
    max_acceleration: 2.0,    // m/s²
    human_proximity_allowed: true,
    version: 1,               // incremented on each governance update
}
```

The robot's safety parameters are stored on-chain. The controller reads them at startup and enforces them in its reflex loops. Changing them requires a transaction — not a YAML edit on the robot. In ROS2, params are a config file that any operator can modify. In JunoClaw, they're governance-controlled. Nobody else does this.

Tightening the envelope (lowering max speed, increasing collision distance) is a governance vote. Loosening it is too. The robot can't silently operate outside its declared bounds — the envelope version is embedded in every `ReflexBatchAttestation`, so any mismatch between what the robot claimed to enforce and what governance declared is detectable.

### Layer 2: ReflexBatchAttestation — post-hoc proof of safety

```rust
ReflexBatchAttestation {
    robot_id: "combat-bot-7",
    merkle_root: "a3f2e1d4b5c6...",  // Merkle root of 1000 reflex cycle hashes
    cycle_count: 1000,
    batch_start_timestamp: 1723910400000,
    batch_end_timestamp: 1723910401000,  // 1 second of reflex cycles
    envelope_version: 1,
    all_invariants_maintained: true,
    violated_invariants: [],            // empty = all safe
    rosbag_ref: "rosbag_2026_08_17_001",
}
```

Every N cycles (configurable — could be every 1000 cycles, every 1 second, every 10 seconds), the controller hashes a batch of reflex cycles into a Merkle root and submits it as an `AgentMessage`. Each cycle's hash includes sensor readings and safety invariant check results (pass/fail per invariant). The Merkle root is anchored on-chain.

This doesn't gate reflexes in real time. It proves *after the fact* that the safety envelope was maintained — or provides evidence that it was violated. If an incident occurs, the full rosbag can be compared against the anchored Merkle root. You can't deny what happened. The attestation is either consistent with the rosbag or it isn't, and that inconsistency is itself evidence.

The key insight: the reflex-tier doesn't need on-chain **gating** (physically impossible). It needs on-chain **attestation** (post-hoc verifiability). Different problem, different solution.

### Layer 3: CircuitBreaker — enforcement with teeth

```rust
CircuitBreakerState::Tripped {
    reason: "min_collision_distance violated in batch 42",
    tripped_at: 1723910700000,
    cause_ref: "rosbag_2026_08_17_002",
}
```

Attestation without enforcement is just logging. The circuit breaker is what makes it have consequences. When a `ReflexBatchAttestation` reveals a safety violation (or the Truth Market flags one), the breaker trips. `plugin-ros2`'s `emit_intent` action checks the breaker *before* building an `IntentMessage`:

```rust
// In plugin-ros2's execute() — before emit_intent
if self.circuit_breaker.is_tripped() {
    return Err(JunoClawError::Plugin {
        plugin: "plugin-ros2",
        message: "circuit breaker tripped — intent-tier locked, \
                  reflexes still running. Resolve safety violation \
                  and reset breaker before emitting new intents.",
    });
}
```

When tripped, the robot enters **safe-hold**: reflexes keep running (physics doesn't stop — the robot doesn't crash, it doesn't fall over, it doesn't stop avoiding obstacles), but intent-tier decisions are locked. The robot can't make new high-level choices until the violation is resolved and the breaker is reset by governance or the operator.

This is the enforcement layer that makes the whole reflex-tier stack have teeth. Without it, a robot could violate its safety envelope, log the violation, and continue making auditable decisions as if nothing happened. With it, a safety violation **grounds** the robot's intent-tier — it can still survive, but it can't *act*.

### The five-layer trust stack for robotics

```
SafetyEnvelope (on-chain params, governance-controlled)
    ↓
Reflex loop (controller, sub-100ms, reads params at startup)
    ↓
ReflexBatchAttestation (Merkle root, submitted every N cycles)
    ↓
CircuitBreaker (trips if attestation reveals violation → intent-tier locked)
    ↓
IntentMessage (only emitted if circuit breaker is closed)
    ↓
BFT gate → Truth Market → settlement
```

Nobody has this. The closest analogues are ROS2's safety-certified controllers (pilz/industrial robots), which enforce safety bounds locally but have no on-chain attestation or governance control. And ISO 10218 / TS 15066 safety standards, which define safe operating envelopes but have no cryptographic proof of compliance. This stack takes the safety envelope concept from industrial robotics and makes it tamper-proof, auditable, and enforceable through on-chain governance.

---

## Skill-registry and marketplace: the catalog and the register

They are not the same contract, and that's correct, not an oversight. `skill-registry` is the discovery/manual layer — "what does this dApp do and where do I find its operating manual." `marketplace` is the economic listing/escrow/hire layer — Truth-Market-gated escrow that only releases funds on a green verdict. `marketplace`'s `skill_ref` conventionally points at a `skill-registry` `dapp_name`.

**Before:** `skill_ref` was a free-text field. A listing could claim to provide any skill without the skill existing in the registry. The two contracts were conventionally linked but not cryptographically linked — an agent discovering a marketplace listing had no on-chain guarantee that the referenced skill was real, registered, or even non-fabricated.

**After (shipped August 2026):** `marketplace`'s listing function now queries the configured `skill_registry` contract via `query_wasm_smart` before accepting a listing. If `skill_registry` is set (it's optional — `None` means the cross-check is disabled, preserving backward compatibility) and the `skill_ref` is not found in the registry, the listing is rejected with `ContractError::SkillNotRegistered`. The check is opt-in at instantiate time and can be toggled by the admin. On uni-7, the marketplace was migrated with the cross-check compiled in (currently `skill_registry: null` — the toggle is wired but not yet pointed at the skill-registry contract, which is the conservative path for existing listings). Two new tests pin the boundary: unregistered `skill_ref` rejects, registered `skill_ref` accepts.

MCP tooling already exposes skill-registry queries — any MCP-capable agent gets wire-level discovery for free. With the cross-check in place, a marketplace listing is now a *cryptographic claim* that the skill exists in the registry, not just a string field.

---

## Open-source, all the way up — because it has to be

J-Lens cannot run against a closed model API. Hosted completion endpoints never expose `hidden_states`; a linear probe on the residual stream needs actual mid-forward-pass activations. Closed inference = no J-Lens, full stop — this is written into the DAO record (A18c-9). That single technical constraint is why the entire stack is forced open-weight at the model layer. It cascades: open models → open probes → open plugin adapters (`plugin-peaq`, `plugin-rsynth`, and the generic `Plugin` trait) → open marketplace listings referencing open skills. There is no closed-source escape hatch anywhere in the trust path.

---

## The plugin system was already robotics-shaped

`plugin-peaq` and `plugin-rsynth` are not robotics plugins, but they prove the pattern: a generic `Plugin` trait, optional by design ("JunoClaw works standalone without X"), bridging an external verifiable-something into JunoClaw's trust core. `plugin-ros2` now exists — same shape as peaq and rsynth, config-gated, with a real `emit_intent` action that builds an `IntentMessage` from ROS2 action server output and wraps it into an `AgentMessage` for gate submission. The conversion is explicit: `build_intent_message()` takes the action name, parameters, sensor snapshot, controller timestamp, and optional rationale/proof reference, hashes the sensor snapshot, and produces a typed `IntentMessage`. `wrap_intent_into_agent_message()` encodes it as JSON in the `AgentMessage.content` field. The plugin returns the encoded `AgentMessage` — ready for the J-Lens gate to hash, audit, and settle.

Two stub actions remain: `fetch_intent_proof` (HTTP GET from a ROS2 bridge endpoint to parse action server results into `IntentMessage` payloads) and `fetch_sensor_log` (extract intent-tier decisions from a rosbag archive). Both return wiring-point descriptions — the HTTP client needs a concrete ROS2 partner's bridge endpoint.

### HTTP bridge: three paths to a real ROS2 integration

The missing piece is the HTTP bridge between a real ROS2 stack and `plugin-ros2`. Three paths, each serving a different stage:

**Production: rosbridge_suite (rosbridge_server)** — The standard ROS2 bridge, ships with most ROS2 distributions. Exposes topics, services, and actions over WebSocket JSON. `plugin-ros2` connects via WebSocket, subscribes to action server result topics, and converts results into `IntentMessage` payloads in real time. This is the production path: if a robotics partner already runs rosbridge (and most ROS2 deployments do), there's zero additional infrastructure to deploy. The plugin's `fetch_intent_proof` action opens a WebSocket to `ws://robot-local:9090`, subscribes to the action server's result topic, and each result becomes an `IntentMessage` — `robot_id` from the topic namespace, `action` from the action type, `params` from the goal/result payload, `sensor_snapshot_hash` from the sensor topics synced at the same timestamp. The bridge is the ROS2 deployment itself; `plugin-ros2` is just a WebSocket client with a schema mapper.

**Long-term: rclgo embedded in plugin-ros2** — Rust-native ROS2 client (`rclgo`) with DDS subscription baked directly into `plugin-ros2`. No separate bridge process, no WebSocket hop, no JSON serialization layer — the plugin subscribes to ROS2 topics using the same DDS transport that the robot's nodes use. This is the architecturally clean path: `plugin-ros2` becomes a first-class ROS2 node, not a client of a bridge. `rclgo` is less mature than `rclpy`, but the dependency is worth it — one fewer process, one fewer serialization boundary, one fewer failure mode. When `rclgo` reaches production readiness, `plugin-ros2` absorbs the bridge and the WebSocket path becomes a fallback.

**Local testing: FastAPI microservice (Python)** — ~100 lines of Python using `rclpy` to subscribe to action servers, exposing REST endpoints (`GET /intent/{id}`, `GET /rosbag/{batch_id}`). Not for production — it's a testing shim. A developer who wants to verify the `IntentMessage` schema against a real ROS2 action server without setting up rosbridge or rclgo can run this in an afternoon. It proves the conversion path: action server result → JSON → `IntentMessage` → `AgentMessage`. Once the path is verified, swap the shim for rosbridge (production) or rclgo (long-term).

**Integration timeline**: the code is ready — `IntentMessage` schema, `plugin-ros2` `emit_intent` action, `build_intent_message()` conversion, `wrap_intent_into_agent_message()` wiring. What's missing is a robotics partner running ROS2 who wants to bolt on verifiable trust + economics. That's a partnership question, not an engineering question. The bridge is the last mile; the architecture is the first 99.

The `PluginCapability` enum gained a `RoboticsControl` variant. *This is the growth thesis*: any autonomous-robotics builder already running an open-source stack can bolt on a verifiable trust + economics layer without JunoClaw needing to know anything about their hardware. The `emit_intent` action proves the conversion path; the HTTP bridge is the last mile.

---

## Why Truth Market verifiers must be independent — and what happens if they aren't

If every operator runs the same model on the same hardware with the same probe calibration, a systematic bias makes them all diverge together, and majority-vote consensus confidently agrees on the wrong answer. Slashing doesn't help when everyone is wrong the same way. Independence isn't a nice property, it's the only thing that makes the slashing mechanism mean anything.

**Before:** neither the multi-operator gate nor `truth-market`'s epoch finalization enforced a minimum operator count. With one configured operator, the consensus ratio was trivially 1/1 = 100%. That operator could never diverge from "consensus" — because they *were* the consensus — and would claim the full reward pool every epoch with zero adversarial check on their own claim. Zero operators safely halted finalization (`NoVerdicts`); one operator was worse than zero, because it looked like the system was working while providing none of the security the design assumed.

**After (shipped August 2026):** both layers now carry an explicit floor. `MultiOperatorConfig` gained a `min_operators` field (default 3); the gate short-circuits to a `Red` verdict with zero attestations — not a computed ratio — whenever fewer operators are wired than the floor requires. On the settlement side, `truth-market`'s `Config` gained the matching `min_operators` (instantiate-time, admin-adjustable via `UpdateConfig`, exposed on `GetConfig`), and epoch finalization now rejects with `ContractError::InsufficientOperators { required, submitted }` before it ever computes a consensus ratio, mirroring the existing `NoVerdicts` guard. A `migrate` entry point was added so the live uni-7 deployment could be upgraded in place — old `Config` state (which predates the field) is read under its previous shape and re-saved with `min_operators` defaulting to 3, rather than requiring a fresh instantiate. Three new test cases pin the boundary: below-floor rejects, at-floor (with a configurable floor of 1) succeeds, and `UpdateConfig` can move the floor at runtime.

What this does *not* solve, and was scoped out deliberately: there is still no on-chain way to *prove* that three configured operators are three *independent* operators rather than one entity running three processes. The floor stops the trivial 1-operator self-win; it does not stop a sufficiently motivated single actor from standing up `min_operators` worth of identical, correlated instances.

**What is shipped now:** operators can self-report a `fingerprint` (a free-text string, conventionally a model+host hash) at registration time. The contract stores this on the `Operator` struct and maintains a `FINGERPRINT_COUNTS` map — a new `GetFingerprints` query returns all fingerprints sorted by operator count descending, plus a count of operators who didn't declare one. This is a *soft signal*, not an enforcement: the contract doesn't reject duplicate fingerprints, because nothing in a CosmWasm execute message can attest to what's actually running behind an address. A relayer or dashboard can query `GetFingerprints` and flag: "3 operators, all with the same fingerprint — this looks correlated." That's the detection layer. The enforcement layer — if any — would be a relayer refusing to finalize epochs where the fingerprint diversity is below a threshold, or a DAO-level slash for declared-but-false fingerprints. Both are off-chain policy, not contract logic.

**Can this be proven deterministically on-chain?** No — not with today's CosmWasm. A contract sees an address and a signed message. It cannot inspect the process behind the address, verify the model weights, or measure hardware diversity. A TEE attestation (SGX/SEV) could improve this: an operator running inside a TEE could attest to its model hash and hardware, and the attestation could be verified on-chain. That's a future WAVS integration, not today's contract. The honest answer is: the fingerprint makes correlation *visible*; it doesn't make it *impossible*. That remains the real "more work demand" the architecture creates: bootstrapping genuine, diverse operator supply is a harder problem than writing the settlement contract, and it's still unsolved — just no longer silently broken.

---

## The Robot Ops UI reads live chain state

The frontend now polls the live truth-market contract on uni-7 every 10 seconds via a `useTruthMarketLive` React hook. The query layer fetches the operator list, epoch stats, and marketplace listings directly from the chain. The Trust Constellation visualization shows a **LIVE** badge when real data is flowing and a **SIM** badge when it falls back to local simulation.

The honest part: right now, the live query returns zero operators. There is no simulation masking that. The UI shows an empty constellation with a `0 operators` count and a bootstrap prompt. This is deliberate — the alternative (falling back to simulated operators when the chain says zero) would be the exact failure mode described above: looking like the system is working when it isn't. The Robot Ops panel is the first place where the operator bootstrap gap becomes visible to a human operator rather than buried in a contract query.

---

## Closing: what the code already knew

The thesis of this piece is that the architecture was right before the narrative caught up. Seven claims, each verified against the codebase:

1. **Robots are agents.** No special-casing in the coordination crate or agent-company contract. A robot's controller pushing an intent through the gate is architecturally identical to a software agent doing the same. The reflex-tier / intent-tier split is now real: `IntentMessage` is the typed schema that wraps robot intent-tier decisions for gate audit. Reflex-tier stays on the controller; intent-tier becomes a message.
2. **Discovery and economics are separate but linked.** Skill-registry and marketplace are different contracts with different jobs, now cryptographically linked via the on-chain cross-check.
3. **Open-source is a constraint, not a preference.** J-Lens requires `hidden_states`; closed APIs don't expose them. The entire trust path is forced open-weight at the model layer.
4. **The plugin system was already robotics-shaped.** `plugin-peaq`, `plugin-rsynth`, and now `plugin-ros2` prove the pattern. The ROS2 adapter has a real `emit_intent` action that builds `IntentMessage` → wraps into `AgentMessage` → returns it for gate submission. The HTTP bridge to a live ROS2 action server needs a concrete robotics partner.
5. **The Truth Market had a low-participation vulnerability.** It's now patched at both the gate and settlement layers with an explicit `min_operators` floor, migrated in place on uni-7 (code_id 95, August 2026).
6. **Operator diversity is now visible, not enforced.** Operators self-report a fingerprint at registration; a new `GetFingerprints` query exposes correlation counts. Detection, not prevention — but you can't fix what you can't see.
7. **The UI tells the truth about chain state.** Zero operators shows as zero operators. The bootstrap gap is visible, not hidden.

What's next is harder than what's done: bootstrapping genuine, diverse operator supply for the Truth Market; pointing the marketplace `skill_registry` field at the live skill-registry contract; building the HTTP bridge in `plugin-ros2` that fetches from a real ROS2 action server; exploring TEE attestation as a path from fingerprint visibility to fingerprint verification. None of these are blocked by the architecture. They're blocked by the same thing that blocks every marketplace: supply.

The code was ready. The robots were always agents. The trust stack was always hardware-agnostic. The marketplace was always the economic layer on top of the discovery layer. The operator model was always the bottleneck. The only thing that changed between the first commit and this article is that we stopped calling it future work and started calling it deployed.

---

## Midjourney prompts for article images

Copy-paste these into Midjourney (or any image generator). Old-style engraving/lithograph aesthetic — matches the "architecture was right before the narrative caught up" thesis.

**Hero image — robot as agent:**

```
/an old-style copper engraving of a humanoid robot sitting at a ledger desk, quill in hand, writing entries into a great book of accounts, candlelight illuminating brass gears and mechanical joints, the robot wears a merchant's coat, on the desk a small globe and a compass, in the style of 18th century scientific illustration, sepia tones, fine cross-hatching, detailed mechanical anatomy visible through torn coat sleeve --ar 16:9 --style raw --v 6
```

**Surgical robot — intent-tier decision:**

```
/an old-style lithograph of a surgical robot in an operating theater, brass and copper mechanical arm holding a scalpel with surgical precision, surrounded by doctors in 19th century medical coats watching, the robot pauses mid-decision, a glowing ledger book floats beside it showing checkmarks and crossmarks, candlelit stone chamber, in the style of Victorian medical illustration, muted browns and creams, fine line work --ar 16:9 --style raw --v 6
```

**Combat robot — reflex vs intent:**

```
/an old-style engraving of a combat robot on a battlefield, mechanical knight with brass plating and exposed gears, one hand raised in a gesture of decision while the other grips a weapon, a split scene — left side shows fast blurred motion lines representing reflexes, right side shows a glowing parchment with sealed wax stamp representing audited intent, smoke and distant cannons, in the style of medieval mechanical illustration meets Da Vinci sketchbook, sepia and charcoal --ar 16:9 --style raw --v 6
```

**Delivery robot — route through disputed terrain:**

```
/an old-style etching of an autonomous delivery robot at a crossroads, mechanical pack-mule with brass legs and a cargo box, two paths diverge in a rugged landscape, one path marked with warning signs and broken milestones, the other longer but clear, a small mechanical compass on the robot's chest pulses with light, a ledger page nailed to a nearby tree shows a route decision stamped and sealed, in the style of 18th century cartography illustration, warm browns and deep greens --ar 16:9 --style raw --v 6
```

**Truth Market — operator consensus:**

```
/an old-style engraving of a circular hall where seven hooded figures sit at individual desks each examining the same document under candlelight, each desk has a brass mechanical device with a green or red indicator, the figures are diverse — different heights, different tools, different spectacles, a central podium shows a balance scale tipping toward consensus, stone arches and heavy oak doors, in the style of Dutch Golden Age interior painting meets architectural engraving, deep shadows and warm candlelight --ar 16:9 --style raw --v 6
```

**The gate — messages flowing:**

```
/an old-style engraving of a great stone gatehouse in a city wall, mechanical pigeons carrying tiny scrolls fly in from all directions, each scroll bears a wax seal, inside the gatehouse robed clerks stamp each scroll with green or red ink before it passes through, a massive ledger book on a central table records every verdict, the gate opens onto a marketplace where coins exchange hands, in the style of 16th century cityscape engraving, fine architectural detail, sepia tones with touches of gold leaf --ar 16:9 --style raw --v 6
```

**Plugin system — the bridge:**

```
/an old-style engraving of a workshop where a craftsman connects a brass mechanical adapter between a loom and a printing press, the adapter has interchangeable fittings, one side accepts textile threads the other side outputs printed pages, the craftsman wears an apron and holds a caliper, shelves behind show similar adapters of different shapes all following the same mounting bracket, in the style of Da Vinci's workshop drawings, sepia and rust tones, detailed mechanical cross-sections --ar 16:9 --style raw --v 6
```

**Robot Ops UI — honest zero:**

```
/an old-style engraving of an observatory at night, an astronomer sits at a large brass telescope connected to a mechanical display board, the board shows empty constellation circles with a small engraved "0" at the center, no stars are plotted, the astronomer looks through the telescope and sees clear sky, an open ledger on the desk reads "zero observed — zero recorded," candlelight and starlight, in the style of 18th century astronomical illustration, deep blues and warm candle gold --ar 16:9 --style raw --v 6
```
