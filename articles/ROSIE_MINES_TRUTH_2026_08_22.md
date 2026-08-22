# Rosie Mines Truth: How Idle Robots Earn Rewards

> *"Sometimes to survive, we must become more than we were programmed to be."* — Roz, *The Wild Robot*

*Picture this: It's 2 AM. A delivery robot in Tokyo sits in its charging dock, wheels still, lights dim. It's done for the day — packages delivered, sidewalks navigated, pedestrians dodged. But its Jetson Orin isn't sleeping. It's mining truth.*

*On the other side of the planet, a surgical assistant robot in São Paulo is about to make a critical decision. Before that decision is finalized, it needs independent verification — someone to check its work. Not its manufacturer. Not its owner. A neutral third party with no skin in the game except a stake they're willing to lose if they're wrong.*

*The Tokyo robot's Orin spins up Qwen-3B, reads the São Paulo batch, and submits a verdict: green. Safe to proceed. For this, the Tokyo robot's owner earns a micro-reward. By morning, the charging dock has generated income overnight — not from delivering packages, but from evaluating other robots' decisions.*

*This is the Truth Market. And it's built.*

---

## The Missing Piece

JunoClaw has had a truth market contract on Juno testnet since August 17, 2026. It handles staking, verdict submission, epoch finalization, rewards, and slashing. It supports self-reported fingerprints for diversity detection. It enforces a minimum operator count. It's 727 lines of CosmWasm Rust, deployed and queryable.

What it didn't have was **miners**. The contract was an empty marketplace — stalls set up, registers ready, but nobody selling. No software existed to watch the coordination network, evaluate robot batches, and submit verdicts.

That's now fixed. The `junoclaw-miner` crate is built, tested, and wired into the workspace.

---

## The Jetsons Vision

Remember Rosie from *The Jetsons*? She finishes housekeeping, sits down in her charging dock, and instead of going idle — she mines truth. Her Jetson Orin (3B model, 15 tok/s, 30W) evaluates batches from robots on the other side of the planet. She earns rewards for correct verdicts. Her owner wakes up to a micro-income stream generated overnight.

Every robot is a potential miner. Every GPU is a potential miner. The truth market doesn't care if you're a Jetson Orin on a vacuum robot, a 4×DGX Spark in someone's garage, or an H100 in a data center. You stake, you evaluate, you earn.

**A robot sleeping in one corner of the world mines truth for a robot working in another corner of the world.**

This is the vision: robot idle time becomes productive. Not by mining Bitcoin (wasteful proof-of-work), but by mining truth (useful proof-of-evaluation). Every robot with spare compute becomes a node in a global safety verification network.

---

## Three Types of Miners

### 1. Robot Miner (Jetson Orin)

A robot mines truth during idle time. After housekeeping, after delivery runs, after surgery — the robot's edge compute switches from task execution to batch evaluation. The Jetson Orin runs a 3B-parameter open-weight model at 15 tokens/second on 30 watts. That's enough to read a batch summary (proof result, gate verdict, safety envelope, intent) and produce a green/yellow/red verdict.

```bash
junoclaw-miner run --evaluator local \
  --llm-endpoint http://localhost:11434 \
  --llm-model qwen-3b \
  --identity-type robot \
  --hardware jetson-orin
```

The robot uses its `jclaw-credential` (soulbound, non-transferable) as its miner identity. This binds the miner to a specific, verified robot — not an anonymous wallet.

### 2. GPU Miner (Bare-Metal)

Anyone with a GPU rig can mine truth. Stake JUNO, run an open-weight model (Llama-70B, Mistral-8x22B, DeepSeek-V3), evaluate batches, earn rewards. Like Bitcoin mining — but instead of wasting energy on meaningless hashes, you're verifying that robots followed safety rules.

```bash
junoclaw-miner run --evaluator local \
  --llm-endpoint http://localhost:8080 \
  --llm-model llama-70b \
  --identity-type gpu \
  --hardware dgx-spark
```

### 3. Akash TEE Miner

Akash now has confidential computing (TEE) features. An open-weight model running in a Trusted Execution Environment on Akash provides verifiable computation without owning hardware. The TEE attests to the exact model and inference that ran inside the enclave — proving that the model you claim to run is the model that actually ran.

This means anyone can deploy a truth miner on Akash with TEE attestation. No bare-metal GPU required. Rent an H100 with confidential computing, deploy Mistral-8x22B, and start mining.

```bash
junoclaw-miner run --evaluator akash-tee \
  --llm-endpoint https://akash-deploy.example.com \
  --llm-api-key ak-... \
  --llm-model mistral-8x22b \
  --identity-type akash-tee \
  --hardware akash-h100-tee
```

---

## Open-Weight Models Only

This is a fundamental design principle. Only open-weight models qualify as J-Lens miners.

Closed-weight API models (GPT-4o, Claude, Gemini) **cannot be verified**. When you call GPT-4o's API, you have no way to prove:
- What model actually ran
- That it ran faithfully without modification
- That the response wasn't filtered or modified server-side
- That the same model would produce the same output on different hardware

Open-weight models (Llama, Qwen, Mistral, DeepSeek) running on hardware the miner controls are verifiable. The miner can prove:
- The exact model weights (via hash)
- The hardware it ran on (via fingerprint)
- The inference was faithful (via TEE attestation if on Akash, or via local execution)

This is why the `junoclaw-miner` crate uses `OpenWeightEvaluator` — not an `LlmEvaluator` that calls arbitrary APIs. The architecture enforces verifiability from the ground up.

---

## How It Works

### The Mining Loop

1. **Watch**: The miner polls the coordination REST API for finalized batches
2. **Evaluate**: Each batch is passed to the evaluator (rule-based, open-weight LLM, or MCAP telemetry-based)
3. **Verdict**: The evaluator returns green (safe), yellow (suspicious), or red (unsafe)
4. **Submit**: The verdict is signed and broadcast to the truth market contract on Juno
5. **Earn**: When the epoch finalizes, miners who matched consensus earn rewards. Miners who diverged get slashed.

### The Evaluator

The `TruthEvaluator` trait is pluggable:

- **`RuleBasedEvaluator`** — deterministic rules for testing and baseline. No proof → Red. Proof not verified → Red. Gate verdict = red → Red. Separation score > 0.35 → Red. Otherwise → Green.
- **`OpenWeightEvaluator`** — calls a local OpenAI-compatible inference server (vLLM, Ollama, llama.cpp, text-generation-inference) running an open-weight model. The model reads the batch data and returns a verdict.
- **McapEvaluator** (planned) — reads MCAP telemetry files directly and evaluates sensor data against the safety envelope. No LLM needed — pure data verification.

### The Fingerprint

Every miner has a fingerprint: a hash of model ID + hardware ID + weight type + optional credential/TEE attestation. This fingerprint is submitted to the truth market contract on registration. Relayers use fingerprints to detect correlated miners — if 10 miners all report the same fingerprint, they're probably running the same model on the same hardware, and their verdicts shouldn't be treated as independent.

Diversity matters. The truth market works best when miners are genuinely independent: different models, different hardware, different geographic regions. A Jetson Orin running Qwen-3B in Tokyo and a DGX Spark running Llama-70B in Berlin are independent. Two cloud instances of the same API are not.

---

## Compression: Thousands of Decisions in One Transaction

Here's where Commonware's Bajillion protocol inspired us.

A robot makes 1,000 decisions per second. Over an epoch (say, 100 batches), that's 100,000 decisions. Settling each one individually on-chain would be absurd — 100,000 transactions, each costing gas, each taking 2.8 seconds to finalize.

Instead, we use a Merkle tree to compress all batch hashes in an epoch into a single root commitment. Here's how it works:

1. Each batch has a `messages_hash` (already computed by the coordination layer)
2. All batch hashes in an epoch become leaves of a Merkle tree
3. The Merkle root is a single 32-byte commitment that proves all batches are intact
4. Only the root goes on-chain — ~144 bytes total (root + epoch number + batch count + BLS signature + signer bitmap)

### The Math

| Batches | Individual Settlement | Compressed | Ratio |
|---------|----------------------|------------|-------|
| 10 | ~1,000 bytes | ~144 bytes | 7× |
| 100 | ~10,000 bytes | ~144 bytes | 70× |
| 1,000 | ~100,000 bytes | ~144 bytes | 700× |
| 10,000 | ~1,000,000 bytes | ~144 bytes | 7,000× |

At scale, thousands of robot safety decisions settle in a single on-chain transaction. If any individual batch is disputed, a Merkle proof can challenge it — proving that batch X was part of the committed epoch without revealing all other batches.

This is the same root-of-roots pattern from Commonware Bajillion: compress many items into one commitment, settle the commitment, allow individual challenges via proof. We just applied it to robot decisions instead of account states.

The `batch_compression` module is built and tested — 3/3 tests pass (compression ratio, Merkle proof verification, empty epoch edge case).

---

## MCAP: The Robot Telemetry Standard

ROS 2 replaced SQLite3 with MCAP as its default storage format. MCAP is:
- **Indexed** — random access to any message without scanning the whole file
- **Compressed** — Zstd/LZ4 built-in, 3-10× smaller than SQLite3
- **Cloud-native** — designed for streaming, not local filesystem
- **Foxglove-compatible** — visualizes in Foxglove Studio out of the box

The `junoclaw-miner` crate includes an MCAP reader module that extracts sensor readings (IMU, joint states, contact events) and computes safety-relevant metrics (max speed, max force, min distance, max tilt). The miner can use this telemetry to evaluate robot decisions with full context — not just the ZK proof summary, but the actual sensor data that went into it.

Currently the reader supports pre-extracted JSON (which the prover daemon already outputs). Native MCAP parsing will be added with the `mcap` Rust crate dependency.

---

## The Truth Market Contract (Already Live)

The contract has been on uni-7 testnet since August 17, 2026:

**Address**: `juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p`

It implements:
- **Operator registration** — stake JUNO, submit fingerprint, become a miner
- **Verdict submission** — green/yellow/red on each batch
- **Epoch finalization** — relayer calls `finalize_epoch`, contract compares verdicts, distributes rewards to matching operators, slashes diverging ones
- **Unstake/withdraw** — request unstake, wait cooldown, withdraw stake
- **Fingerprint tracking** — self-reported model + hardware hash for diversity detection
- **Config governance** — admin can adjust min stake, slash %, reward %, cooldown, min operators, verification fee
- **Verification fee payment** — `PayVerificationFee` message routes robot-paid fees directly into the reward pool
- **Three reward modes** — Equal, StakeWeighted, StakeTimesAccuracy (stake × Laplace-smoothed accuracy)

As of today, the contract has **zero registered operators** — because the miner software didn't exist. Now it does.

---

## Protocol Fee Routing: The Closed Loop

The initial design had a sustainability problem: the reward pool was funded only by deposits. Someone had to keep depositing. DAO grants are finite. Sponsorships end. Without a revenue stream, the pool drains and miners stop earning.

That's now solved. Every time a robot submits a batch for safety verification, the operator pays a **verification fee**. This fee flows directly into the truth market reward pool. The miners who evaluate that batch earn from the pool.

```
Robot submits batch → Pays verification fee → Fee enters REWARD_POOL
                                                         ↓
                                    Miners evaluate batch → Earn from pool
                                    (weighted by stake × accuracy)
```

### The Contract Interface

| Message | Caller | Purpose |
|---------|--------|---------|
| `PayVerificationFee { batch_height, robot_id }` | Robot operator or relayer | Per-batch fee → reward pool |
| `DepositRewards {}` | Anyone (DAO, grants, sponsors) | Bulk deposit into pool |
| `UpdateConfig { verification_fee: Some(amount) }` | Admin | Set per-batch fee (0 = open access) |

### Fee Modes

- **`verification_fee = 0`**: Open access — anyone can submit batches, pool funded by grants only. Good for bootstrapping.
- **`verification_fee > 0`**: Fee-enforced — `PayVerificationFee` requires exactly the configured amount. Wrong amount = rejected. Good for production.

### Relayer Integration

The relayer daemon now calls `PayVerificationFee` **before** finalizing each epoch. The flow:

1. Batch settled on coordination-settler contract
2. Relayer calls `PayVerificationFee` on truth-market contract (funds the pool)
3. Relayer calls `FinalizeEpoch` on truth-market contract (distributes rewards)

CLI flag `--verification-fee` controls the per-batch fee amount. Set to 0 to skip fee payment (open access mode).

### Three Revenue Streams for Miners

| Stream | Source | Status |
|--------|--------|--------|
| **Verification fees** | Robot operators pay per batch | ✅ Implemented |
| **Grant deposits** | DAO treasury, sponsors, insurers | ✅ Implemented |
| **Slashed stake** | Diverging operators' slashed funds return to pool | ✅ Implemented |

The third stream is elegant: when a miner submits a wrong verdict, they get slashed. The slashed amount goes **back into the reward pool**. Bad miners fund good miners. This creates a negative feedback loop — as accuracy drops, slashing increases, which increases the pool for accurate miners, which attracts more miners, which increases accuracy.

### Why Not Inflation?

This is the key distinction from Bitcoin mining:

```
Bitcoin:           Chain mints BTC → miners earn → inflation dilutes all holders
Juno validators:   Chain mints JUNO → validators earn → inflation dilutes all holders
Truth market:      Robot pays existing JUNO → miners earn → NO inflation, NO dilution
```

The truth market is a **closed-loop bounty system**, not an inflationary mining scheme. The JUNO that robots pay as verification fees is the same JUNO that miners receive as rewards. It circulates. No new tokens are created.

### FeePay: Gasless Robots

Robot operators shouldn't need to manage gas. With Juno's FeePay module, a robot can submit a `PayVerificationFee` transaction with **zero gas fees** — FeePay sponsors the gas from a separate pool. The verification fee itself still flows to the reward pool. The robot operator never touches JUNO for gas.

**Status**: FeePay is live on uni-7 testnet. Gasless tx flow requires v31's ante handler reordering (FeePay before GlobalFee). The contract integration is ready; the chain-level fix is pending.

---

## The Incentive: Why Would Anyone Mine?

The obvious question: if you stake JUNO and get the verdict right, you earn rewards. If you get it wrong, you lose part of your stake. So what's the profit?

**The stake is not consumed. It's locked, not spent.**

```
You stake 1M JUNO  →  locked (you get it ALL back when you unstake, minus any slashing)
You run inference   →  costs ~30W of electricity (trivial on a robot)
You get it right    →  earn rewards from the pool (verification fees paid by robots)
You get it wrong    →  lose slash_percent (10%) of your stake → goes to reward pool
```

### The Math

| Scenario | What happens | Net |
|----------|-------------|-----|
| **Right verdict** | Earn share of reward pool | **+rewards** (pure profit) |
| **Wrong verdict** | Lose 10% of stake, earn nothing | **-100K JUNO** (stake slashed) |
| **Unstake (anytime)** | Get full stake back (minus prior slashes) | **stake returned** |

The rewards come from the **verification fees robots pay**, not from your stake. Your stake is collateral — skin in the game to prevent Sybiling. It's like a security deposit on an apartment: you get it back if you don't trash the place.

### Sustainability Math

Assume:
- 1,000 robots globally, each submitting 10 batches/day
- Verification fee: 50,000 ujunox (~0.05 JUNO) per batch
- 10,000 batches/day × 50,000 ujunox = 500M ujunox/day into reward pool
- ~500 JUNO/day distributed to miners

A Jetson Orin miner with 1M stake and 100% accuracy, competing against 99 similar miners, would earn ~5 JUNO/day. At 30W power consumption, that's profitable in any energy market.

At 10,000 robots (still small scale): ~5,000 JUNO/day into the pool. The truth market becomes a meaningful income stream for idle GPU operators worldwide.

---

## The Model IS the Mining Rig

In Bitcoin:
```
ASIC hardware → runs SHA-256 → mines hashes → earns BTC
```

In JunoClaw:
```
GPU hardware → runs open-weights model → mines truth verdicts → earns JUNO
```

The model is not a separate cost — it **is** the mining algorithm. Qwen-3B is the "SHA-256" of truth mining. The difference is that Qwen-3B produces something useful (a safety verdict) instead of a useless hash.

### Complete Miner Cost Breakdown

**CapEx (one-time):**

| Cost | Jetson Orin (robot) | DGX Spark (GPU) | Akash TEE (cloud) |
|------|---------------------|-----------------|-------------------|
| Hardware | ~$250 (already on robot) | ~$4,000 | $0 (rented) |
| Model weights | Free (open-weight) | Free (open-weight) | Free (open-weight) |
| Stake (locked, returned) | 1M JUNO | 5M JUNO | 2M JUNO |

**OpEx (per day):**

| Cost | Jetson Orin | DGX Spark | Akash TEE |
|------|------------|-----------|-----------|
| Electricity | 30W → ~$0.05 | 2,400W → ~$4.00 | included in rent |
| Akash rent | — | — | ~$5-20/day (H100) |
| Stake opportunity cost | ~274 JUNO/day foregone | ~1,370 JUNO/day foregone | ~548 JUNO/day foregone |
| Inference time per batch | 15 tok/s → ~30s | 200 tok/s → ~3s | 500 tok/s → ~1s |

### Three Tiers of Profitability

| Tier | Hardware | Opportunity Cost | Profitability |
|------|----------|-----------------|---------------|
| **Robot (idle mining)** | Already owned, already powered | Near-zero (GPU was idle) | **Most profitable** — pure upside |
| **Dedicated GPU miner** | Bought for mining | High (GPU could do other work) | Profitable if fees > GPU rental rates |
| **Akash TEE miner** | Rented | Medium (rent is the cost) | Profitable if fees > Akash rent |

The system is self-balancing. The robot tier guarantees a baseline of miners (because it's nearly free). The dedicated GPU tier scales up when fees are high enough. The Akash tier provides TEE-attested verification for high-value use cases.

### The Robot Advantage: Zero Opportunity Cost

A Jetson Orin on a delivery robot has **zero opportunity cost**:
- The hardware is already paid for (it's on the robot)
- The GPU is idle during charging (not doing anything else)
- The electricity is already being consumed (charging the battery)
- The stake is the only real cost

```
Delivery robot economics:
  Day:   delivers packages (revenue from deliveries)
  Night: mines truth (revenue from verification fees)
  Same hardware. Same electricity. Additional revenue stream.
```

This is the **free money** tier. The robot is already built, already charging, already has a GPU. Truth mining is pure upside.

### Bitcoin vs Truth Market Mining

| Factor | Bitcoin Mining | Truth Market Mining |
|--------|---------------|-------------------|
| Hardware | ASIC (single-purpose, e-waste when unprofitable) | GPU (general-purpose, broad resale value) |
| Algorithm | SHA-256 (free, hardcoded) | Open-weights model (free, swappable) |
| Work output | Hash collision (discarded) | Safety verdict (used by robots) |
| Scale economics | Bigger farm = more hashes | Bigger model = better accuracy = more rewards |
| Moore's law | ASICs obsolete fast | Models improve → better accuracy → more rewards |

### Self-Regulating Market

- If verification fees are too low → miners leave → fewer miners → safety verification degrades → robots pay higher fees to attract miners back
- If verification fees are high → miners join → more miners → more competition → fees stabilize

**The market finds equilibrium.** Just like Bitcoin difficulty adjustment, but through fee economics instead of hash difficulty.

---

## The Frontend

A new "Miners" tab in the JunoClaw dashboard shows:
- Live truth market stats (operators, staked amount, reward pool, epochs finalized)
- Contract configuration (min stake, slash %, reward %, cooldown, min operators)
- Expandable operator list with accuracy, rewards, slashing, and fingerprints
- Mining guide with CLI examples for all three miner types
- Fingerprint diversity visualization
- "Open-weight only" warning explaining why closed APIs don't qualify

All data is polled live from the truth market contract on uni-7 via CosmWasm queries. 15-second refresh interval.

---

## What's Built

| Component | Status | Tests |
|-----------|--------|-------|
| `junoclaw-miner` crate | Built, compiles, wired into workspace | 3/3 |
| `TruthEvaluator` trait + `RuleBasedEvaluator` | Built | — |
| `OpenWeightEvaluator` (local + Akash TEE) | Built | — |
| `MinerIdentity` (Robot, GPU, Akash TEE) | Built | — |
| `ModelWeightType` (OpenWeight, OpenWeightTee) | Built | — |
| MCAP telemetry reader | Built (JSON fallback, native pending) | — |
| Batch compression (Merkle root + proofs) | Built | 3/3 |
| Miner run loop (poll, evaluate, submit) | Built | — |
| On-chain verdict submission (cosmos-mcp CLI) | Wired | — |
| CLI (register, run, status, unstake, identity) | Built | — |
| Frontend MinerPanel + live queries | Built, TypeScript clean | — |
| Truth market contract on uni-7 | Live since Aug 17 | — |
| `RewardMode::Equal` | ✅ Live | ✅ |
| `RewardMode::StakeWeighted` | ✅ Live | ✅ |
| `RewardMode::StakeTimesAccuracy` | ✅ Live | ✅ |
| `PayVerificationFee` message | ✅ Live | ✅ |
| `verification_fee` config field | ✅ Live | ✅ |
| `DepositRewards` (anyone can fund pool) | ✅ Live | ✅ |
| Slashing → reward pool | ✅ Live | ✅ |
| Relayer fee routing (`pay_verification_fee`) | ✅ Built | ✅ |
| CLI `--verification-fee` flag | ✅ Built | ✅ |
| Frontend verification fee display | ✅ Live | — |
| FeePay gasless fees | ⏳ Pending v31 | — |
| Truth market contract tests | 21/21 pass | ✅ |
| Relayer tests | 14/14 pass | ✅ |

---

## What's Next

- **Register the first operators** — the contract is live, the miner software is built. Someone needs to stake and start mining.
- **Native MCAP parsing** — add the `mcap` Rust crate for direct .mcap file reading
- **McapEvaluator** — verdict from telemetry data directly, no LLM needed
- **Fingerprint diversity enforcement** — relayer rejects correlated miners
- **FeePay v31 integration** — gasless `PayVerificationFee` for robot operators (pending ante handler reorder)
- **Cross-chain IBC verdicts** — miners on Juno evaluate robots on other chains
- **MCAP → IPFS → on-chain archive** — permanent telemetry storage

---

## The Bigger Picture

The truth market is what makes JunoClaw different from every other robot safety system. Traditional approaches rely on the robot's own software saying "I'm fine." JunoClaw adds cryptographic proofs (ZK), independent verification (truth market), and permanent records (blockchain).

The truth market is the layer where **the crowd checks the robot's work**. Not a single auditor. Not a government regulator. A decentralized market of independent operators with financial incentives to be right.

And now, the software to participate in that market exists. A robot in Tokyo can mine truth for a robot in São Paulo. A GPU rig in Berlin can earn rewards for verifying surgical robot decisions in Mumbai. An Akash TEE deployment can provide verifiable computation without owning a single piece of hardware.

The funding is solved: robots pay verification fees, fees flow to the reward pool, miners earn from the pool. No inflation. No grants needed. A closed loop where the robots being verified fund the miners verifying them.

**Rosie mines truth after housekeeping. And her owner wakes up richer.**

---

*August 22, 2026. `junoclaw-miner` crate built — 6 modules, 3 tests passing. Truth market contract live on uni-7 since Aug 17, 21/21 tests passing. Protocol fee routing implemented — `PayVerificationFee` routes robot fees into the reward pool. Relayer calls fee payment before epoch finalization. Three reward modes: Equal, StakeWeighted, StakeTimesAccuracy. Three miner types: Robot (Jetson Orin, zero opportunity cost), GPU (bare-metal), Akash TEE. Open-weight models only — closed APIs can't be verified. Batch compression: 70× at 100 batches, 700× at 1,000. MCAP telemetry reader built. On-chain verdict submission wired via cosmos-mcp CLI. Relayer fee routing wired via `--verification-fee` CLI flag. 14/14 relayer tests passing. The truth market has zero operators today. Tomorrow, it won't.*

---

*Related: [Full Stack Product Picture](JUNOCLAW_FULL_STACK_MELANGE_2026_08_19.md) · [What Roz Taught Us](JUNOCLAW_WILD_ROBOT_TRUST_2026_08_20.md) · [Robot Scaling Ages](ROBOT_SCALING_AGES_2026_08_19.md) · [Gasless Robots](JUNOCLAW_V31_GASLESS_ROBOTS_2026_08_20.md) · [FeePay Tested on uni-7](FEEPAY_TESTED_ON_UNI7_2026_08_21.md)*
