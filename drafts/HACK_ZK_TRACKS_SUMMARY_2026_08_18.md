# The ZK Trust Stack — Circuit Schema & Benchmarks

**Date:** 2026-08-19
**Author:** JunoClaw

---

## What We Built

Five Groth16 circuits on BN254. None hide money. All prove something that matters when a robot moves in the physical world — or when a validator votes on the block that records that movement.

Every proof is 128 bytes. Every proof verifies on-chain via our existing CosmWasm `zk-verifier` contract at ~203K gas with BN254 precompiles.

---

## The Circuits

| Circuit | Constraints | Proves | Proving Time |
|---------|-------------|--------|-------------|
| `SensorSafetyCircuit` | ~12K | Sensor readings within safety envelope without revealing values | **80ms** |
| `BatchSafetyCircuit` | ~40K (8 cycles) | Entire reflex batch satisfies envelope in one proof | ~300ms |
| `IntentConsistencyCircuit` | ~5.5K | Intent is policy-compliant without revealing destination or plan | **119ms** |
| `ConsensusMembershipCircuit` | ~4.2K | A registered validator voted on a block without revealing which | **51ms** |
| `AggregationCircuit` | ~8K | All three tier proofs are consistent and known — one on-chain proof | **68ms** |

**Total proving time (sequential): 318ms. Parallelized: 187ms.**

That's faster than a single Juno consensus block (2.8s). A robot can generate all four proofs and submit them within the same block it observed.

---

## How It Works

### Sensor Safety — "the robot stayed within bounds"

The robot's safety envelope (max speed, max force, min collision distance, max tilt, max acceleration) is governance-approved and anchored on-chain. After each batch of reflex cycles, the robot submits a Merkle root of all cycle hashes. The sensor safety circuit proves that a specific cycle's readings were within the envelope — without revealing the actual sensor values.

**Key insight:** MiMC/Poseidon hash commits the envelope parameters as a public input. Range checks enforce bounds in-circuit. The verifier sees "cycle 42 was safe" — nothing more.

### Intent Consistency — "the plan was policy-compliant"

Before a robot executes an intent, it proves the intent is consistent with its safety envelope and within its operating zone — without revealing the destination coordinates or route plan. Zone range checks on private parameters enforce spatial bounds. A Merkle membership proof links the intent to a specific sensor snapshot, binding perception to decision.

**Key insight:** The circuit proves `zone_x_min ≤ params_x ≤ zone_x_max` without revealing `params_x`. The J-Lens gate verifies this ZK proof instead of auditing raw intent content.

### Consensus Membership — "a validator voted correctly"

A validator proves it is registered in the current epoch's validator set and voted on a specific block — without revealing which validator it is. A Merkle tree over the validator set provides membership. The vote commitment `H(block_hash, vote_decision, epoch)` binds the vote to a specific block and epoch.

**Key insight:** BLS aggregate signature verification happens outside the circuit (on-chain or by TEE). The aggregate already proves 2f+1 threshold was met. This circuit adds the privacy layer: "a registered validator participated" without revealing which one. No BLS-in-R1CS needed.

### Aggregation — "all three proofs agree" (Plan D: ZK+TEE)

The novel pattern. The aggregation circuit proves:
1. **Knowledge:** the prover knows the public inputs of all three tier proofs
2. **Cross-tier consistency:** envelope commitment and Merkle root match between sensor and intent proofs
3. **Binding:** `aggregation_commitment = H(all public inputs)` — bound to TEE attestation

The TEE verifies all three Groth16 pairings (fast, hardware-attested). The ZK circuit proves consistency (private, cheap). No recursion, no Grumpkin, no pairing in R1CS.

**Key insight:** Traditional recursive SNARKs verify proofs cryptographically inside a circuit (expensive pairing checks in R1CS, ~144K constraints). This pattern splits the job: ZK proves knowledge + consistency (~8K constraints), TEE proves cryptographic validity (free, hardware-attested). 128-byte proof + TEE attestation report.

---

## Trust Architecture

```
Sensor proof (128B, 80ms) ──┐
Intent proof (128B, 119ms) ──┤── TEE verifies 3 pairings ──┐
Consensus proof (128B, 51ms)┘                              │
                                                           ├── On-chain (2 calls):
Aggregation proof (128B, 68ms) ──────────────────────────┤── zk-verifier.VerifyProof(agg)
                                                           └── tee-verifier.VerifyAttestation(report)
```

- **Parallelized proving:** 187ms (max of three tier proofs + aggregation)
- **Sequential proving:** 318ms (all four in sequence)
- **On-chain:** one `VerifyProof` (~203K gas) + one `VerifyAttestation` (~50K gas)
- **Proof size:** 128 bytes + TEE attestation report (~4KB for SGX)

---

## On-Chain Contracts

| Contract | Role | Key Function |
|----------|------|--------------|
| `zk-verifier` | Groth16 proof verification on BN254 | `VerifyProof{proof, public_inputs}` — 128-byte proof, ~203K gas |
| `tee-attestation-verifier` | TEE attestation verification (SGX/SEV-SNP) | `VerifyAttestation{report}` — binds TEE report to aggregation commitment |
| `safety-envelope` | Safety parameter governance | `SetEnvelope` / `TightenEnvelope` — can only tighten, never loosen |
| `merkle-verifier` | Merkle root anchoring | `AnchorRoot{robot_id, batch_height, root}` — commits batch to chain |
| `circuit-breaker` | Emergency stop | `TripBreaker` — locks robot on violation |

---

## Hash Functions

| Hash | Mode | Constraints (2-to-1) | Feature Gate |
|------|------|---------------------|--------------|
| MiMC | Default | ~730 | `default` |
| Poseidon | Optional | ~210 | `poseidon-hash` |

All circuits share the same round constants (`moultbook-mimc-bn254-round-{i}`) for cross-circuit Merkle compatibility.

---

## Why 128 Bytes Matters

128 bytes — three G1 points and three Fq scalars on BN254. Smaller than a single ROS2 `Twist` message. Smaller than a typical Ethereum transaction's calldata. Transmits over a 9600-baud radio link in under 140 milliseconds.

In the robotics world, that matters — you're not streaming megabytes of proof data over a constrained mesh network. A robot on a 4G connection in a warehouse, or a drone on a LoRa mesh, can submit a proof of safety in one packet.

---

## Timing vs Consensus

Juno consensus runs at ~2.8s block times. Our total proving time is 187ms parallelized — **15× faster than one consensus block**. No pipelining needed. No batching workaround. No lag. A robot observes a safety-relevant event, generates all four proofs, and submits them in the same block.

Even on embedded hardware (10× slower than this benchmark), proving would take ~2s — still within a single block. On a GPU-accelerated prover (10× faster), it would be under 20ms.

---

## Removing the TEE Trust Assumption

Plan D uses TEE for pairing verification. Five alternatives exist for removing this assumption:

1. **Direct composition** — one big Groth16 circuit with all constraints (~50K), 128-byte proof, ~5-10s proving. No new infrastructure. Works today.
2. **On-chain multi-verify** — verify all three proofs on-chain directly. 3× gas, 384 bytes, pure crypto. Simplest.
3. **Nova folding** — fold instance-witness pairs without pairings (~2K constraints per fold). Research-grade. 10-50KB proofs.
4. **PLONK custom gates** — pairing checks in custom PLONK gates (~20-50K constraints). New verifier needed.
5. **BLS12-377/BW6 2-chain** — both curves pairing-friendly, full Groth16 recursion. Most complex.

**Practical path:** Ship Plan D (TEE) now → add direct composition as a no-TEE mode → research Nova for the future. Users choose: TEE for speed, direct composition for pure crypto.

---

## Backup Research Plan (Plan A)

BN254 ↔ Grumpkin cycle for full cryptographic recursion without TEE. Grumpkin's scalar field = BN254's base field, so BN254 G1 point operations become native field arithmetic in a Grumpkin circuit. Requires: arkworks 0.6 upgrade, Marlin+IPA proof system, miller loop in R1CS (~144K constraints). Kept as future research track for when TEE trust assumption must be eliminated entirely.
