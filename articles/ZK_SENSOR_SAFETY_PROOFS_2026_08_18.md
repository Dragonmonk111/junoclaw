# Everyone's Using Zero-Knowledge Proofs to Hide Money. We're Using Them to Prove Robots Are Safe.

**Date:** 2026-08-18
**Author:** JunoClaw

---

ZK proofs are crypto's favorite party trick — Zcash shields transactions, Tornado Cash mixes Ethereum, every privacy coin hides balances. Useful, but narrow.

We're using ZK for something else: **proving robots are safe without revealing their sensor data, and proving their intents are policy-compliant without revealing the route.**

We built three Groth16 circuits over BN254. None hide money. All prove something that matters in the physical world.

---

## The Problem

A robot operates in a shared space. Its safety envelope — max speed, max force, min collision distance, max tilt, max acceleration — is governance-approved and anchored on-chain via CosmWasm contracts. The robot's reflex controller runs at sub-100ms cycles, fusing sensor data to stay within bounds.

After each batch of cycles, the robot submits a Merkle root of all cycle hashes on-chain. The root is public. The batch is committed.

**But how do you prove cycle 42's readings were within the envelope without revealing the actual sensor values?**

Sensor data is sensitive — not "someone might steal my Bitcoin" sensitive, but "this reveals how my robot works" sensitive:
- **Distance sensors** reveal location and environment geometry
- **Force/acceleration profiles** reveal proprietary control algorithms
- **Speed patterns** reveal routes and schedules
- **Tilt data** reveals terrain, which competitors can use to map your operating area

You need to prove **compliance** without **disclosure**.

---

## What We Built

Three ZK circuits, each protecting a different layer:

### Circuit 1: Moultbook Membership — Agent Identity

Proves "I'm a registered DAO member" without revealing *which* member. Uses a Merkle tree over registered agent keys.

### Circuit 2: Sensor Safety — Robot Compliance

Proves sensor readings were within the safety envelope. Three constraints in one 128-byte proof:

1. **Range checks** — each reading satisfies its limit (speed ≤ max_speed, etc.). The circuit decomposes `b - a` into 64 boolean bits and reconstructs it. If non-negative, `a ≤ b`. No values revealed.

2. **Envelope binding** — `H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment`. The commitment is public (which envelope was used), but the actual thresholds are private.

3. **Merkle membership** — `H(speed, force, distance, tilt, accel)` is a leaf in the batch's Merkle tree. The root is already on-chain. The proof shows *this set of readings* is in the tree without revealing which leaf.

**Hash function:** MiMC by default (x⁵, 91 rounds, ~250 constraints/hash). Poseidon optional via feature flag (state=3, alpha=5, 8 full + 57 partial rounds, ~210 constraints/hash — **60% fewer** than MiMC). SHA-256 would cost ~25,000 constraints per hash — 100× more.

### Circuit 3: Intent Consistency — Private Intent Proofs

When a robot decides to do something (navigate to a point, pick up an object), it emits an `IntentMessage`. Previously, the full intent was visible to the J-Lens gate and all consensus operators — leaking routes, destinations, and proprietary parameters.

The intent circuit proves five things in one proof:
1. **Intent binding** — commitment links action, parameters, sensor snapshot, envelope, and agent identity
2. **Envelope binding** — same envelope commitment as the reflex-tier proof (links intent to sensor safety)
3. **Policy compliance** — destination coordinates are within the authorized zone (range-checked, destination hidden)
4. **Policy binding** — zone parameters hash to a public policy commitment
5. **Sensor consistency** — sensor snapshot is a Merkle leaf in the same reflex batch

The gate verifies the ZK proof instead of seeing the raw intent. It audits *whether* the intent is legitimate, not *what* it is.

---

## The Stack

```
ON-CHAIN (CosmWasm)          PLUGIN-ROS2              ZK CIRCUITS
─────────────────────        ─────────────            ──────────────────────
SafetyEnvelope               queries                  SensorSafetyCircuit
CircuitBreaker               CircuitBreaker           BatchSafetyCircuit
MerkleVerifier               .IsLocked                IntentConsistencyCircuit
ZKVerifier                   queries SafetyEnvelope
TEEAttestationVerifier       checks envelope
                             version match
```

**On-chain:** Governance sets the envelope. The breaker trips on violations. The Merkle verifier anchors batch roots. The ZK verifier checks Groth16 proofs. The TEE verifier checks attestation reports.

**Plugin:** The robot queries on-chain state before acting. Breaker tripped? No intents. Envelope mismatch? Attestation fails.

**ZK proofs:** Generated per-cycle, per-batch, or per-intent. Reference on-chain Merkle root and envelope commitment as public inputs.

The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. None slow down the robot.

---

## On-Chain Verification

The `zk-verifier` contract verifies real Groth16 proofs on-chain — tested end-to-end with actual `SensorSafetyCircuit` proofs. Integration tests cover the full trust loop across five contracts: SafetyEnvelope, MerkleVerifier, CircuitBreaker, ZKVerifier, and TEEAttestationVerifier.

### Gas Costs

| Metric | Pure Wasm | BN254 Precompile | Improvement |
|--------|-----------|-------------------|-------------|
| Store gas | 3,802,965 | 2,956,765 | 22% reduction |
| VerifyProof gas | ~371,000 | ~203,000 | 1.82× reduction |
| Wasm size | 566 KB | 443 KB | 123 KB smaller |
| Verification time | ~440 ms | ~110 ms | 4× faster |

The proof is ~128 bytes — smaller than a typical Ethereum transaction's calldata. We've posted these benchmarks to the CosmWasm community to advocate for native BN254 precompile support.

---

## Batch Aggregation

Instead of one proof per cycle, the `BatchSafetyCircuit` proves an entire batch in a single Groth16 proof. All N sensor readings are checked against the envelope, hashed into leaves, and Merkle-verified inside one constraint system. One on-chain verification regardless of batch size — 8 cycles collapsed into 1 proof saves ~1.6M gas.

## TEE-Attested Proving

The `TEEAttestationVerifier` contract checks SGX/SEV-SNP attestation reports on-chain: measurement matches governance-approved trusted measurement, attestation type is supported, report data binds to the ZK proof hash, and signature/pubkey are well-formed.

**ZK proves safety compliance. TEE proves witness authenticity.** Even if the prover's machine is compromised, the attestation measurement won't match — fabricated sensor data can't produce a valid attestation.

---

## Three Tiers

| Tier | Proves | Hides | Status |
|------|--------|-------|--------|
| **Reflex** | Sensor readings within envelope | Sensor values, envelope params | ✅ Built |
| **Intent** | Intent is legitimate & policy-compliant | Route, destination, agent identity | ✅ Built |
| **Consensus** | Validator participated correctly | Which validator, vote content | 🔬 Research |

Without all three, privacy leaks at the weakest layer. Prove sensors were safe but expose the route? Competitors reconstruct your operations. Hide the route but reveal which validators process your messages? Adversaries target your infrastructure.

All three tiers share the same primitives — Groth16, BN254, ZK-friendly hashes, Merkle trees. The same `zk-verifier` contract verifies proofs from any tier.

### Tier 3: Consensus (future)

The coordination layer uses a 4-node BFT mesh with 300ms block times. Today, validator participation is visible to all peers — revealing which validators process which robot's intents. A consensus-tier ZK circuit would prove validator eligibility, correct voting, and threshold achievement without revealing identities. This is active research (whisper-sub, anonymous consensus). The architecture accommodates it when the cryptography catches up.

---

## What This Enables

A robot can prove:
- *"My speed was within the limit at cycle 42"*
- *"All readings in batch 17 satisfied the safety envelope"* (batch proof)
- *"My intent to navigate to (x,y) is within the authorized zone"* (intent proof)

...without revealing:
- Speed, distance, force, tilt, or acceleration at any cycle
- Envelope parameters (the actual thresholds)
- Destination coordinates or route details
- Agent identity

---

## The Bigger Picture

The crypto industry perfected ZK proofs for financial privacy. Meanwhile, the physical world is filling up with autonomous systems that generate sensitive data — robots with sensors, self-driving cars with lidar, drones with cameras. All of this data proves something: the system was safe, stayed in its lane, didn't exceed limits. But the data itself is proprietary.

**ZK proofs are the bridge between public accountability and private operations.**

You don't need to hide a money transfer. You need to hide a force profile. You don't need to shield a balance. You need to shield a location.

---

## The Vision

Step back and look at what these three circuits add up to.

**A robot that remembers.** The Moultbook gives a robot a persistent, verifiable identity on-chain. It's not a session key or a certificate that expires — it's a membership proof anchored in a DAO. The robot can prove *who it is* across sessions, across operators, across chains. When it switches owners or manufacturers, its identity history stays intact and auditable. A robot with a Moultbook membership is a robot with a reputation that can't be erased.

**A robot that plans with others.** The intent circuit lets a robot prove *what it's about to do* is legitimate — without revealing the plan. Two robots from different manufacturers can coordinate in a shared warehouse: each proves its intent is within policy, neither sees the other's route. A human supervisor can audit the intent proof without seeing proprietary coordinates. The J-Lens gate checks legitimacy, not content. Robots plan together. Secrets stay secret.

**A robot that works safely in tandem.** The sensor safety circuit proves each robot stayed within its envelope. The batch aggregation circuit proves an entire shift's worth of cycles in one proof. The TEE attestation proves the proof itself wasn't forged. Two robots from competing firms can share a workspace, each proving safety to the same on-chain verifier, neither revealing sensor data to the other. Safe collaboration without competitive leakage.

**A complete robotic OS to plug into.** This isn't a single robot or a single firm's stack. It's an operating system for autonomous machines:

- **On-chain contracts** define governance — safety envelopes, circuit breakers, Merkle roots, policy zones. Any firm can deploy them. Any chain can run them.
- **The plugin layer** (plugin-ros2) bridges any ROS2-compatible robot to the on-chain world. Boston Dynamics, Unitree, Agility — any robot that speaks ROS2 can plug in.
- **The ZK circuits** prove safety, identity, and intent without revealing proprietary data. A logistics company proves its fleet is safe without exposing routes. A hospital proves its robots stayed within force limits without revealing patient proximity data. A manufacturer proves its assembly arms stayed within tilt bounds without revealing its production line layout.
- **The consensus layer** (4-node BFT, 300ms blocks) finalizes batches of intents and attestations. Multiple operators validate each other's robots. No single firm controls the truth.

The pitch to a massive firm isn't "put your robot on our blockchain." It's: **plug your robot into a trust stack where safety is provable, identity is persistent, collaboration is safe, and your proprietary data never leaves your machine.**

The ZK proof is 128 bytes. The trust is infinite.

---

## Test Coverage

- **Sensor safety:** 12 tests (valid proof, violation fails, wrong envelope, hash determinism, Merkle tree, Poseidon mode)
- **Batch safety:** 3 tests (valid batch, violation in batch, wrong envelope)
- **Intent consistency:** 6 tests (valid proof, policy violation, wrong envelope, wrong Merkle root, determinism)
- **TEE attestation:** 6 tests (success, wrong measurement, unsupported type, unauthorized/authorized update, instantiate)
- **Integration:** Full trust loop across 5 contracts (envelope → Merkle → proof → breaker → intent lock)

---

## Code

**Circuits:** `circuits/sensor-safety/` (per-cycle), `circuits/batch-safety/` (per-batch), `circuits/intent-safety/` (intent-tier), `circuits/moultbook-membership/` (identity)

**Contracts:** `contracts/zk-verifier/` (Groth16 verification), `contracts/tee-attestation-verifier/` (SGX/SEV-SNP), `contracts/safety-envelope/`, `contracts/circuit-breaker/`, `contracts/merkle-verifier/`, `contracts/integration-tests/`

**Infrastructure:** `plugins/plugin-ros2/src/onchain.rs` (on-chain queries), `crates/junoclaw-coordination/` (BFT consensus + intent schema), `circuits/sensor-safety/examples/gen-safety-proof.rs` (CLI)

---

## Roadmap

**Tier 1 (reflex) — built ✓**
- ✅ On-chain Groth16 verifier (real proofs verified on-chain)
- ✅ Integration tests (full trust loop across 5 contracts)
- ✅ Poseidon hash upgrade (~60% fewer constraints)
- ✅ Batch proof aggregation (N cycles → 1 proof)
- ✅ TEE-attested proving (SGX/SEV-SNP verification)

**Tier 2 (intent) — built ✓**
- ✅ Intent consistency circuit (5 constraints: binding, envelope, policy, sensor, identity)
- ✅ Policy compliance (destination range-checked, hidden)
- ✅ Agent identity hiding (hashed inside proof)

**Tier 3 (consensus) — research phase**
- 🔬 Anonymous validator membership
- 🔬 Private vote correctness
- 🔬 Aggregate threshold proof (2f+1 without revealing signer set)

**Cross-cutting**
- ✅ BN254 precompile advocacy (1.82× gas reduction demonstrated)
- 🔬 Proof composition (unified circuit spanning all tiers)

---

*The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. Two tiers built — reflex and intent — and at each, the same question: prove it was right, without showing what "it" was. The third tier — consensus — waits for the cryptography to catch up.*
