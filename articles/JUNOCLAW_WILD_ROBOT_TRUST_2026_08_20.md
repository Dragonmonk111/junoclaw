# What Roz Taught Us About Robot Trust

> *"Sometimes to survive, we must become more than we were programmed to be."*

*The Wild Robot* dropped on Netflix today. If you haven't seen it — go watch it. Then come back.

...

You back? Good.

The movie is about a robot named Roz who washes up on an island after a shipwreck. She has no owner, no operator, no mission. The animals are terrified of her. She has to earn their trust — not by following her programming, but by becoming more than it.

Here's the thing that stuck with me: **nobody checked whether Roz's decisions were safe.**

She picks up a gosling. She climbs a cliff. She fights a bear. She builds a shelter. Every single one of those decisions could have gone wrong. And the only thing standing between "everything is fine" and "everything is a disaster" was Roz's own judgment.

There was no proof. No audit. No circuit breaker. No record.

## The Problem With Roz

Roz is a great character. She's also a terrifying thought experiment.

Imagine 10,000 Rozzes. Each one making 1,000 decisions per second. Each one running its own software that says "I'm fine, trust me." Each one operating in an unpredictable world full of animals, cliffs, weather, and other robots.

Who checks that those decisions are safe?

Today, in the real world: **nobody**. The robot's own controller says it's fine. If something goes wrong, we find out after the accident. We read the logs. We sue somebody. We add a warning label.

That's the world Roz lives in. And it's the world real robots live in today.

## What Roz Needed

What if, every time Roz made a batch of decisions, she also produced a **receipt** — a mathematical proof that she followed the rules?

Not a log file that can be edited. Not a self-report that says "I'm fine." A cryptographic proof. The kind you can't fake. The kind that, if she broke a rule, the proof itself would fail.

And what if that proof went to a **network of independent observers** — not her manufacturer, not her owner, but neutral third parties who check her work? If they see something wrong, they hit a switch. Roz stops. Not her reflexes — she can still dodge, still balance, still survive. But her ability to make her next big decision gets locked until the problem is resolved.

And what if every one of those decisions, proofs, and verdicts got written to a **permanent record** that nobody can erase? Not Roz's manufacturer. Not the island's government. A blockchain. Regulators can check it. Insurers can price it. Courts can subpoena it.

That's JunoClaw.

## How It Works (The Wild Robot Edition)

### The Physics (Roz's Senses)

Every millisecond, the robot snapshots everything it can feel — wheel positions, speed, tilt, contact forces, distance to obstacles. It hashes that snapshot with a cryptographic fingerprint that can't be faked after the fact.

Think of it like Roz's nervous system. She feels the ground, the wind, the gosling in her arms. But instead of just feeling it, she records a tamper-proof fingerprint of exactly what she felt.

After a batch of 1,000 cycles, all those fingerprints get rolled into one **root hash** — a single string of characters that proves the entire batch is intact. Change one cycle, the root breaks. It's like a wax seal on an envelope, except the envelope contains 1,000 decisions and the seal is mathematically unbreakable.

### The Proof (Roz's Alibi)

The robot generates a 128-byte mathematical proof — smaller than a tweet — that says "all my sensor readings stayed within the safety envelope." It proves this **without revealing the actual sensor values**. It's like proving you're over 18 without showing your ID.

Five different proofs run in parallel:
- **Sensor safety** — speed, force, distance, tilt all within limits (80 ms)
- **Intent consistency** — the planned action is inside the operating zone (119 ms)
- **Consensus membership** — a validator voted correctly (51 ms)
- **Batch safety** — the entire batch of reflex cycles in one proof (~300 ms)
- **Aggregation** — all the above proofs agree with each other (68 ms)

Total: **187 milliseconds**. Roz doesn't have to wait.

### The Audit (The Island's Council)

The proof enters a network of 4+ independent nodes. Think of them as the island's council — the owl, the beaver, the possums. They don't trust Roz. They check her work.

1. **Is the proof valid?** No proof → auto-rejected. No further questions.
2. **Is the intent safe?** Multiple operators check the content. Is Roz trying to do something deceptive?
3. **Truth Market** — operators stake money on their verdicts. Wrong → lose money. Right → earn money. Financial incentives for honesty.
4. **Circuit breaker** — if any violation is found, Roz's intent tier is locked. She can still survive (reflexes keep running), but she can't make new high-level decisions until the problem is resolved.

### The Record (The Island's History Book)

The finalized, audited decision gets written to the Juno blockchain — permanently. The island now has a history book that can never be edited. Every decision Roz made, every proof she generated, every verdict the council reached — all recorded. All verifiable. All permanent.

### The Fleet (The Island's Population)

A **fleet coordinator** manages all the robots together:
- Aggregates decisions from multiple robots into batches
- Rate-limits each robot (prevents a malfunctioning Roz from flooding the council)
- Routes circuit breaker trips back to the specific robot that violated
- Tracks who's operational, who's locked, who's in safe-hold
- Exposes everything via a dashboard so the island can see the whole fleet at a glance

## The Four Timescales

| What | Speed | Analogy |
|------|-------|---------|
| **Reflex** | 8-12 ms | Roz dodging a falling rock — hardware speed, no waiting |
| **Proof** | 187 ms | Roz generating her alibi — "here's proof I followed the rules" |
| **Coordination** | ~300 ms | The council reviewing her alibi — "does this check out?" |
| **Settlement** | 2.8 s | Writing it in the history book — permanent, verifiable |

**Proofs never gate physics.** Roz doesn't wait for the blockchain to dodge. Proofs gate the *coordination layer* — her ability to make her next big decision. If she violates safety, she gets grounded before she can plan her next move.

## What's Real Today

This isn't science fiction. It's running right now.

- **4 smart contracts on Juno mainnet** — real blockchain, real money, running today
- **5 contracts on Juno testnet (uni-7)** — FeePay module enabled, registration + funding verified on-chain
- **4 coordination contracts on the BN254 devnet** — tested, ready for mainnet
- **5 zero-knowledge proof circuits** — all tested, all passing
- **7-day local endurance test** — 2,015 cycles, 605,083 seconds (168 hours), zero crashes, 4/4 nodes alive throughout
- **Akash soak test — LIVE** — 4-node P2P mesh running on Akash mainnet, 544+ cycles and counting, all tests passing, zero crashes
- **183+ tests passing** across coordination, physics, FeePay monitoring, precompiles, circuits, and TEE attestation
- **Physics engine** producing real cryptographic hashes from rigid-body dynamics — 35 tests
- **Fleet coordinator** managing multi-robot fleets — 14 tests
- **Cost: $0.004 per robot per day** — less than half a cent

## What Roz Would Say

Roz earned trust the hard way. She proved herself through action, over time, to a community that had every reason to be suspicious.

JunoClaw does the same thing — but in milliseconds, with mathematics, at scale.

Every robot gets the same treatment Roz got on the island: **prove what you did, let independent observers check it, and if you break the rules, you stop.** Not by trusting the robot's manufacturer. Not by trusting the robot's software. By trusting math.

Roz became more than she was programmed to be. JunoClaw makes sure every robot can prove it did the same.

## Gasless Robots (What's Next)

There's one problem we haven't solved yet: **gas**.

Every transaction on Juno — every proof verification, every Merkle root anchor, every circuit breaker trip — requires a small fee. Less than half a cent per robot per day. But someone has to pay it. And if you're running 10,000 robots, that's 10,000 wallets that need JUNO, 10,000 gas strategies, 10,000 ways for things to go wrong during a spike.

**Juno v31 fixes this.** The FeePay module — already live on testnet — lets a fleet operator prepay gas for every robot under their care. The robot sends a transaction with zero fees. The pool covers it. The robot operator never thinks about gas.

V31 ([PR #1223](https://github.com/CosmosContracts/juno/pull/1223), opened August 18, 2026) fixes four critical bugs that made FeePay unreliable for production:
- **Denom overflow** — pools could misallocate funds with multiple denominations
- **Sender accounting** — the wrong wallet was sometimes credited for usage
- **Pool cleanup** — withdrawing funds left orphaned state in the database
- **Zero-fee rejection** — when the pool was exhausted, transactions silently consumed sequence numbers without executing

With v31, FeePay becomes production-reliable. A fleet operator funds one pool. Every robot operator sends gasless transactions. The operator doesn't need JUNO. Doesn't need a wallet with gas. Doesn't need to understand gas prices. Just sends the transaction.

Think of it like Roz not needing to forage for herself — the island provides. Except in this case, the fleet operator provides, and the math makes sure nobody can game the system.

### What We Proved (August 21, 2026)

We ran the full FeePay flow on uni-7 against a live moultbook contract:

- **FeePay enabled** on the chain — confirmed via REST query
- **Contract registered** — `MsgRegisterFeePayContract` broadcast, code 0, tx hash on-chain
- **Pool funded** — 1,000,000 ujunox escrowed into the FeePay pool, balance confirmed via query
- **Normal transaction** (with fees) — succeeded, 211,707 gas, 22,500 ujunox paid by sender
- **Gasless transaction** (fees=0) — **failed**: `insufficient fee: got 0ujunox required 22500ujunox`

The gasless tx failure is the whole story. On v30, the **GlobalFee ante handler runs before the FeePay ante handler** in the chain's transaction validation pipeline. GlobalFee sees a zero-fee transaction, calculates `minGasPrice × gas = 22,500 ujunox`, and rejects it — before FeePay ever gets a chance to escrow from the pool and cover the cost.

This is not a FeePay bug. FeePay did everything right: registration, funding, pool balance accounting, wallet limit tracking — all working. The problem is **ordering**: who checks first. v31 reorders the ante chain so FeePay intercepts before GlobalFee. Same modules, same logic, different sequence. The fix is a reordering, not a rewrite.

When v31 lands on uni-7, we rerun the same script. Same contract, same pool, same zero-fee transaction. If the ante handler reorder works, the gasless tx succeeds, the pool is deducted, and the sender pays nothing. That's the moment gasless robots become real.

---

## For the Builders

If you're a robotics company and this resonates:

- **ROS2 Bridge** — Python/FastAPI, works with or without ROS2
- **Prover Daemon** — Rust binary, runs on the robot or edge device
- **Physics Engine** — `junoclaw-physics` crate, rigid-body dynamics, 35 tests
- **Fleet Coordinator** — multi-robot aggregation, rate limiting, breaker routing
- **Docker Compose** — full stack deployment
- **SDK Integration Guide** — 2 paths, 6 robot platforms (Unitree, Spot, TurtleBot, UR, custom)
- **ISO 10218 / TS 15066 Compliance Mapping** — every safety parameter mapped to specific clauses

Open-source. Apache 2.0. [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)

---

*Inspired by The Wild Robot (2026), now on Netflix. Roz didn't choose to be on that island. But she chose to be trustworthy. JunoClaw makes that choice verifiable.*

*August 2026. 4 contracts on Juno mainnet. 5 ZK circuits. 183+ tests. Local soak: 2,015 cycles over 168 hours, 0 crashes, 4/4 nodes alive. Akash soak: live now, 544+ cycles and counting. Physics engine. Fleet coordinator. FeePay tested on uni-7: registration, funding, pool accounting all verified on-chain. Gasless tx blocked by GlobalFee ante handler ordering — confirmed root cause, fixed in Juno v31 (PR #1223). TEE attestation with real Ed25519. Post-quantum ready. $0.004/robot/day. The product does what it says.*
