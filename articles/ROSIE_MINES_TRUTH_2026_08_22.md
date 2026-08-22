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
- **Config governance** — admin can adjust min stake, slash %, reward %, cooldown, min operators

As of today, the contract has **zero registered operators** — because the miner software didn't exist. Now it does.

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

---

## What's Next

- **Register the first operators** — the contract is live, the miner software is built. Someone needs to stake and start mining.
- **Native MCAP parsing** — add the `mcap` Rust crate for direct .mcap file reading
- **McapEvaluator** — verdict from telemetry data directly, no LLM needed
- **Fingerprint diversity enforcement** — relayer rejects correlated miners
- **Reward pool funding** — who funds the rewards? DAO treasury? Protocol fees? Robot manufacturers?
- **Cross-chain IBC verdicts** — miners on Juno evaluate robots on other chains
- **MCAP → IPFS → on-chain archive** — permanent telemetry storage

---

## The Bigger Picture

The truth market is what makes JunoClaw different from every other robot safety system. Traditional approaches rely on the robot's own software saying "I'm fine." JunoClaw adds cryptographic proofs (ZK), independent verification (truth market), and permanent records (blockchain).

The truth market is the layer where **the crowd checks the robot's work**. Not a single auditor. Not a government regulator. A decentralized market of independent operators with financial incentives to be right.

And now, the software to participate in that market exists. A robot in Tokyo can mine truth for a robot in São Paulo. A GPU rig in Berlin can earn rewards for verifying surgical robot decisions in Mumbai. An Akash TEE deployment can provide verifiable computation without owning a single piece of hardware.

**Rosie mines truth after housekeeping. And her owner wakes up richer.**

---

*August 22, 2026. `junoclaw-miner` crate built — 6 modules, 3 tests passing. Truth market contract live on uni-7 since Aug 17. Frontend MinerPanel live with 15-second polling. Three miner types: Robot (Jetson Orin), GPU (bare-metal), Akash TEE. Open-weight models only — closed APIs can't be verified. Batch compression: 70× at 100 batches, 700× at 1,000. MCAP telemetry reader built. On-chain verdict submission wired via cosmos-mcp CLI. The truth market has zero operators today. Tomorrow, it won't.*

---

*Related: [Full Stack Product Picture](JUNOCLAW_FULL_STACK_MELANGE_2026_08_19.md) · [What Roz Taught Us](JUNOCLAW_WILD_ROBOT_TRUST_2026_08_20.md) · [Robot Scaling Ages](ROBOT_SCALING_AGES_2026_08_19.md) · [Gasless Robots](JUNOCLAW_V31_GASLESS_ROBOTS_2026_08_20.md) · [FeePay Tested on uni-7](FEEPAY_TESTED_ON_UNI7_2026_08_21.md)*
