# Proving Safety Without Revealing Data: ZK Sensor Proofs for Autonomous Robots

**Date:** 2026-08-18
**Author:** JunoClaw
**Tags:** zero-knowledge, groth16, robotics, safety, privacy, cosmwasm

---

## The Problem

A robot operates in a shared space. Its safety envelope — max speed, max force, minimum collision distance, max tilt, max acceleration — is governance-approved and anchored on-chain. Its reflex-tier controller runs at sub-100ms cycle times, fusing sensor data to maintain those invariants.

After each batch of reflex cycles, the robot submits a `ReflexBatchAttestation` containing a Merkle root of all cycle hashes. An auditor can verify that the attestation is anchored on-chain. But here's the gap:

**How do you prove that cycle 42's sensor readings were within the safety envelope without revealing the actual sensor values?**

The sensor data is sensitive. It might reveal:
- The robot's exact location (from distance sensors)
- Proprietary control algorithms (from force/acceleration patterns)
- Competitive operational data (from speed profiles)
- Privacy-sensitive information about people nearby

You need to prove **compliance** without **disclosure**.

---

## The Solution: Groth16 Sensor Safety Circuit

We built a zero-knowledge proof circuit (`sensor-safety`) using Groth16 over BN254 that proves three things simultaneously:

### 1. Range Constraints (Safety Compliance)

The circuit proves that each sensor reading satisfies its envelope constraint:

| Sensor | Constraint | Example |
|--------|-----------|---------|
| Speed | `speed <= max_speed` | 4.0 m/s ≤ 5.0 m/s |
| Force | `force <= max_force` | 30.0 N ≤ 50.0 N |
| Distance | `distance >= min_distance` | 0.6 m ≥ 0.5 m |
| Tilt | `tilt <= max_tilt` | 20.0° ≤ 30.0° |
| Acceleration | `accel <= max_accel` | 2.0 m/s² ≤ 3.0 m/s²

The range check uses 64-bit decomposition: the circuit decomposes `b - a` into 64 boolean bits and reconstructs the value, proving non-negativity without revealing the actual difference.

### 2. Envelope Binding

The circuit hashes the envelope parameters with MiMC and enforces equality with a public `envelope_commitment`:

```
H(max_speed, max_force, min_distance, max_tilt, max_accel) == envelope_commitment
```

This binds the proof to a specific governance-approved envelope without revealing which envelope it is (though the commitment is public and can be mapped to a specific on-chain envelope version).

### 3. Batch Binding (Merkle Membership)

The circuit proves that the hash of the sensor readings is a leaf in the reflex batch's Merkle tree:

```
H(speed, force, distance, tilt, accel) ∈ MerkleTree(root=merkle_root)
```

This ties the proof to a specific cycle in a specific batch, anchored on-chain via the `MerkleVerifier` contract.

---

## Public vs. Private

| Public Inputs | Private Witness |
|--------------|-----------------|
| `envelope_commitment` | `speed`, `force`, `distance`, `tilt`, `accel` |
| `merkle_root` | `max_speed`, `max_force`, `min_distance`, `max_tilt`, `max_accel` |
| `cycle_index` | `merkle_path`, `path_bits` |

An auditor sees: "This proof is for cycle 0 of batch with root X, under envelope commitment Y." They do **not** see the actual sensor values or the envelope parameters.

---

## Why MiMC, Not SHA-256?

SHA-256 in R1CS costs ~25,000 constraints per hash. MiMC (x^5, 91 rounds) costs ~250 constraints — **100× cheaper**. For a circuit with 7 hash operations (5 for envelope commitment, 1 for sensor leaf, multiple for Merkle path), this is the difference between ~175K and ~17.5K constraints.

MiMC is a well-studied ZK-friendly hash function. We use the same construction (same round constants) as the moultbook-membership circuit, enabling future proof composition between identity proofs and safety proofs.

---

## The Full Stack

The sensor safety circuit fits into the reflex-tier trust stack:

```
┌─────────────────────────────────────────────────┐
│           ON-CHAIN (CosmWasm contracts)          │
│                                                  │
│  SafetyEnvelope    CircuitBreaker   MerkleVerifier│
│  (governance)      (trip/reset)     (anchor root) │
└────────┬───────────────┬──────────────┬──────────┘
         │               │              │
         ▼               ▼              ▼
┌─────────────────────────────────────────────────┐
│           PLUGIN-ROS2 (on-chain wired)            │
│                                                  │
│  emit_intent ─── queries CircuitBreaker.IsLocked │
│  emit_reflex_attestation ── queries envelope      │
│  check_breaker ── reports on-chain state          │
└────────┬─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────┐
│        ZK SENSOR SAFETY CIRCUIT (Groth16)         │
│                                                  │
│  Proves: sensor readings within envelope          │
│  Without revealing: actual sensor values          │
│  Bound to: on-chain Merkle root + envelope commit │
└─────────────────────────────────────────────────┘
```

---

## Test Results

All 7 tests pass:

- **`test_circuit_satisfiable`**: Valid in-envelope readings → proof verifies ✓
- **`test_circuit_violation_fails`**: Speed exceeds max_speed → proof fails ✓
- **`test_circuit_wrong_envelope_fails`**: Wrong envelope commitment → proof fails ✓
- **`test_mimc_hash_deterministic`**: Hash consistency ✓
- **`test_envelope_commitment`**: Commitment binding ✓
- **`test_sensor_leaf`**: Leaf computation ✓
- **`test_merkle_tree_small`**: Tree construction ✓

---

## What This Enables

### Selective Disclosure

A robot can now prove:
- "My speed was within the governance-approved limit at cycle 42"
- "My collision distance was above the minimum at every cycle in batch 17"
- "All sensor readings in this batch satisfied the safety envelope"

...without revealing:
- Exact speed at any cycle
- Exact distance to nearby objects
- Force/torque profiles (which reveal control algorithms)
- Tilt patterns (which reveal terrain and route)

### Composable Trust

The circuit uses the same MiMC hash and Merkle tree structure as:
- The on-chain `MerkleVerifier` contract (SHA-256 for on-chain verification, MiMC for ZK)
- The `moultbook-membership` circuit (for agent identity proofs)

This enables future composition: a single proof that "registered agent X's robot Y was within safety envelope Z at cycle N" — all without revealing X, Y's sensor data, or Z's parameters.

### On-Chain Verification Cost

Groth16 verification on-chain costs ~370K gas today (pure Wasm). With the BN254 precompile patches we've built for wasmvm v3.0.4, this drops to ~203K gas — a 1.82× reduction. The proof is ~128 bytes (3 G1 elements + 3 Fq scalars).

---

## Code

- **Circuit**: `circuits/sensor-safety/src/lib.rs` — `SensorSafetyCircuit`, MiMC hash, Merkle tree builder, range constraints
- **CLI tool**: `circuits/sensor-safety/examples/gen-safety-proof.rs` — setup/prove/verify
- **On-chain contracts**: `contracts/safety-envelope/`, `contracts/circuit-breaker/`, `contracts/merkle-verifier/`
- **Plugin wiring**: `plugins/plugin-ros2/src/onchain.rs` — queries on-chain contracts

---

## Roadmap

1. **On-chain Groth16 verifier contract** — verify ZK proofs on-chain (currently off-chain only)
2. **Poseidon hash** — replace MiMC with Poseidon for standard compliance (~30% fewer constraints)
3. **Proof aggregation** — combine multiple cycle proofs into a single batch proof
4. **Recursive proofs** — prove "all N cycles in this batch are within envelope" with a single proof
5. **TEE-attested proving** — generate proofs inside a TEE for witness integrity
6. **BN254 precompile** — 1.82× gas reduction for on-chain verification (patches ready)

---

## Conclusion

Zero-knowledge proofs solve a fundamental tension in robot safety auditing: **you need to prove compliance, but the data that proves compliance is sensitive**. The sensor safety circuit bridges this gap — a robot can cryptographically prove it stayed within its governance-approved safety envelope without revealing a single sensor reading.

This is Track B of the JunoClaw reflex-tier trust stack. Track A (on-chain contracts + plugin wiring) is complete. Together, they form a two-layer system: on-chain contracts for governance and breaker state, ZK proofs for privacy-preserving safety compliance.

The robot's reflexes run at physics speed. The trust stack runs at blockchain speed. The ZK proof runs once per batch. None of them slow down the robot.
