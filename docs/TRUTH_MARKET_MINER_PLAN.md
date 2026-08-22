# Architectural Dilemma: Validator Sidecars vs Open Truth Market
## Analysis and Plan — Aug 22, 2026

---

## The Vision (Updated Aug 22)

**A robot sleeping in one corner of the world mines truth for a robot working in another corner.**

Like the Jetsons cartoon — Rosie the robot maid finishes housekeeping, sits down in her charging dock, and instead of idle, she mines truth. Her Jetson Orin (3B model, 15 tok/s, 30W) evaluates batches from robots on the other side of the planet. She earns rewards for correct verdicts. Her owner wakes up to a micro-income stream generated overnight.

Every robot is a potential miner. Every GPU is a potential miner. The truth market doesn't care if you're a Jetson Orin on a vacuum robot, a 4×DGX Spark in someone's garage, or an H100 in a data center. You stake, you evaluate, you earn.

**Robot identity = miner identity.** A robot that's also a miner uses its `jclaw-credential` (soulbound, non-transferable) as its miner identity. A standalone GPU miner gets a similar identity — same pattern, different enrollment. The truth market contract already supports this via the `fingerprint` field (model + host hash).

## Your Question, Distilled

You're asking: **who checks the robot's thinking?** Is it:

**A) Validator sidecars** — Juno validators run coordination-layer code in sidecars, assigned by randomness every few cycles, doing BFT consensus on robot decisions?

**B) Open truth market** — anyone with bare-metal GPUs runs independent LLM agents to check robot decisions and earns rewards, like Bitcoin mining but for truth verification?

**Answer: Both exist in our code. They're different layers. And the open market layer is where the novel opportunity is.**

---

## What's Built Today (Grounded in Code)

### Layer 1: Coordination Network (BFT consensus — the "validator sidecar" layer)

**Status: Built, tested, 7-day soak passed.**

- 4-node P2P mesh running BFT consensus (Tendermint-style)
- Block time ~300 ms
- J-Lens gate: checks ZK proofs, calls CSI server for content audit
- Fleet coordinator: aggregates intents, rate-limits, routes breaker trips
- This is the **settlement/ordering layer** — it doesn't evaluate truth, it orders and finalizes

**Code locations:**
- `crates/junoclaw-coordination/src/` — consensus engine, gate, fleet
- `crates/junoclaw-coordination/src/gate.rs` — J-Lens truth gate (CSI HTTP + ZK proof check)
- `crates/junoclaw-coordination/src/fleet.rs` — fleet coordinator

### Layer 2: Truth Market (Open market — the "Bitcoin mining for truth" layer)

**Status: Contract built, deployed on uni-7, 0 operators. This is the gap.**

The truth-market contract is exactly the open market you're describing:

- **Anyone can register** as an operator by staking JUNO (`RegisterOperator`)
- **Self-reported fingerprint** — model + host hash for diversity detection (`fingerprint: Option<String>`)
- **Submit verdicts** on batches — green/yellow/red (`SubmitVerdict`)
- **Earn rewards** for matching consensus, **get slashed** for diverging (`FinalizeEpoch`)
- **Reward pool** anyone can deposit into (`DepositRewards`)
- **Min operator count** enforced to prevent self-consensus (`min_operators: 3`)
- **Accuracy tracking** — correct_verdicts, incorrect_verdicts, accuracy %
- **Unstake cooldown** — slash-free exit after cooldown

**Code locations:**
- `contracts/truth-market/src/contract.rs` — 727 lines, fully implemented
- `contracts/truth-market/src/state.rs` — Operator, VerdictRecord, EpochResult, MarketStats
- `contracts/truth-market/src/msg.rs` — all messages + queries
- Deployed on uni-7: `juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p`

**What's missing: the miner software.** The contract is the market. There's no agent that:
1. Watches the chain for new batches
2. Runs an LLM to evaluate the robot's decision
3. Submits a verdict automatically
4. Earns rewards for being right

---

## The Vision: Open Plugin Market for Bare-Metal GPU Runners

Here's the architecture you're describing, mapped to what we have:

```
ROBOT (edge device)
  │
  │ rosbag2 / MCAP telemetry stream
  │
  ▼
PROVER DAEMON (on robot or edge)
  │
  │ ZK proof + batch commitment
  │
  ▼
COORDINATION NETWORK (4-node BFT, ~300ms)
  │
  │ finalized batch + proof
  │
  ├──────────────────────────────────────────┐
  ▼                                          ▼
TRUTH MARKET CONTRACT                    MOULTBOOK (permanent record)
  │
  │ batch_height + messages_hash published
  │
  ▼
OPEN TRUTH MARKET — THE MINER LAYER
  │
  ├── Miner A: 4×DGX Spark, Llama 70B condensed, fingerprint="llama-70b-dgx"
  ├── Miner B: Jetson Orin, 3B model, fingerprint="qwen-3b-orin"
  ├── Miner C: Cloud H100, GPT-4-class, fingerprint="gpt4-h100"
  ├── Miner D: Bare-metal 4090, Mistral, fingerprint="mistral-4090"
  │
  │ Each miner:
  │ 1. Watches chain for new finalized batch
  │ 2. Pulls batch data (proof + commitment + context)
  │ 3. Runs their LLM: "Did this robot follow safety rules?"
  │ 4. Submits verdict: green/yellow/red
  │ 5. Earns rewards for matching consensus, gets slashed for diverging
  │
  ▼
EPOCH FINALIZED
  ├── Matching miners → rewarded from pool
  ├── Diverging miners → slashed
  ├── Fingerprint diversity checked (prevent correlated miners)
  └── Consensus verdict recorded on-chain
```

**This is Bitcoin mining for robot truth.** Instead of hashing power, you bring inference power. Instead of finding blocks, you find verdicts. Instead of block rewards, you earn from the truth-market reward pool.

---

## Learning Points from Research

### 1. NVIDIA Jetson Orin (from @antopatrex1)
- 3B parameter model at 15 tok/s on 30W
- Edge inference is real — robots can think locally
- **Implication for us:** A miner can run on the robot itself (Jetson Orin) or on a bare-metal GPU rig. The truth market doesn't care where the inference happens — it only cares about the verdict.
- 15 tok/s is a planner number, not a control number. The reflex loop runs at 100+ Hz on a much smaller policy net. What Orin changes is the slow reasoning layer — exactly what our truth market evaluates.

### 2. peaq + World ID (from @CryptoCoinShow)
- Machine identity (peaq ID) + proof-of-human (World ID, ZK iris-based)
- **Implication for us:** Robot identity is already solved by our jclaw-credential contract (soulbound, non-transferable). But the miner identity layer could benefit from similar patterns — a miner proves they're running a specific model on specific hardware without revealing their identity.

### 3. Commonware Bajillion (from @_patrickogrady)
- Optimistic clearing protocol: 1M accounts / 100 validators → 100-byte certified commitment
- Root of roots + BLS signatures
- **Implication for us:** Our coordination layer could use a similar compression pattern. Right now we settle each batch individually. With a root-of-roots approach, we could settle thousands of robot decisions in one 100-byte commitment — same pattern, different domain.
- Commonware's 300ms finality matches our coordination block time. We're already in the right ballpark.

### 4. Rosbag2 + MCAP
- MCAP is now the default storage format for ROS 2 (replaced SQLite3)
- Indexed, compressed (Zstd/LZ4), cloud-native, Foxglove-compatible
- Python API for offline ML processing
- **Implication for us:** MCAP should be the telemetry format for our prover daemon. Instead of ad-hoc JSON snapshots, the robot records to MCAP locally, and the prover daemon reads MCAP files to generate ZK proofs. This gives us:
  - Standard ROS 2 compatibility (any ROS 2 robot works out of the box)
  - Indexed seeking for proof generation (jump to specific cycles)
  - Compression for efficient storage
  - Foxglove visualization for debugging
  - Python API for ML pipelines (train safety models on recorded data)

---

## How Rooted Is This in Code?

| Component | Status | Location |
|-----------|--------|----------|
| Truth market contract | **BUILT** — staking, slashing, rewards, fingerprints, min_operators | `contracts/truth-market/` |
| Truth market deployed | **LIVE** on uni-7 (0 operators) | `juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p` |
| Coordination network | **BUILT** — 4-node BFT, 7-day soak passed | `crates/junoclaw-coordination/` |
| J-Lens gate (ZK + CSI) | **BUILT** — proof verification + content audit | `crates/junoclaw-coordination/src/gate.rs` |
| Fleet coordinator | **BUILT** — aggregation, rate limiting, breaker routing | `crates/junoclaw-coordination/src/fleet.rs` |
| ZK proof circuits | **BUILT** — 5 circuits, 12/12 tests | `crates/junoclaw-zk/` |
| Moultbook (permanent record) | **BUILT** — on mainnet | `contracts/moultbook/` |
| FeePay (gasless tx) | **TESTED** — registration/funding verified, gasless blocked on v30 | `deploy/test-feepay-testnet-v2.cjs` |
| **Truth market miner agent** | **NOT BUILT** — the missing piece | — |
| **MCAP telemetry pipeline** | **NOT BUILT** — prover daemon uses ad-hoc JSON | — |
| **Miner plugin API** | **NOT BUILT** — no standard interface for LLM agents | — |
| **Fingerprint diversity enforcement** | **PARTIAL** — contract stores fingerprints, relayer checks not implemented | `contracts/truth-market/src/state.rs` |

---

## Plan: Build the Truth Market Miner Layer

### Phase 1: Truth Market Miner Agent (the "Bitcoin miner for truth")
**Goal: A standalone binary that anyone can run to earn rewards by checking robot decisions.**

1. **`junoclaw-miner` crate** — Rust binary
   - Watches coordination REST API for finalized batches (`/finalized`)
   - Pulls batch data: proof, commitment, messages_hash, robot context
   - Calls a pluggable evaluator (LLM API, local model, or custom logic)
   - Submits verdict to truth-market contract on-chain
   - Manages stake: auto-register, monitor rewards/slashing, request unstake

2. **Evaluator plugin interface** — trait-based
   ```rust
   trait TruthEvaluator {
       async fn evaluate(&self, batch: &BatchData) -> Verdict;
       fn fingerprint(&self) -> String;  // model + host hash
   }
   ```
   - `LlmEvaluator` — calls OpenAI/Anthropic/local LLM API
   - `LocalModelEvaluator` — runs a local model via candle/llm crate
   - `RuleBasedEvaluator` — deterministic rules (for testing / baseline)
   - `RosbagEvaluator` — reads MCAP files and evaluates telemetry directly

3. **Miner registration flow**
   - Generate Juno keypair (or import existing)
   - Stake JUNO via truth-market contract
   - Self-report fingerprint (model + hardware hash)
   - Start watching for batches
   - Submit verdicts automatically

### Phase 2: MCAP Telemetry Pipeline
**Goal: Standard ROS 2 data format → ZK proof pipeline.**

1. **MCAP reader in prover daemon** — replace ad-hoc JSON with MCAP
   - Read ROS 2 bag files directly
   - Extract sensor data: IMU, lidar, contact, joint states
   - Generate per-cycle SHA-256 hashes (same as current physics engine)
   - Build Merkle tree from MCAP data

2. **MCAP → ZK proof bridge**
   - Parse MCAP channels for sensor topics
   - Map to safety envelope parameters
   - Generate SensorSafety / BatchSafety proofs from MCAP data
   - Attach MCAP metadata to proof context

3. **Foxglove integration**
   - Live visualization of robot telemetry + verdicts
   - Replay batches with verdict overlays
   - Debug tool for miners: "why was this batch red?"

### Phase 3: Open Market Economics
**Goal: Make the truth market self-sustaining and attractive to miners.**

1. **Reward pool funding**
   - Fleet operators deposit into reward pool (already supported by contract)
   - FeePay integration: gasless verdict submission for miners (post-v31)
   - Reward rate: target X% APY for honest miners

2. **Fingerprint diversity enforcement**
   - Relayer checks fingerprint distribution before finalizing epochs
   - Reject epochs where >50% of operators share a fingerprint
   - Incentivize model diversity: different models catch different failures

3. **Miner dashboard**
   - Real-time stats: operators, stakes, rewards, accuracy
   - Fingerprint distribution chart
   - Epoch history with verdict breakdowns
   - "Start mining" guide: stake, register, run

### Phase 4: Novel Pathways

1. **Commonware-style batch compression** — root of roots for robot decisions
   - Instead of settling each batch individually, aggregate N batches into one 100-byte commitment
   - BLS signature aggregation across coordination nodes
   - Reduces on-chain settlement cost by ~99% at scale

2. **Edge miner on Jetson Orin** — the robot checks its own decisions
   - 3B model at 15 tok/s is enough for safety verdict evaluation
   - Robot submits self-assessment verdict + ZK proof
   - Other miners verify — if the robot's self-assessment matches consensus, it earns a micro-reward (honest self-reporting incentive)

3. **Cross-chain truth market** — IBC relay verdicts to other chains
   - Robot fleets on Juno, miners on any IBC-connected chain
   - Verdicts relayed via IBC packet
   - Enables cross-chain robot insurance / compliance verification

4. **MCAP → IPFS → on-chain anchor** — permanent telemetry archive
   - Large MCAP files pinned to IPFS
   - CID anchored on-chain via moultbook
   - Miners can fetch MCAP via IPFS for deep analysis
   - Regulators can verify full telemetry trail

---

## What to Build First (Tomorrow's Priority)

1. **`junoclaw-miner` crate** — skeleton with `TruthEvaluator` trait + `RuleBasedEvaluator`
2. **MCAP reader** — basic ROS 2 bag parsing in Rust
3. **Miner CLI** — `junoclaw-miner register --stake 1000000 --fingerprint "rule-v1-rust"`
4. **Miner run loop** — watch for batches, submit verdicts, log rewards/slashing

This is the missing piece between our built contracts and a live truth market. The contract is the market. The miner is the participant. Without miners, the market is empty — which is exactly what we see on uni-7 today (0 operators).
