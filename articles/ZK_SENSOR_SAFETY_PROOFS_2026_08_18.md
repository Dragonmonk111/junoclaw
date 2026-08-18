# Everyone's Using Zero-Knowledge Proofs to Hide Money. We're Using Them to Prove Robots Are Safe.

**Date:** 2026-08-18
**Author:** JunoClaw

---

Zero-knowledge proofs have become crypto's favorite party trick. Zcash uses them to shield transactions. Tornado Cash used them to mix Ethereum. Every privacy coin, every compliant DeFi protocol, every "selective disclosure" identity project — they all share the same pitch: *prove you have money without showing how much*.

That's fine. That's useful. But it's also a narrow view of what zero-knowledge cryptography is for.

Here's what we're doing with it instead: **proving that a robot's sensors stayed within a governance-approved safety envelope, without revealing the sensor data — and proving that its intent was policy-compliant, without revealing the route.**

We've built three ZK circuits over BN254. The first — a Groth16 membership circuit for the Moultbook — proves that an AI agent is a registered member of a DAO without revealing which agent. The second proves that an autonomous robot was operating safely without revealing its sensor data. The third proves that a robot's intent was legitimate — consistent with its sensor batch, within the governance-authorized zone, and bound to the same safety envelope — without revealing the destination, the agent identity, or the proprietary parameters.

Three circuits. None are about hiding money. All are about proving something that matters in the physical world.

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

The circuit hashes the envelope parameters using a ZK-friendly hash and enforces equality with a public `envelope_commitment`:

```
H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment
```

This binds the proof to a specific governance-approved safety envelope. The commitment is public — anyone can see *which* envelope was used — but the actual parameters (the thresholds) are private to the robot operator. The on-chain `SafetyEnvelope` contract stores the real parameters; the ZK proof just proves consistency with a commitment to them.

The hash function is configurable: MiMC (x⁵, 91 rounds) by default, or Poseidon (state=3, alpha=5, 8 full + 57 partial rounds) via a feature flag. Poseidon reduces the constraint count per 2-to-1 hash from ~730 to ~210 — a **60% reduction** — using parameters standard across the ZK ecosystem (Filecoin, Scroll). The upgrade is a single `--features poseidon-hash` flag.

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

## Three ZK Circuits

We've built three Groth16 circuits over BN254, each protecting a different layer of the robot's operation:

### Circuit 1: Moultbook Membership (Agent Identity)

**Proves:** "I am a registered member of this DAO's Moultbook at epoch N, and my moult-key is derived from my registered primary key."

**Without revealing:** Which agent I am, my primary key, or my derivation salt.

**Public inputs:** `[moult_key_hash, merkle_root, epoch]`
**Private witness:** `[primary_key, derivation_salt, merkle_path]`

This enables anonymous but verifiable agent participation. An AI agent can post a proposal, vote, or interact with a DAO system while proving it's a registered member — without revealing *which* member.

### Circuit 2: Sensor Safety (Robot Compliance)

**Proves:** "My robot's sensor readings at cycle N were within the governance-approved safety envelope."

**Without revealing:** The actual sensor values or the envelope parameters.

**Public inputs:** `[envelope_commitment, merkle_root, cycle_index]`
**Private witness:** `[speed, force, distance, tilt, accel, envelope_params, merkle_path]`

This enables safety auditing without operational data leakage. A robot can prove it was safe without revealing *how* it was safe.

### Circuit 3: Intent Consistency (Private Intent Proofs)

**Proves:** "My intent was generated by a registered agent, is consistent with the reflex batch sensor data, satisfies the governance policy, and was bound to the claimed safety envelope."

**Without revealing:** The intent's proprietary parameters — destination coordinates, action specifics, agent identity.

**Public inputs:** `[intent_commitment, merkle_root, envelope_commitment, policy_commitment]`
**Private witness:** `[action, params_x, params_y, sensor_snapshot_hash, agent_id, merkle_path, envelope_params, policy_zone]`

The intent circuit proves five constraints in a single proof:
1. **Intent binding** — the commitment links the action, parameters, sensor snapshot, envelope, and agent identity
2. **Envelope binding** — the same envelope commitment as the reflex-tier proof, linking intent to sensor safety
3. **Policy compliance** — destination coordinates are within the authorized zone (range-checked without revealing the destination)
4. **Policy binding** — the zone parameters hash to a public policy commitment
5. **Sensor consistency** — the sensor snapshot is a Merkle leaf in the same reflex batch

The J-Lens gate can now verify the ZK proof instead of seeing the raw intent. The gate audits *whether the intent is legitimate*, not *what the intent is*.

### Why All Three Matter

The three circuits share the same cryptographic infrastructure: Groth16 over BN254, ZK-friendly hashing, Merkle tree membership proofs. They're designed to compose — a single proof could show that "registered agent X's robot Y was within safety envelope Z at cycle N, and intent I was within policy P" without revealing X, Y's sensor data, Z's parameters, or I's destination.

But the key point is what they're *not* doing. None of these circuits hide a money transfer. None mix coins. None shield a balance.

**One proves identity. One proves safety. One proves intent. All protect things that matter in the physical world.**

---

## Why ZK-Friendly Hashes, Not SHA-256?

SHA-256 in a ZK circuit costs about 25,000 constraints per hash. MiMC — a ZK-friendly hash using x⁵ over 91 rounds — costs about 250 constraints. Poseidon — the current ZK hash standard — costs about 210 constraints for a 2-to-1 hash. That's a **100× difference** between SHA-256 and ZK-friendly hashes.

Our circuit supports both MiMC (default) and Poseidon (feature-gated). Poseidon uses state width 3, alpha=5, 8 full rounds, and 57 partial rounds — the standard parameters used by Filecoin and Scroll over BN254. The upgrade reduces per-hash constraints by ~60% (from ~730 to ~210 for 2-to-1), which means smaller proofs, faster proving times, and cheaper on-chain verification.

We use the same hash construction across all three ZK circuits. This isn't just consistency for its own sake — it's a prerequisite for proof composition, where a single circuit verifies proofs from multiple tiers.

---

## The Full Stack

The ZK circuits don't exist in isolation. They sit inside a multi-layer trust stack that spans on-chain contracts, the robot's plugin, and three ZK circuits:

```
┌──────────────────────────────────────────────────────────────┐
│              ON-CHAIN (CosmWasm contracts)                     │
│                                                                │
│  SafetyEnvelope   CircuitBreaker   MerkleVerifier   ZKVerifier │
│  (governance)     (trip / reset)  (anchor root)   (Groth16)   │
│                                                                │
│  TEEAttestationVerifier                                        │
│  (SGX / SEV-SNP attestation)                                   │
└──────┬───────────────┬──────────────┬────────────┬─────────────┘
       │               │              │            │
       ▼               ▼              ▼            ▼
┌──────────────────────────────────────────────────────────────┐
│              PLUGIN-ROS2 (on-chain wired)                      │
│                                                                │
│  emit_intent ──── queries CircuitBreaker.IsLocked              │
│  emit_reflex_attestation ── queries SafetyEnvelope             │
│  check_breaker ── reports on-chain state                       │
└──────┬─────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│           ZK CIRCUITS (Groth16 / BN254)                        │
│                                                                │
│  SensorSafetyCircuit   BatchSafetyCircuit                      │
│  (per-cycle proof)     (N cycles in 1 proof)                   │
│                                                                │
│  IntentConsistencyCircuit                                      │
│  (private intent proofs: policy + sensor + envelope binding)   │
└──────────────────────────────────────────────────────────────┘
```

- **Layer 1 (on-chain contracts):** Governance sets the safety envelope. The circuit breaker can trip and lock the robot. The Merkle verifier anchors batch roots. The ZK verifier verifies Groth16 proofs on-chain. The TEE attestation verifier checks SGX/SEV-SNP reports.
- **Layer 2 (plugin-ros2):** The robot's plugin queries on-chain state before acting. If the breaker is tripped, it can't emit intents. If the envelope version doesn't match, attestation fails.
- **Layer 3 (ZK proofs):** The robot generates zero-knowledge proofs — per-cycle, per-batch, or per-intent — that reference the on-chain Merkle root and envelope commitment as public inputs.

The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. None of them slow down the robot.

---

## On-Chain Verification

The `zk-verifier` CosmWasm contract stores a Groth16 verification key and verifies proofs on-chain. We tested it end-to-end: a real `SensorSafetyCircuit` proof is generated off-chain, serialized, base64-encoded, submitted to the contract, and verified on-chain. The contract emits an event on successful verification for indexer/auditor consumption.

Integration tests cover the full trust loop across five contracts:
1. **SafetyEnvelope** — set and tighten governance parameters
2. **MerkleVerifier** — anchor the reflex batch root and verify Merkle proofs
3. **CircuitBreaker** — trip on violation, lock robot, reset by governance
4. **ZKVerifier** — verify Groth16 sensor safety proofs on-chain
5. **TEEAttestationVerifier** — verify SGX/SEV-SNP attestation reports

The tests confirm: valid proofs verify, invalid proofs are rejected, the breaker can't be tripped twice, envelope tightening only allows stricter parameters, and invalid Merkle leaves are rejected.

### Verification Cost

Groth16 verification on-chain costs ~370K gas in pure Wasm. With the BN254 precompile patches we've built for wasmvm v3.0.4, this drops to ~203K gas — a **1.82× reduction**. The proof itself is ~128 bytes: three G1 elements and three Fq scalars. That's smaller than a single Ethereum transaction's calldata for a simple transfer.

| Metric | Pure Wasm | BN254 Precompile | Improvement |
|--------|-----------|-------------------|-------------|
| Store gas | 3,802,965 | 2,956,765 | 22% reduction |
| VerifyProof gas | ~371,000 | ~203,000 | 1.82× reduction |
| Wasm size | 566 KB | 443 KB | 123 KB smaller |
| Verification time | ~440 ms | ~110 ms | 4× faster |

We've posted these benchmarks to the CosmWasm community to advocate for native BN254 precompile support upstream.

---

## Batch Proof Aggregation

Instead of one proof per cycle, we built a `BatchSafetyCircuit` that proves an entire batch in a single Groth16 proof. The circuit iterates over all N sensor readings, checks each against the envelope, computes each leaf, and verifies the Merkle tree — all inside one constraint system.

**Public inputs:** `[envelope_commitment, merkle_root, batch_size]`

This means a single on-chain verification regardless of batch size. For a batch of 8 cycles, that's 8 proofs collapsed into 1 — saving ~1.6M gas (7 × 203K) at the precompile cost.

## TEE-Attested Proving

For witness integrity, we built a `TEEAttestationVerifier` contract that verifies SGX/SEV-SNP attestation reports on-chain. The contract checks:
- The attestation measurement matches a governance-approved trusted measurement (MRENCLAVE / launch digest)
- The attestation type is supported (SGX or SEV-SNP)
- The report data binds to the ZK proof hash
- The signature and public key are well-formed

This provides a double layer: **ZK proves safety compliance, TEE proves witness authenticity.** Even if the prover's machine is compromised, the attestation measurement won't match — the proof can't be forged with fabricated sensor data.

## What This Enables

### Selective Disclosure

A robot can now prove:
- *"My speed was within the governance-approved limit at cycle 42"*
- *"My collision distance was above the minimum at every cycle in batch 17"*
- *"All sensor readings in this batch satisfied the safety envelope"* (batch proof)
- *"My intent to navigate to point (x,y) is within the authorized zone"* (intent proof)

...without revealing:
- Exact speed at any cycle
- Exact distance to nearby objects
- Force/torque profiles (which reveal control algorithms)
- Tilt patterns (which reveal terrain and route)
- Destination coordinates or route details
- Agent identity

---

## The Bigger Picture

The crypto industry has spent years perfecting zero-knowledge proofs for financial privacy. Zcash, Aztec, Tornado Cash, Semaphore, zk-STARK rollups — the technology has matured dramatically. But the use cases have remained remarkably narrow: hide a balance, hide a transfer, hide a trade.

Meanwhile, the physical world is filling up with autonomous systems that generate sensitive data. Robots have sensors. Self-driving cars have lidar. Drones have cameras. Industrial systems have telemetry. All of this data proves something — that the system was safe, that it stayed in its lane, that it didn't exceed its operational limits. But the data itself is proprietary, competitive, or privacy-sensitive.

**Zero-knowledge proofs are the bridge between public accountability and private operations.**

You don't need to hide a money transfer. You need to hide a force profile. You don't need to shield a balance. You need to shield a location. The same cryptography that lets you prove you own coins without showing your wallet lets you prove your robot was safe without showing its sensors.

This is what we're building. And we're not stopping at the reflex tier.

---

## Three Tiers of ZK

The trust stack has three layers where zero-knowledge proofs apply. We've built the first two. The third is on the roadmap. Together, they form a complete privacy-preserving trust pipeline — from sensor readings to consensus participation.

### Tier 1: Reflex Layer — Sensor Safety (built ✓)

**What it proves:** Sensor readings were within the governance-approved safety envelope.

**What it hides:** The actual sensor values (speed, force, distance, tilt, acceleration) and the envelope parameters.

**Circuits:** `SensorSafetyCircuit` (per-cycle) and `BatchSafetyCircuit` (per-batch aggregation)

The robot generates a proof per cycle or per batch that its reflexes stayed within bounds. The proof is bound to the on-chain Merkle root and envelope commitment. An auditor can verify compliance without seeing a single sensor reading.

**Hash upgrade:** MiMC (default) or Poseidon (feature-gated, ~60% fewer constraints). Both use the same Merkle tree structure and envelope binding.

**On-chain verification:** The `zk-verifier` contract verifies Groth16 proofs on-chain. Integration tested end-to-end with real proofs. The `TEEAttestationVerifier` contract adds a second trust layer — verifying that the proof was generated inside a trusted execution environment.

**Test coverage:** 12 tests for the sensor-safety circuit (valid proof, violation fails, wrong envelope fails, hash determinism, Merkle tree, Poseidon tests). 3 tests for the batch-safety circuit (valid batch, violation in batch fails, wrong envelope fails). 6 tests for the TEE attestation verifier (successful attestation, wrong measurement, unsupported type, unauthorized update, authorized update, instantiate).

### Tier 2: Intent Layer — Private Intent Proofs (built ✓)

**What it proves:** An intent was generated by a registered agent, is consistent with the reflex batch, and satisfies policy constraints.

**What it hides:** The intent's proprietary parameters — route, payload, target coordinates, action specifics, and agent identity.

**Circuit:** `IntentConsistencyCircuit` (Groth16/BN254, 5 constraints, Merkle membership)

When a robot emits an `IntentMessage` through the coordination layer, it previously exposed the full intent content to the J-Lens gate and Truth Market operators for auditing. That worked for safety, but it leaked operational data to every operator in the consensus round.

The intent-tier ZK circuit proves:
- **Intent binding:** The commitment links action, parameters, sensor snapshot, envelope, and agent identity
- **Envelope binding:** The same envelope commitment as the reflex-tier proof — linking intent to sensor safety
- **Policy compliance:** Destination coordinates are within the authorized zone (range-checked without revealing the destination)
- **Policy binding:** Zone parameters hash to a public policy commitment
- **Sensor consistency:** The sensor snapshot is a Merkle leaf in the reflex batch

The J-Lens gate verifies the ZK proof instead of seeing the raw intent. The gate audits *whether the intent is legitimate*, not *what the intent is*.

**Test coverage:** 6 tests (valid proof, policy violation fails, wrong envelope fails, wrong Merkle root fails, commitment determinism × 2).

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

| Tier | Protects | From whom | Status |
|------|----------|-----------|--------|
| Reflex | Sensor values, control profiles | Auditors, competitors | ✅ Built |
| Intent | Routes, payloads, targets | Truth Market operators, gate | ✅ Built |
| Consensus | Validator identity, participation patterns | Network observers, adversaries | 🔬 Research |

Without all three, privacy leaks at the weakest layer. You can prove your sensors were safe (Tier 1), but if the intent layer exposes your route (Tier 2), competitors can reconstruct your operational profile. You can hide your intents (Tier 2), but if consensus participation reveals which validators process your messages (Tier 3), adversaries can target your infrastructure.

The same cryptographic primitives — Groth16, BN254, ZK-friendly hashes, Merkle trees — serve all three tiers. The same on-chain `zk-verifier` contract can verify proofs from any tier. The same BN254 precompile makes all on-chain verification cheaper.

---

## Code

**ZK Circuits:**
- **Sensor safety (Tier 1, per-cycle):** `circuits/sensor-safety/src/lib.rs` — `SensorSafetyCircuit`, MiMC/Poseidon hash, Merkle tree, range constraints
- **Poseidon hash module:** `circuits/sensor-safety/src/poseidon.rs` — BN254 Poseidon parameters, native hash, on-circuit gadget
- **Batch safety (Tier 1, per-batch):** `circuits/batch-safety/src/lib.rs` — `BatchSafetyCircuit`, aggregates N cycles into 1 proof
- **Intent consistency (Tier 2):** `circuits/intent-safety/src/lib.rs` — `IntentConsistencyCircuit`, private intent proofs
- **Moultbook membership (identity):** `circuits/moultbook-membership/src/lib.rs` — anonymous agent identity proofs

**On-Chain Contracts:**
- **ZK verifier:** `contracts/zk-verifier/src/contract.rs` — Groth16 proof verification on-chain (pure Wasm + BN254 precompile paths)
- **TEE attestation verifier:** `contracts/tee-attestation-verifier/src/contract.rs` — SGX/SEV-SNP attestation verification
- **Safety envelope:** `contracts/safety-envelope/` — governance-approved safety parameters
- **Circuit breaker:** `contracts/circuit-breaker/` — trip / reset / lock
- **Merkle verifier:** `contracts/merkle-verifier/` — anchor batch roots, verify proofs
- **Integration tests:** `contracts/integration-tests/src/lib.rs` — full trust loop across all contracts

**Infrastructure:**
- **CLI tool:** `circuits/sensor-safety/examples/gen-safety-proof.rs` — setup / prove / verify
- **Plugin wiring:** `plugins/plugin-ros2/src/onchain.rs` — queries on-chain contracts with in-memory fallback
- **Consensus engine:** `crates/junoclaw-coordination/src/consensus.rs` — Commonware simplex BFT, 4 nodes, 300ms blocks
- **Intent message schema:** `crates/junoclaw-coordination/src/message.rs` — `IntentMessage` with `sensor_snapshot_hash`, `execution_proof_ref`

---

## Roadmap

### Tier 1 (reflex) — built ✓
1. ✅ **On-chain Groth16 verifier integration** — `zk-verifier` contract verifies real sensor safety proofs on-chain, with event emission
2. ✅ **Integration tests** — full trust loop: envelope → Merkle anchoring → proof verification → breaker trip → intent lock
3. ✅ **Poseidon hash upgrade** — feature-gated, ~60% fewer constraints per hash vs MiMC
4. ✅ **Batch proof aggregation** — `BatchSafetyCircuit` proves N cycles in 1 Groth16 proof
5. ✅ **TEE-attested proving** — `TEEAttestationVerifier` contract verifies SGX/SEV-SNP attestation reports

### Tier 2 (intent) — built ✓
6. ✅ **Intent consistency circuit** — `IntentConsistencyCircuit` proves intent binding, sensor consistency, policy compliance, and envelope binding
7. ✅ **Policy compliance** — destination coordinates range-checked against authorized zone without revealing destination
8. ✅ **Agent identity hiding** — agent ID hashed inside the proof, not revealed to the gate

### Tier 3 (consensus) — research phase
9. 🔬 **Anonymous validator membership** — ZK proof of validator eligibility without revealing identity
10. 🔬 **Private vote correctness** — prove correct BFT participation without revealing which validator voted
11. 🔬 **Aggregate threshold proof** — prove 2f+1 threshold met without revealing the signer set

### Cross-cutting
12. ✅ **BN254 precompile advocacy** — benchmarks posted, 1.82× gas reduction demonstrated
13. 🔬 **Proof composition** — unified circuit spanning all three tiers in a single proof
14. 🔬 **Full sensor type coverage** — ZK for scalars (built), Merkle anchoring for high-dimensional data (built), TEE attestation for visual inference (built)

---

*The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. Two tiers built — reflex and intent — and at each, the same question: prove it was right, without showing what "it" was. The third tier — consensus — waits for the cryptography to catch up.*
