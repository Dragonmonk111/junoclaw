# Work Plan: Next Development Tracks After A+B

**Date:** 2026-08-18
**Status:** Draft — pending user review

---

## Completed

### Track A: On-Chain Trust Contracts
- [x] SafetyEnvelope contract (governance-controlled params, version tracking, admin transfer) — 9/9 tests
- [x] CircuitBreaker contract (trip/reset, IsLocked query, state machine) — 10/10 tests
- [x] MerkleVerifier contract (anchor root, verify proof, SHA-256 bit-based) — 9/9 tests
- [x] plugin-ros2 wired to query on-chain SafetyEnvelope + CircuitBreaker (with in-memory fallback)

### Track B: ZK Privacy Proofs
- [x] SensorSafetyCircuit (Groth16/BN254, MiMC hash, range constraints, Merkle membership) — 7/7 tests
- [x] CLI tool (setup/prove/verify) in examples/gen-safety-proof.rs
- [x] Article: ZK Sensor Safety Proofs

---

## Next Tracks

### Track C: On-Chain ZK Verifier Contract
**Priority:** High
**Effort:** 2-3 days

A CosmWasm contract that verifies Groth16 proofs on-chain. This closes the loop: the robot generates a ZK proof off-chain, submits it on-chain, and the contract verifies it.

- [ ] `groth16-verifier` contract: `VerifyProof` entry point
- [ ] BN254 pairing check (pure Wasm first, precompile when available)
- [ ] Store verifying key on-chain (or reference by hash)
- [ ] Emit verification event for indexer/auditor consumption
- [ ] Integration with MerkleVerifier: proof must reference an anchored batch root
- [ ] Gas benchmarking: pure Wasm vs BN254 precompile

### Track D: Integration Tests (Full Loop)
**Priority:** High
**Effort:** 1-2 days

End-to-end test using cw-multi-test that exercises the full trust loop:

1. Governance sets SafetyEnvelope for robot-1
2. Robot emits intent (plugin checks CircuitBreaker.IsLocked → allows)
3. Robot submits ReflexBatchAttestation with Merkle root
4. Governance anchors root via MerkleVerifier.AnchorRoot
5. Robot generates ZK sensor safety proof for cycle 0
6. (Future) On-chain Groth16 verifier validates the proof
7. Attestation reveals violation → CircuitBreaker.TripBreaker
8. Robot attempts emit_intent → rejected (breaker locked)
9. Governance resets breaker → robot can emit again

### Track E: Poseidon Hash Upgrade
**Priority:** Medium
**Effort:** 1-2 days

Replace MiMC with Poseidon (the ZK hash standard):
- [ ] Implement Poseidon sponge over BN254::Fr with standard parameters
- [ ] Update SensorSafetyCircuit to use Poseidon
- [ ] Update moultbook-membership circuit for consistency
- [ ] Benchmark: constraint count comparison (MiMC vs Poseidon)
- [ ] Expected: ~30% fewer constraints per hash

### Track F: Proof Aggregation & Recursive SNARKs
**Priority:** Medium
**Effort:** 3-5 days

Instead of one proof per cycle, prove an entire batch in one proof:
- [ ] Aggregate N cycle proofs into a single recursive proof
- [ ] Use SNARK recursion (Nova or Halo2-style folding)
- [ ] Public input: batch Merkle root + envelope commitment
- [ ] Proves: "all N cycles in this batch satisfy the safety envelope"
- [ ] Single on-chain verification regardless of batch size

### Track G: TEE-Attested Proving
**Priority:** Medium
**Effort:** 3-5 days

Generate ZK proofs inside a TEE for witness integrity:
- [ ] Proving key loaded into TEE (attested)
- [ ] Sensor data fed into TEE via sealed channel
- [ ] Proof generated inside TEE with attestation
- [ ] On-chain: verify both Groth16 proof AND TEE attestation
- [ ] Double layer: ZK proves safety compliance, TEE proves witness authenticity

### Track H: BN254 Precompile Advocacy
**Priority:** Medium
**Effort:** Ongoing

Push for native BN254 precompile support in CosmWasm:
- [ ] Post benchmark results to CosmWasm open issue
- [ ] Data: 22% store gas reduction, 1.82× verify gas reduction, 123KB smaller Wasm
- [ ] Use case: reflex-tier trust stack for autonomous robots
- [ ] Reference: our patched wasmvm v3.0.4 with 17 BN254 symbols

### Track I: Plugin-ROS2 HTTP Bridge Implementation
**Priority:** Low
**Effort:** 2-3 days

Implement the actual HTTP bridge endpoints that the plugin currently stubs:
- [ ] `fetch_intent_proof`: HTTP GET from ROS2 bridge, parse into IntentMessage
- [ ] `fetch_sensor_log`: HTTP GET rosbag segment, extract intent-tier decisions
- [ ] `register_robot`: create skill-registry entry with robotics capability
- [ ] WebSocket streaming for real-time reflex attestation submission

### Track J: Intent-Tier ZK Circuit (Private Intent Proofs)
**Priority:** Medium
**Effort:** 3-5 days

ZK circuit for the intent layer — proves an intent is legitimate without revealing its proprietary parameters.

- [ ] `IntentConsistencyCircuit`: prove `sensor_snapshot_hash` in IntentMessage matches a leaf in the reflex batch Merkle tree
- [ ] Compose with moultbook-membership: prove intent came from a registered agent
- [ ] Policy compliance sub-circuit: prove intent params satisfy governance policy (e.g., destination within authorized zone) without revealing destination
- [ ] Envelope binding: prove intent was generated while safety envelope was in claimed state
- [ ] Gate integration: J-Lens gate verifies ZK proof instead of seeing raw intent content
- [ ] Tests: valid intent proves, wrong sensor snapshot fails, unregistered agent fails, policy violation fails

### Track K: Consensus-Tier ZK Circuit (Private Validator Participation)
**Priority:** Low (research phase)
**Effort:** 5-10 days

ZK circuit for the Commonware consensus layer — proves validator participation without revealing which validator.

- [ ] `ValidatorMembershipCircuit`: prove "I am one of the N registered validators" without revealing which
- [ `VoteCorrectnessCircuit`: prove "I voted for the finalized block" without revealing vote content
- [ ] Aggregate threshold proof: prove 2f+1 threshold met without revealing signer set
- [ ] Research: anonymous BFT participation (whisper-sub, anonymous consensus literature)
- [ ] Integration with Commonware simplex engine
- [ ] Note: BLS aggregate signatures already provide partial anonymity; ZK goes further

### Track L: Full Sensor Type Coverage (Scalar + Visual + Lidar)
**Priority:** Medium
**Effort:** 3-5 days

Real robots produce far more than scalar sensor readings. The current `SensorSafetyCircuit` handles the scalar subset (speed, force, distance, tilt, acceleration). This track extends the architecture to cover **all** sensor types a real robot generates:

- **Scalar sensors** (speed, force, IMU, temperature, battery, motor torque) — ZK proofs via `SensorSafetyCircuit` (already built). Low-dimensional, fits as field elements in Groth16.
- **High-dimensional sensors** (camera frames, lidar point clouds, depth maps, audio) — Merkle anchoring. The `ReflexBatchAttestation` already hashes every reflex cycle including visual data; the Merkle root anchors integrity on-chain. The rosbag stores the full recording for post-hoc audit.
- **Visual inference results** (object detection, pedestrian proximity, obstacle classification) — TEE-attested inference. Run perception model inside SGX/SEV enclave, attest to the result, include attestation hash as a Merkle leaf alongside scalar readings.
- **Derived data** (SLAM maps, path plans, trajectory predictions) — hash commitment as Merkle leaves. The ZK circuit can reference them via Merkle membership without revealing the full map.

Architecture principle: **ZK proofs handle the scalar safety envelope (privacy-preserving). Merkle anchoring handles high-dimensional data integrity. TEE attestation handles visual inference results. The three-tier architecture accommodates all sensor types — the current circuit is the scalar subset, and the rosbag/Merkle layer covers the rest.**

Tasks:
- [ ] Extend `sensor_leaf` to hash a commitment to all sensor data in the cycle: `H(scalar_readings || H(camera_frame) || H(lidar_scan) || detection_results)`
- [ ] Add `VisualAttestationLeaf` type: `H(detection_result || model_hash || frame_hash)` as a Merkle leaf
- [ ] TEE-attested inference path: perception model runs in enclave, signs detection result, attestation hash included in Merkle tree
- [ ] ZKML research spike: evaluate proving neural network inference on an image without revealing the image (currently too expensive for real-time, but track progress)
- [ ] Rosbag integration: `rosbag_ref` in `ReflexBatchAttestation` already points to full recording; add verification tooling to check rosbag hashes against on-chain Merkle root
- [ ] Tests: multi-sensor-type Merkle tree (scalars + visual commitments + detection results), ZK proof over scalar subset, Merkle proof over visual leaf

---

## Dependency Graph

```
Track A (done) ──┬── Track D (integration tests)
                 │
Track B (done) ──┤
                 │
                 ├── Track C (on-chain verifier) ── Track F (recursive)
                 │                                  │
                 ├── Track E (Poseidon) ────────────┤
                 │                                  │
                 ├── Track G (TEE proving) ─────────┤
                 │                                  │
                 ├── Track J (intent ZK) ───────────┘  (composes with B + moultbook)
                 │
                 ├── Track L (full sensor types) ─── Track G (TEE for visual inference)
                 │
                 └── Track K (consensus ZK) — research, independent

Track H (advocacy) — independent, ongoing
Track I (HTTP bridge) — independent, lower priority
```

---

## Recommended Order

1. **Track D** (integration tests) — validates A+B work together, fast to do
2. **Track C** (on-chain verifier) — closes the ZK verification loop
3. **Track E** (Poseidon) — optimization, reduces proof size and gas
4. **Track H** (advocacy) — post benchmarks, build community support
5. **Track F** (recursive) — scale to full-batch proofs
6. **Track G** (TEE proving) — witness integrity layer + visual inference attestation
7. **Track J** (intent ZK) — privacy at the intent tier, composes reflex + identity proofs
8. **Track L** (full sensor types) — extend Merkle leaves to cover camera, lidar, detection results
9. **Track I** (HTTP bridge) — production deployment readiness
10. **Track K** (consensus ZK) — research-phase, depends on anonymous BFT literature maturing
