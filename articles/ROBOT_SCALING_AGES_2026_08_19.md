# The Robot Remembers: ZK Safety Proofs and the Scaling Ages of Autonomous Machines

*From 8-millisecond reflexes to 128-byte proofs to 300-millisecond coordination — a deterministic plan for robotics safety across five eras of hardware.*

---

> *"The robot does not need permission to move. It needs proof that it moved safely. The proof is smaller than the command that moved it."*

---

## Prologue: The Timing Gap

A robot's balance controller adjusts motor torque in 8 milliseconds. Its collision avoidance module triggers a lateral dodge in 12 milliseconds. These are sensor-fusion loops running at 100-1000Hz — physics, not decisions.

A Juno consensus block takes 2.8 seconds.

Between 8 milliseconds and 2.8 seconds, there is a gap of 350×. Every robotics-blockchain project in history has died in this gap. They tried to route reflexes through consensus. The robot collided before the first vote was cast.

We took a different approach. The reflexes run at full speed. After the batch, the robot generates a proof — in 187 milliseconds — that every cycle was safe. That proof is 128 bytes. Then the proof enters the coordination layer — a Commonware BFT mesh running at ~300ms block times — where agents audit the proof through a J-Lens gate, fetch moultbook context for the decision, vote on the outcome via DAO consensus, and trip the circuit breaker if the proof shows a violation. Only then does the settled result land on Juno at 2.8 seconds.

Four timescales: 8ms reflex, 187ms proof, ~300ms coordination, 2.8s settlement. Each gates the next, none gates physics.

This is the story of how we measured those numbers, why they're safe for the next decade, and what happens when robots scale from one to a billion.

---

## Act I: The Market Reality

On August 19, 2026, Unitree — a Chinese humanoid robot maker — surged **over 600%** in its Shanghai market debut. It opened at 1,100 yuan ($163) per share against an IPO price of ~150 yuan ($22). Its market capitalization briefly reached **445 billion yuan ($66 billion)**.

This is not a VC fantasy. This is not a lab prototype. This is a public company with a public valuation. A $66 billion robot company. On day one.

The implication is immediate: **the question is no longer whether humanoid robots scale. The question is how fast — and who builds the safety layer before they do.**

A robot with a $66 billion market cap cannot operate without an audit trail. It cannot move in public spaces without proving it followed safety rules. It cannot have its parameters edited by an operator without governance. It cannot collide with a person and have no cryptographic record of what happened.

This is why the reflex-tier trust stack exists. Not as an academic exercise. As the legal, financial, and social prerequisite for robots at population scale. When a robot company is worth $66 billion, the cost of one unverifiable collision is not just money — it is the end of public trust in the entire industry.

The five circuits are not a research project. They are a **liability firewall**. They let a robot company say: "Our robot moved. Here is 128 bytes proving it was safe. Here is the on-chain record of the safety envelope it followed. Here is the circuit breaker that would have tripped if it wasn't."

That's a $66 billion-scale answer to a $66 billion-scale question.

---

## Act II: The Five Circuits

Five Groth16 circuits on BN254. Each proves one thing. All produce 128-byte proofs.

| Circuit | What It Proves | Constraints | Proving |
|---------|---------------|-------------|---------|
| SensorSafety | Sensor readings within governance-approved envelope — without revealing values | ~12K | 80ms |
| BatchSafety | N reflex cycles aggregated into one proof via hash chain | ~40K (8 cycles) | ~300ms |
| IntentConsistency | Intent is within operating zone — without revealing coordinates | ~5.5K | 119ms |
| ConsensusMembership | A registered validator voted on a block — without revealing which one | ~4.2K | 51ms |
| Aggregation (Plan D) | All three tier proofs agree — TEE verifies pairings, ZK proves consistency | ~8K | 68ms |

The aggregation circuit is the novel pattern. TEE verifies the three inner Groth16 pairings — fast, hardware-attested. ZK proves cross-tier consistency — private, cheap. No recursion. No Grumpkin. No pairing in R1CS.

---

## Act III: The Measurement

Estimated 3-7 seconds. Actual: **187ms parallelized** on a commodity AMD Ryzen 5 5600H.

| Circuit | Proving | Verifying |
|---------|---------|-----------|
| SensorSafety | 80 ms | 3 ms |
| IntentConsistency | 119 ms | 5 ms |
| ConsensusMembership | 51 ms | 3 ms |
| Aggregation | 68 ms | 2 ms |
| **Total (parallelized)** | **187 ms** | **13 ms** |

187ms = 6.7% of one 2.8s Juno block. 15× headroom on a mid-range laptop CPU, no GPU.

### How the four timescales relate

Reflexes run at 8-12ms. Proofs take 187ms. Coordination takes ~300ms. Settlement takes 2.8s. **Nothing gates physics. Each gates the next decision.**

```
Reflex loop:       8ms/cycle    → hardware speed, no consensus, no proof
                    ↓
Batch complete:    N cycles     → Merkle tree of all cycle hashes
                    ↓
Proof generation:  187ms        → 4 Groth16 proofs in parallel
                    ↓
Coordination:      ~300ms       → Commonware BFT mesh orders the proof
                    ├── J-Lens gate audits the intent (red/yellow/green)
                    ├── Moultbook context fetched (heartbeat history, prior decisions)
                    ├── Multi-operator Truth Market evaluates the outcome
                    └── Circuit breaker checks: trip if violation detected
                    ↓
Settlement:        2.8s         → Juno on-chain verification in 13ms
                    ├── zk-verifier.VerifyProof(agg)
                    └── tee-verifier.VerifyAttestation(report)
                    ↓
Circuit breaker:   if violation → intent tier locked, robot grounded
```

Proofs don't gate reflexes. The coordination layer doesn't gate proofs. Each layer gates the **next decision** — not the physics. The robot moves first. The proof follows. The coordination mesh audits the proof and the intent together. If either shows a violation, the circuit breaker trips — no new intents until governance reset.

The coordination layer is where agents decide outcomes. It's a Commonware P2P mesh running BFT consensus at ~300ms block times — fast enough to consume intent-tier decisions within a robot's decision cycle, slow enough to be deliberate. Every `AgentMessage` passes through a J-Lens gate before acceptance: red-gated messages are blocked, yellow-gated messages carry warnings, green-gated messages pass. The mesh fetches moultbook context — heartbeat history, prior decisions, topic chains — so operators evaluating an intent can see the robot's full provenance. Multiple operators evaluate independently; the Truth Market aggregates their verdicts; consensus determines honesty. The settled outcome lands on Juno as final on-chain evidence.

---

## The 300ms Boundary: What Coordination Is — and Isn't

A robot running at 8-10 m/s covers 25-35 centimeters between camera frames at 30 fps. The perception-to-action loop for collision avoidance, balance correction, and curve navigation must close in under 33 milliseconds — or the robot has already hit the wall before it sees it. This is the reflex tier. It runs on-robot, at 100-1000Hz, and it will never go through a coordination mesh.

**The 300ms coordination layer is not for reflexes. It is for the decisions above reflexes.**

When a humanoid skateboard controller like HUSKY runs its whole-body policy at 50 Hz (20ms) with a 500 Hz PD torque loop (2ms), it is solving the physics problem: stay balanced, push, steer by leaning. The coordination layer solves a different problem: *should this robot be skateboarding down this particular sidewalk right now?* Is the intent safe? Do other operators agree? Does the robot's heartbeat history show anomalies? Should the circuit breaker trip?

This is the intent tier — strategic, auditable, governable. It includes decisions like:

- **Route selection**: "Take the warehouse corridor, not the loading dock."
- **Mode switching**: "Switch from patrol speed to sprint — approved by 3 of 4 operators."
- **Task delegation**: "Robot A handles the north perimeter; Robot B holds position."
- **Safety overrides**: "Abort current trajectory — J-Lens gate flagged the intent as red."
- **Provenance queries**: "Fetch the last 10 heartbeat attestations for this robot before approving."

None of these are real-time control. None of them gate motor torque. They gate **the next intent** — the next strategic decision the robot is allowed to make. The robot's own policy handles the physics between intents. The coordination mesh handles the audit between intents.

A high-context model doesn't need to reason from scratch in 300ms. The robot's policy already generated the intent. The model's job is **evaluation**, not generation: fetch pre-indexed moultbook context (sub-50ms), run the J-Lens gate audit (one classification call), aggregate multi-operator verdicts in parallel across the block window. Classification is faster than generation. Audit is faster than planning. 300ms is enough for that.

The industry is already converging on this split — whether it knows it or not. When Eren Chen writes that "autonomy is the new bottleneck" for humanoids running into barriers at speed, he is describing the reflex-tier perception problem. When HUSKY separates its 50 Hz policy from its 500 Hz PD loop, it is implementing the reflex/hardware split. The missing layer — the one nobody has built yet — is the intent tier: the coordination mesh that audits what the robot decided to do, before the chain settles it. That is what the ~300ms Commonware layer is.

---

## Act IV: The Scaling Ages

The real question is not "is 187ms safe today." It is: **will it stay safe as robots scale?**

Two forces compete: circuit complexity grows (more sensors, larger validator sets, fleet aggregation), hardware speed grows (better algorithms, GPU, ASIC). Here is the deterministic plan.

### Age 0: Now (2026) — Commodity CPU

| Parameter | Value |
|-----------|-------|
| Hardware | AMD Ryzen 5 5600H (commodity laptop) |
| Hash function | MiMC (91 rounds, ~730 constraints per 2-to-1) |
| Total constraints | ~30K across all circuits |
| Proving time | 187ms parallelized, 318ms sequential |
| Coordination block | ~300ms (Commonware BFT, 4-node mesh) |
| Settlement block | 2.8s (Juno) |
| **Total decision cycle** | **~487ms proof + coordination, 17% of one Juno block** |
| **Headroom** | **5.7× against Juno block time** |

Proof finishes in 6.7% of one block time. Coordination adds ~300ms — together, 487ms is still 17% of one 2.8s Juno block. Even on embedded ARM (~10× slower), proving takes ~1.87s and coordination adds ~300ms — still within one 2.8s block. A Jetson Orin Nano can prove and coordinate in real-time.

### Age 1: Poseidon (2026-2027) — Algorithmic Speedup

| Parameter | Value |
|-----------|-------|
| Change | Enable Poseidon hash (already built, feature-gated) |
| Constraint reduction | ~3.5× fewer hash constraints (210 vs 730 per hash) |
| Expected proving time | ~60-80ms parallelized |
| **Headroom** | **35×** |

Already built behind a `poseidon-hash` feature flag. One build flag, no new code. Constraints drop from ~30K to ~12K. Proving drops to ~60ms — faster than a human blink.

### Age 2: GPU Proving (2027-2028) — Hardware Speedup

| Parameter | Value |
|-----------|-------|
| Hardware | Consumer GPU (RTX 4070 or equivalent) |
| Speedup | 10-100× for MSM and NTT operations |
| Expected proving time | ~2-8ms parallelized |
| **Headroom** | **350-1400×** |

Groth16 proving is dominated by MSM and NTT — both parallelize near-linearly on GPU. arkworks already has GPU backends. At 2-8ms, proving becomes irrelevant — faster than a camera frame capture. Edge Jetson Orin: ~20-40ms. Data center A100: ~2-4ms.

### Age 3: Fleet Aggregation (2028-2030) — Scaling the Problem

| Scenario | Approach | Proving Time | On-chain Cost |
|----------|----------|-------------|---------------|
| 1 robot (current) | Plan D: 4 proofs + aggregation | 187ms | 1× VerifyProof + 1× TEE |
| 10 robots | 10× independent proofs, on-chain multi-verify | 187ms (parallel) | 10× VerifyProof |
| 100 robots | 100× independent + TEE-attested batch | 187ms (parallel) | 1× VerifyProof + batch attestation |
| 1,000 robots | Nova folding (research) or hierarchical aggregation | ~200ms + fold overhead | 1× VerifyProof (folded) |
| 10,000+ robots | Hierarchical: per-robot proofs → district aggregation → global proof | ~500ms (3-level) | 1× VerifyProof |

**Invariant:** proving time scales with circuit size, not robot count. Each robot proves independently in 187ms. The aggregation circuit is constant-size (~8K constraints).

Three fleet options:

- **A — On-chain multi-verify (today):** 100 robots = 100× 203K gas = 20.3M gas. No new code.
- **B — TEE-attested batch (extends Plan D):** One VerifyProof regardless of fleet size. TEE verifies N pairings in ~50ms.
- **C — Nova folding (research):** ~2K constraints per fold. Avoids pairing-in-R1CS entirely. 10-50KB proofs. Needs new verifier.

### Age 4: ASIC Proving (2030+) — The End of Concern

| Parameter | Value |
|-----------|-------|
| Hardware | FPGA/ASIC provers (Ulvetanna, Cysuit) |
| Speedup | 100-1000× |
| Expected proving time | <1ms for circuits up to 1M constraints |
| **Headroom** | **>2800×** |

Ulvetanna and Cysuit are building FPGA/ASIC provers. MSM and NTT map directly to hardware — 1M-point MSM in <1ms. Proving drops below sensor ADC noise. The question becomes meaningless. The answer is always yes.

### The Scaling Wall

| Growth Factor | Constraint Cost | When It Matters |
|---------------|----------------|-----------------|
| +1 Merkle depth | +730 (MiMC) / +210 (Poseidon) | 100 sensors → +5K |
| +1 safety parameter | +200 (range check) | 6-DOF → +1.2K |
| +1 validator doubling | +730 (membership) | 10K validators → +10K |
| Fleet aggregation | N × individual OR constant (Plan D) | See Age 3 |

**Wall:** ~500K constraints = ~5s on commodity CPU (~17× current). A 100-sensor 6-DOF robot with 10K validators = ~40K constraints, proving in ~250ms. The wall only approaches for fleet-level single-circuit aggregation — and by then, GPU/ASIC moves it 100-1000× further out.

---

## Act V: The 128-Byte Proof

128 bytes. Three G1 points, three Fq scalars on BN254. Smaller than a ROS2 `Twist` message. Smaller than the JSON metadata describing its own file format.

Transmits over 9600-baud LoRa in 140ms. Over 4G in <1ms. One packet — no fragmentation, no retransmission. A warehouse robot, a drone on LoRa mesh, a search-and-rescue robot on satellite — all submit proof of safety in one packet.

**The proof is smaller than the sensor data it proves.**

---

## Act VI: Removing the TEE

Plan D uses a TEE (Intel SGX / AMD SEV-SNP) to verify the three inner Groth16 pairings. The TEE is a trust assumption — hardware can be compromised. Five paths to remove it:

| Path | How | Trade-off |
|------|-----|----------|
| **1. Direct Composition** | One big circuit, all tiers, ~50K constraints | No TEE, but 5-10s proving, shared proving key |
| **2. On-Chain Multi-Verify** | 3× VerifyProof calls, 609K gas | Simplest no-TEE path, works today, more gas |
| **3. Nova Folding** | Fold instance-witness pairs, no pairings | 10-50KB proofs, new verifier, research-grade |
| **4. PLONK Custom Gates** | Pairing in custom gates, ~20-50K constraints | New proof system, trusted setup |
| **5. BLS12-377/BW6** | Full recursion on both sides | Most complex, different curve, complete |

**Practical path:** Ship Plan D (TEE) now. Add Path 2 (multi-verify) as no-TEE mode. Research Path 3 (Nova) for the future. Users choose. Circuits don't change — only the verification path.

---

## Epilogue: The Robot That Remembers

The robot moves at 8ms. The proof follows in 187ms. The coordination mesh audits it in ~300ms. The block closes in 2.8s. The proof is 128 bytes — smaller than the command that moved the robot.

The robot does not ask permission. It moves first — fast, local, autonomous. Then it remembers. And the memory is cryptographic. And the memory is tiny. And the coordination layer — agents, gates, moultbook context, truth markets — decides what to do about that memory before the chain ever sees it.

If the proof shows a violation, the circuit breaker trips. The robot is grounded. No human judgment. No clipboard audit. Just 128 bytes of math, audited by a mesh of operators, settled on a chain.

**128 bytes. 187ms proof. ~300ms coordination. 2.8s settlement. Four timescales, one deterministic plan from commodity CPU to ASIC.**

---

## Links & Resources

- **GitHub**: [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)
- **Circuits**: `circuits/sensor-safety/`, `circuits/intent-safety/`, `circuits/consensus-safety/`, `circuits/proof-aggregation/`
- **Benchmark**: `circuits/proof-aggregation/examples/proving_times.rs`

### Previous Articles

1. [You Can't Gate Physics: A Reflex-Tier Trust Stack for Autonomous Robots](articles/REFLEX_TIER_TRUST_STACK_2026_08_17.md)
2. [ZK Sensor Safety Proofs: Privacy for Robot Telemetry](articles/ZK_SENSOR_SAFETY_PROOFS_2026_08_18.md)
3. **This article** — The Robot Remembers: ZK Safety Proofs and the Scaling Ages

---

*Built August 2026. Five circuits. 128 bytes each. 187ms proving. ~300ms coordination. 2.8s settlement. Four timescales. One deterministic plan from commodity CPU to ASIC.*
