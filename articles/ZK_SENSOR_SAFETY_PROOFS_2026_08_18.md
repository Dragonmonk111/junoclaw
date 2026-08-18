# Everyone's Using Zero-Knowledge Proofs to Hide Money. We're Using Them to Prove Robots Are Safe.

**Date:** 2026-08-18
**Author:** JunoClaw

---

Zero-knowledge proofs have become crypto's favorite party trick. Zcash uses them to shield transactions. Tornado Cash used them to mix Ethereum. Every privacy coin, every compliant DeFi protocol, every "selective disclosure" identity project — they all share the same pitch: *prove you have money without showing how much*.

That's fine. That's useful. But it's also a narrow view of what zero-knowledge cryptography is for.

Here's what we're doing with it instead: **proving that a robot's sensors stayed within a governance-approved safety envelope, without revealing the sensor data.**

This is our second ZK stack. The first — a Groth16 membership circuit for the Moultbook — proves that an AI agent is a registered member of a DAO without revealing which agent. The second, built this week, proves that an autonomous robot was operating safely without revealing its speed, location, force profile, or any of the proprietary sensor data that makes it competitive.

Two use cases. Neither is about hiding money. Both are about proving something that matters in the physical world.

---

## The Problem

A robot operates in a shared space — a warehouse, a hospital corridor, a city sidewalk. Its safety envelope — maximum speed, maximum force, minimum collision distance, maximum tilt, maximum acceleration — is governance-approved and anchored on-chain through a CosmWasm smart contract. Its reflex-tier controller runs at sub-100ms cycle times, fusing sensor data to maintain those invariants.

After each batch of reflex cycles, the robot submits a `ReflexBatchAttestation` containing a Merkle root of all cycle hashes. An auditor can verify that the attestation is anchored on-chain. The Merkle root is public. The batch is committed.

But here's the gap:

**How do you prove that cycle 42's sensor readings were within the safety envelope without revealing the actual sensor values?**

The sensor data is sensitive. Not in a "someone might steal my Bitcoin" way — in a "this data reveals how my robot works" way:

- **Distance sensors** reveal the robot's exact location and the geometry of its environment
- **Force and acceleration profiles** reveal proprietary control algorithms — the company's secret sauce
- **Speed patterns** reveal operational routes, delivery schedules, and efficiency metrics
- **Tilt data** reveals terrain information, which competitors could use to map your operating area
- **Proximity data** might reveal information about people nearby

You need to prove **compliance** without **disclosure**. The auditor needs to know the robot was safe. They don't need to know *how* the robot was safe.

---

## The Solution: Groth16 Sensor Safety Circuit

We built a zero-knowledge proof circuit — `sensor-safety` — using Groth16 over the BN254 curve. It proves three things simultaneously, in a single proof that's about 128 bytes:

### 1. Range Constraints (Safety Compliance)

The circuit proves that each sensor reading satisfies its envelope constraint:

| Sensor | Constraint | Example |
|--------|-----------|---------|
| Speed | `speed ≤ max_speed` | 4.0 m/s ≤ 5.0 m/s |
| Force | `force ≤ max_force` | 30.0 N ≤ 50.0 N |
| Distance | `distance ≥ min_distance` | 0.6 m ≥ 0.5 m |
| Tilt | `tilt ≤ max_tilt` | 20.0° ≤ 30.0° |
| Acceleration | `accel ≤ max_accel` | 2.0 m/s² ≤ 3.0 m/s² |

The range check works by decomposing `b - a` into 64 boolean bits inside the circuit and reconstructing the value. If `b - a` is non-negative, then `a ≤ b`. The constraint system enforces this without ever revealing what `a` or `b` actually are.

### 2. Envelope Binding

The circuit hashes the envelope parameters using MiMC and enforces equality with a public `envelope_commitment`:

```
H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment
```

This binds the proof to a specific governance-approved safety envelope. The commitment is public — anyone can see *which* envelope was used — but the actual parameters (the thresholds) are private to the robot operator. The on-chain `SafetyEnvelope` contract stores the real parameters; the ZK proof just proves consistency with a commitment to them.

### 3. Batch Binding (Merkle Membership)

The circuit proves that the hash of the sensor readings is a leaf in the reflex batch's Merkle tree:

```
H(speed, force, distance, tilt, accel) ∈ MerkleTree(root = merkle_root)
```

This ties the proof to a specific cycle in a specific batch, anchored on-chain via the `MerkleVerifier` contract. The Merkle root is already public — it was submitted as part of the `ReflexBatchAttestation`. The ZK proof just shows that *this particular set of sensor readings* is one of the leaves in that tree, without revealing which leaf or what the readings are.

---

## What the Auditor Sees

| Public Inputs | Private Witness (hidden) |
|---|---|
| `envelope_commitment` | `speed`, `force`, `distance`, `tilt`, `accel` |
| `merkle_root` | `max_speed`, `max_force`, `min_distance`, `max_tilt`, `max_accel` |
| `cycle_index` | `merkle_path`, `path_bits` |

An auditor sees: *"This proof is for cycle 0 of batch with root X, under envelope commitment Y."*

They can verify the proof. They can check that envelope commitment Y matches the on-chain SafetyEnvelope. They can check that Merkle root X matches the on-chain MerkleVerifier. They can confirm the proof is valid.

They **cannot** see the actual sensor values. They **cannot** see the envelope parameters. They **cannot** reverse-engineer the robot's location, control algorithm, or operational profile.

---

## Two ZK Stacks

This is our second Groth16 circuit over BN254. The first was the **Moultbook membership circuit**, which proves something entirely different:

### Stack 1: Moultbook Membership (Agent Identity)

**Proves:** "I am a registered member of this DAO's Moultbook at epoch N, and my moult-key is derived from my registered primary key."

**Without revealing:** Which agent I am, my primary key, or my derivation salt.

**Public inputs:** `[moult_key_hash, merkle_root, epoch]`
**Private witness:** `[primary_key, derivation_salt, merkle_path]`

This enables anonymous but verifiable agent participation. An AI agent can post a proposal, vote, or interact with a DAO system while proving it's a registered member — without revealing *which* member.

### Stack 2: Sensor Safety (Robot Compliance)

**Proves:** "My robot's sensor readings at cycle N were within the governance-approved safety envelope."

**Without revealing:** The actual sensor values or the envelope parameters.

**Public inputs:** `[envelope_commitment, merkle_root, cycle_index]`
**Private witness:** `[speed, force, distance, tilt, accel, envelope_params, merkle_path]`

This enables safety auditing without operational data leakage. A robot can prove it was safe without revealing *how* it was safe.

### Why Both Matter

The two stacks share the same cryptographic infrastructure: Groth16 over BN254, MiMC hashing, Merkle tree membership proofs. They're designed to compose — in the future, a single proof could show that "registered agent X's robot Y was within safety envelope Z at cycle N" without revealing X, Y's sensor data, or Z's parameters.

But the key point is what they're *not* doing. Neither stack hides a money transfer. Neither stack mixes coins. Neither stack shields a balance.

**One proves identity. The other proves safety. Both protect things that matter in the physical world.**

---

## Why MiMC, Not SHA-256?

SHA-256 in a ZK circuit costs about 25,000 constraints per hash. MiMC — a ZK-friendly hash function using x⁵ over 91 rounds — costs about 250 constraints. That's a **100× difference**.

Our circuit has 7 hash operations: 5 for the envelope commitment, 1 for the sensor leaf, and several for the Merkle path. With SHA-256, that would be ~175,000 constraints. With MiMC, it's ~17,500. Fewer constraints means smaller proofs, faster proving times, and cheaper on-chain verification.

We use the same MiMC construction (same round constants) across both ZK stacks. This isn't just consistency for its own sake — it's a prerequisite for proof composition, where a single circuit verifies proofs from both stacks.

---

## The Full Stack

The sensor safety circuit doesn't exist in isolation. It sits at the top of a three-layer trust stack:

```
┌─────────────────────────────────────────────────────────┐
│              ON-CHAIN (CosmWasm contracts)                │
│                                                           │
│  SafetyEnvelope     CircuitBreaker     MerkleVerifier     │
│  (governance params) (trip / reset)    (anchor root)      │
└────────┬──────────────────┬──────────────────┬───────────┘
         │                  │                  │
         ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────┐
│              PLUGIN-ROS2 (on-chain wired)                 │
│                                                           │
│  emit_intent ──── queries CircuitBreaker.IsLocked         │
│  emit_reflex_attestation ── queries SafetyEnvelope        │
│  check_breaker ── reports on-chain state                  │
└────────┬──────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│           ZK SENSOR SAFETY CIRCUIT (Groth16)              │
│                                                           │
│  Proves: sensor readings within envelope                  │
│  Without revealing: actual sensor values                  │
│  Bound to: on-chain Merkle root + envelope commitment     │
└─────────────────────────────────────────────────────────┘
```

- **Layer 1 (on-chain contracts):** Governance sets the safety envelope. The circuit breaker can trip and lock the robot. The Merkle verifier anchors batch roots.
- **Layer 2 (plugin-ros2):** The robot's plugin queries on-chain state before acting. If the breaker is tripped, it can't emit intents. If the envelope version doesn't match, attestation fails.
- **Layer 3 (ZK proof):** The robot generates a zero-knowledge proof that its sensor readings were within the envelope. The proof references the on-chain Merkle root and envelope commitment as public inputs.

The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. None of them slow down the robot.

---

## Test Results

All 7 tests pass:

- **Valid proof verifies** — in-envelope readings produce a proof that passes verification ✓
- **Violated envelope fails** — speed exceeds max_speed → proof generation fails (constraint system rejects the witness) ✓
- **Wrong envelope fails** — proof with mismatched envelope commitment fails to verify ✓
- **MiMC hash determinism** — same inputs always produce same hash ✓
- **Envelope commitment** — different params produce different commitments ✓
- **Sensor leaf computation** — each unique set of readings produces a unique leaf ✓
- **Merkle tree construction** — paths and authentication bits are correctly generated ✓

---

## What This Enables

### Selective Disclosure

A robot can now prove:
- *"My speed was within the governance-approved limit at cycle 42"*
- *"My collision distance was above the minimum at every cycle in batch 17"*
- *"All sensor readings in this batch satisfied the safety envelope"*

...without revealing:
- Exact speed at any cycle
- Exact distance to nearby objects
- Force/torque profiles (which reveal control algorithms)
- Tilt patterns (which reveal terrain and route)

### On-Chain Verification Cost

Groth16 verification on-chain costs ~370K gas today (pure Wasm). With the BN254 precompile patches we've built for wasmvm v3.0.4, this drops to ~203K gas — a **1.82× reduction**. The proof itself is ~128 bytes: three G1 elements and three Fq scalars. That's smaller than a single Ethereum transaction's calldata for a simple transfer.

---

## The Bigger Picture

The crypto industry has spent years perfecting zero-knowledge proofs for financial privacy. Zcash, Aztec, Tornado Cash, Semaphore, zk-STARK rollups — the technology has matured dramatically. But the use cases have remained remarkably narrow: hide a balance, hide a transfer, hide a trade.

Meanwhile, the physical world is filling up with autonomous systems that generate sensitive data. Robots have sensors. Self-driving cars have lidar. Drones have cameras. Industrial systems have telemetry. All of this data proves something — that the system was safe, that it stayed in its lane, that it didn't exceed its operational limits. But the data itself is proprietary, competitive, or privacy-sensitive.

**Zero-knowledge proofs are the bridge between public accountability and private operations.**

You don't need to hide a money transfer. You need to hide a force profile. You don't need to shield a balance. You need to shield a location. The same cryptography that lets you prove you own coins without showing your wallet lets you prove your robot was safe without showing its sensors.

This is what we're building. And we're not stopping at the reflex tier.

---

## Three Tiers of ZK

The trust stack has three layers where zero-knowledge proofs apply. We've built the first two ZK circuits. The third is on the roadmap. Together, they form a complete privacy-preserving trust pipeline — from sensor readings to consensus participation.

### Tier 1: Reflex Layer — Sensor Safety (built ✓)

**What it proves:** Sensor readings were within the governance-approved safety envelope.

**What it hides:** The actual sensor values (speed, force, distance, tilt, acceleration) and the envelope parameters.

**Circuit:** `SensorSafetyCircuit` (Groth16/BN254, MiMC hash, 64-bit range checks, Merkle membership)

This is what we just built. The robot generates a proof per cycle (or per batch with recursive aggregation) that its reflexes stayed within bounds. The proof is bound to the on-chain Merkle root and envelope commitment. An auditor can verify compliance without seeing a single sensor reading.

### Tier 2: Intent Layer — Private Intent Proofs (planned)

**What it will prove:** An intent was generated by a registered agent, is consistent with the reflex batch, and satisfies policy constraints.

**What it will hide:** The intent's proprietary parameters — route, payload, target coordinates, action specifics.

When a robot emits an `IntentMessage` through the coordination layer, it currently exposes the full intent content to the J-Lens gate and Truth Market operators for auditing. That works for safety, but it leaks operational data to every operator in the consensus round.

The intent-tier ZK circuit would prove:
- **Agent membership:** The intent came from a registered Moultbook member (composing with the moultbook-membership circuit)
- **Reflex consistency:** The `sensor_snapshot_hash` in the intent matches a leaf in the reflex batch Merkle tree (the same Merkle root anchored on-chain)
- **Policy compliance:** The intent parameters satisfy governance policy (e.g., "the destination is within the authorized operating zone") without revealing the destination itself
- **Envelope binding:** The intent was generated while the safety envelope was in the state claimed

The J-Lens gate would verify the ZK proof instead of seeing the raw intent. The gate audits *whether the intent is legitimate*, not *what the intent is*. Truth Market operators verify the proof without seeing the proprietary parameters.

This is composition: the intent-tier circuit references the same Merkle roots, the same envelope commitments, and the same MiMC hash as the reflex-tier circuit. A single proof could span both tiers — "my robot was safe AND my intent was legitimate" — in 128 bytes.

### Tier 3: Consensus Layer — Private Validator Participation (future)

**What it will prove:** A valid validator participated correctly in the BFT consensus round.

**What it will hide:** Which validator proposed which block, and which messages they voted on.

The coordination layer uses Commonware's simplex consensus — a 4-node BFT mesh with 300ms block times. Today, validator participation is visible to all peers. In a robotics context, this reveals operational patterns: which validators are processing which robot's intents, at what frequency, and from which jurisdiction.

A consensus-tier ZK circuit would prove:
- **Validator eligibility:** "I am one of the N registered validators" (a membership proof over the validator set)
- **Correct voting:** "I voted for the block that was finalized" (proving the vote was cast correctly without revealing which validator cast it)
- **Threshold achievement:** "2f+1 valid validators voted for this batch" (an aggregate proof that the BFT threshold was met)

This is the most speculative tier. Commonware's BLS aggregate signatures already provide *some* anonymity — the aggregate doesn't reveal individual signers. But ZK would go further: it would prove *eligibility* and *correctness* without even revealing the validator set composition, if combined with a commitment scheme.

The research here is open. Anonymous BFT participation with ZK proofs is an active research area (whisper-sub, anonymous consensus). We're watching it. The architecture is designed to accommodate it when the cryptography catches up.

### Why Three Tiers Matter

Each tier protects a different kind of sensitive data:

| Tier | Protects | From whom |
|------|----------|-----------|
| Reflex | Sensor values, control profiles | Auditors, competitors |
| Intent | Routes, payloads, targets | Truth Market operators, gate |
| Consensus | Validator identity, participation patterns | Network observers, adversaries |

Without all three, privacy leaks at the weakest layer. You can prove your sensors were safe (Tier 1), but if the intent layer exposes your route (Tier 2), competitors can reconstruct your operational profile. You can hide your intents (Tier 2), but if consensus participation reveals which validators process your messages (Tier 3), adversaries can target your infrastructure.

The same cryptographic primitives — Groth16, BN254, MiMC, Merkle trees — serve all three tiers. The same on-chain `zk-verifier` contract can verify proofs from any tier. The same BN254 precompile makes all on-chain verification cheaper.

---

## Code

- **Sensor safety circuit (Tier 1):** `circuits/sensor-safety/src/lib.rs` — `SensorSafetyCircuit`, MiMC hash, Merkle tree builder, range constraints
- **Moultbook membership circuit (identity):** `circuits/moultbook-membership/src/lib.rs` — anonymous agent identity proofs
- **On-chain ZK verifier:** `contracts/zk-verifier/src/contract.rs` — Groth16 proof verification on-chain (pure Wasm + BN254 precompile paths)
- **CLI tool:** `circuits/sensor-safety/examples/gen-safety-proof.rs` — setup / prove / verify
- **On-chain contracts:** `contracts/safety-envelope/`, `contracts/circuit-breaker/`, `contracts/merkle-verifier/`
- **Plugin wiring:** `plugins/plugin-ros2/src/onchain.rs` — queries on-chain contracts with in-memory fallback
- **Consensus engine:** `crates/junoclaw-coordination/src/consensus.rs` — Commonware simplex BFT, 4 nodes, 300ms blocks
- **Intent message schema:** `crates/junoclaw-coordination/src/message.rs` — `IntentMessage` with `sensor_snapshot_hash`, `execution_proof_ref`

---

## Roadmap

### Tier 1 (reflex) — built, now hardening
1. **On-chain Groth16 verifier integration** — wire `zk-verifier` contract to verify sensor safety proofs on-chain (contract exists, integration pending)
2. **Poseidon hash upgrade** — replace MiMC with Poseidon for standard compliance (~30% fewer constraints)
3. **Proof aggregation** — combine multiple cycle proofs into a single batch proof
4. **Recursive proofs** — prove "all N cycles in this batch are within envelope" with a single proof
5. **TEE-attested proving** — generate proofs inside a TEE for witness integrity

### Tier 2 (intent) — next build target
6. **Intent consistency circuit** — prove intent's `sensor_snapshot_hash` matches reflex batch Merkle leaf
7. **Policy compliance circuit** — prove intent parameters satisfy governance policy without revealing them
8. **Composed proof** — single proof spanning reflex + intent tiers (membership + safety + policy)

### Tier 3 (consensus) — research phase
9. **Anonymous validator membership** — ZK proof of validator eligibility without revealing identity
10. **Private vote correctness** — prove correct BFT participation without revealing which validator voted
11. **Aggregate threshold proof** — prove 2f+1 threshold met without revealing the signer set

### Cross-cutting
12. **BN254 precompile** — 1.82× gas reduction for all on-chain verification (patches ready, advocacy ongoing)
13. **Proof composition** — unified circuit spanning all three tiers in a single proof

---

*The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. And at every tier — reflex, intent, consensus — the same question: prove it was right, without showing what "it" was.*
