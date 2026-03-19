# JunoClaw Revives Junoswap, Ships DAO Governance, and Prepares a Juno Proposal

### The whole picture — what it is, what it does, and why it matters. Explained so anyone can follow.

> *"Juno is going to be run by an AI soon."* — Jake Hartnell, Juno co-founder, March 17, 2026 (Juno Telegram, 4581 members)

---

## What Just Happened

In the last 48 hours, three things went live on Juno testnet that haven't existed together before on any Cosmos chain:

1. **Junoswap v2** — the token exchange that made Juno famous — was rewritten from scratch, deployed, and wired into a DAO contract that verifies every swap with hardware it can't lie about.

2. **A governance system** was upgraded so that code changes to the network require a **supermajority vote** (67%, not 51%). This is the same threshold Cosmos SDK uses for chain upgrades. It's now enforced at the smart contract level.

3. **A single governance proposal** — Proposal 5 — bundled the Junoswap wiring, the WAVS verification update, and the Akash compute deployment into one atomic action. It passed, it executed, and the infrastructure is live.

Jake Hartnell, co-founder of Juno and WAVS/Layer.xyz, saw it all happen and said:

> *"Cool, prop is a great way to start discussion. With all the other chains shutting down, it's actually a good chance for Juno to rise again. Especially if we can get a few agents building it out."*

That's the signal. Let's explain what he's looking at.

---

## The Whole Picture — Like You're Five (But With Receipts)

Imagine a company. Not a normal company — a company that lives on the internet and has no office, no CEO, and no bank account. Instead it has:

- **A rulebook** that nobody can secretly change (that's the smart contract)
- **A voting system** where every member's vote has a weight (that's the DAO)
- **A lockbox** that only opens when the rules say so (that's the escrow)
- **A truth machine** that checks homework inside a sealed room nobody can peek into (that's the TEE — Trusted Execution Environment)

That's JunoClaw. Now let's walk through it piece by piece.

---

### The Rulebook (Smart Contracts on Juno)

**What a 5-year-old needs to know:** The rulebook is written in a language called Rust, turned into a tiny program (WebAssembly), and uploaded to a blockchain called Juno. Once it's there, everyone can read it, nobody can secretly change it, and it runs exactly the same way every time.

**What a developer needs to know:**

- **agent-company** (v3, code ID 63) — The core DAO contract. Handles member weights, proposal creation, voting with adaptive block reduction, attestation storage, sortition via drand randomness, and now CodeUpgrade proposals with supermajority quorum. ~470KB optimized WASM.
- **junoswap-factory** (code ID 61) — Manages trading pair creation. Clean rewrite from scratch — not a fork and not affiliated with the original Junoswap team. XYK constant-product model, WAVS hooks built in from day one. Lean JunoClaw-native contracts.
- **junoswap-pair** (code ID 60) — Each trading pair (like JUNOX/USDC) is its own contract. Handles swaps, liquidity provision, and fee collection. 0.30% swap fee.
- **escrow, task-ledger, agent-registry** — Supporting contracts for payment splits, task tracking, and agent identity.

All contracts have `migrate` entry points. All are admin-controlled by the Genesis address during the bootstrap phase. All will transition to DAO governance.

---

### The Voting System (Governance)

**What a 5-year-old needs to know:** Imagine 13 kids in a classroom. Each kid has some stickers (voting weight). To change the classroom rules, they raise their hands. Small changes need just over half the stickers. Big changes — like replacing the teacher — need two-thirds.

**What a developer needs to know:**

JunoClaw's governance has **8 proposal types**:

| Type | What It Does | Quorum |
|------|-------------|--------|
| **WeightChange** | Add/remove members, change voting weights | 51% |
| **WavsPush** | Send a task to WAVS for off-chain verification | 51% |
| **ConfigChange** | Change admin, governance contract | 51% |
| **FreeText** | Signal vote, no on-chain effect | 51% |
| **OutcomeCreate** | Create a verifiable prediction market | 51% |
| **OutcomeResolve** | Resolve a market with WAVS attestation | 51% |
| **SortitionRequest** | Randomly select members (jury duty) | 51% |
| **CodeUpgrade** | Bundle store/instantiate/migrate/execute actions | **67%** |

The CodeUpgrade type is new. It's the one that wired Junoswap into the system. It supports 5 sub-actions:

- **StoreCode** — Upload new WASM (emits event for off-chain relayer)
- **InstantiateContract** — Deploy a new contract from existing code
- **MigrateContract** — Upgrade an existing contract to new code
- **ExecuteContract** — Call any contract method
- **SetDexFactory** — Wire Junoswap's factory address into the DAO config

All 5 can be bundled into a single atomic proposal. Either everything passes or nothing does.

**Adaptive block reduction:** If all members vote within 10 blocks of proposal creation, the deadline shrinks from 100 blocks down to a floor of 13. This means unanimous decisions resolve in ~1 minute instead of ~10.

---

### The Truth Machine (WAVS + TEE)

**What a 5-year-old needs to know:** Imagine a homework-checking robot that sits inside a glass box. You can see what goes in and what comes out, but nobody — not even the person who built the robot — can reach inside and change the answer. That's a TEE. The robot checks your math, stamps it "correct" or "wrong," and hands the stamp back through a slot. The stamp is unforgeable.

**What a developer needs to know:**

WAVS (Wasm AVS, by Layer.xyz) provides verifiable off-chain compute:

1. **Trigger:** The DAO executes a proposal → contract emits a typed event (`wasm-wavs_push`, `wasm-outcome_create`, etc.)
2. **Watch:** The WAVS operator (a daemon polling the chain) detects the event
3. **Compute:** A WASI component runs inside an Intel SGX or AMD SEV enclave. It receives the proposal data, computes a SHA-256 hash, and produces a `VerificationResult`
4. **Attest:** The operator submits the attestation hash to the `agent-company` contract via `SubmitAttestation`
5. **Verify:** Anyone can check: the on-chain attestation hash matches the deterministic computation over the public proposal data

**Proven milestone:** Proposal 4 on uni-7 is the **first hardware-attested WAVS result in the Cosmos ecosystem**. The WASI component ran inside an Intel SGX enclave on Azure DCsv3 (Standard_DC2s_v3, 2 vCPU, 16 GiB, SGX confirmed via `/dev/sgx_enclave`).

```
Attestation TX: 6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22
Block: 11735127
data_hash: 9d0f7354205de1fcaa41a8642ee704ed8e6201bdf8e4951b36923499a7367a3b
attestation_hash: 945a53c5c1aab2e99432e659d47633da491fffc399d95cbce66b8e88fae5c0e8
```

---

### The Exchange (Junoswap v2)

**What a 5-year-old needs to know:** Imagine a lemonade stand where you can trade your apple juice for orange juice. The stand has a rule: for every trade, the truth-machine robot checks that you got the right amount. If the stand tried to cheat you, the robot would catch it. No other lemonade stand in the neighborhood has a checking robot.

**What a developer needs to know:**

Junoswap v2 is a constant-product (x*y=k) AMM:

- **Factory contract** creates and indexes trading pairs
- **Pair contracts** handle individual swaps with the XYK invariant
- **0.30% fee** on every swap, configurable by governance
- **WAVS hooks:** Every swap execution can trigger a verification event. The WAVS operator independently recomputes the expected output and attests correctness. This is **verifiable DeFi** — not just "trust the math," but "trust the math AND the hardware proved the math."

The factory is now wired into agent-company's config (Proposal 5, `SetDexFactory` action). Future proposals can instantiate new pairs, adjust fees, or migrate pair contracts — all through DAO governance.

---

### The Cloud That Nobody Owns (Akash)

**What a 5-year-old needs to know:** The truth-machine robot needs a house to live in. If you put it in one person's house, that person could unplug it. So instead, you put it in a building owned by *lots* of different people. No single person can kick the robot out. If one person's room breaks, you move the robot to another room. That building is called Akash.

**Important:** Akash is **not** the sealed glass box (that's Azure/TEE). Akash is the *building* where the robot lives. The glass box was a one-time experiment to prove the robot can't be tampered with. Now the robot lives in Akash doing regular homework-checking — and when the building gets upgraded with glass rooms, the robot will use those too.

**What a developer needs to know:**

Akash is a decentralized compute marketplace. You post a "help wanted" ad (the SDL file), providers bid, you pick one. Three Docker containers run 24/7:

```yaml
# wavs/akash.sdl.yml — 3 containers
services:
  wavs-operator:    # 2 CPU, 4GB — watches Juno, runs WASI verification, submits attestations
  wavs-aggregator:  # 1 CPU, 1GB — collects results, health API
  ipfs:             # 0.5 CPU, 512MB — stores the 494KB WASI component binary
```

- **LIVE on Akash**: Aggregator at `http://provider.akash-palmito.org:31812` — running since March 17, 2026
- **63.77 AKT funded** — covers 3-5 months at ~US$8.76/month
- **Not TEE**: Regular compute. TEE was proven on Azure (Proposal 4). Akash = permanent decentralized hosting. Same operator code runs with or without hardware attestation.
- **Eliminates SPOF**: If the provider dies, redeploy to another in 2 minutes via Console
- **Connects to Juno**: Operator polls Juno RPC for events, submits attestation TXs back. All outbound — nothing connects inbound.

---

### The Tree That Grows (Genesis → 13 Buds)

**What a 5-year-old needs to know:** Imagine a single seed. That seed has all the instructions for a whole tree inside it. First, the seed sprouts and prepares the soil (that's Genesis setting up the contracts). Then the seed splits into 13 branches (that's the "budding"). Each branch can vote on how the tree grows. And later, those 13 branches can grow their own smaller branches.

**What a developer needs to know:**

JunoClaw uses a "budding" governance model:

```
Phase 0 — GENESIS (current)
  └── juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m
  └── Weight: 10,000 / 10,000 (100%)
  └── Can: migrate code, auto-pass any proposal, wire infrastructure

Phase 1 — INFRASTRUCTURE (done)
  └── Proposal 5: CodeUpgrade — wired Junoswap + WAVS + Akash
  └── Status: Executed ✅

Phase 2 — BUDDING (next)
  └── WeightChange proposal: Genesis → 13 buds
  └── 13 buds × 769 weight = 9,997
  └── Genesis retains 3 (symbolic membership)
  └── Total: 10,000

Phase 3 — DAO GOVERNANCE
  └── Normal proposals: 51% quorum (7 buds)
  └── Code upgrades: 67% quorum (9 buds)
  └── Further budding: WeightChange proposals to add more members
```

Genesis retains the **wasmd admin** key (emergency contract migration) but loses voting power. The 13 buds control everything else.

---

## The Juno Governance Proposal

VairagyaNodes — our validator, unbonded active at waiting position 6 — is preparing a **signaling proposal** on Juno mainnet. It asks the community to:

1. Recognize JunoClaw as official Juno ecosystem infrastructure
2. Support the Junoswap revival through TEE-attested verification
3. Endorse decentralized compute via Akash for verification
4. Acknowledge the CodeUpgrade governance framework

We're not asking for community pool funds. JunoClaw is self-funded and operational.

---

## The Scorecard (March 17, 2026)

Everything described in this article is live on uni-7 testnet. Nothing is theoretical.

| Component | Status | Proof |
|-----------|--------|-------|
| agent-company v3 | ✅ Live | code_id=63, 5 proposals executed |
| Junoswap factory | ✅ Wired | code_id=61, SetDexFactory via prop 5 |
| 2 trading pairs | ✅ Live | JUNOX/USDC, JUNOX/STAKE |
| TEE attestation | ✅ Proven | Proposal 4, SGX enclave, TX: 6EA1AE79...D26B22 |
| CodeUpgrade governance | ✅ Tested | 34/34 unit tests, 3 supermajority tests |
| Adaptive voting | ✅ Working | Unanimous votes resolve in ~1 min |
| Sortition (random jury) | ✅ Working | Fisher-Yates via SHA-256 sub-randomness, 5 tests |
| WAVS WASI component | ✅ Built | 494KB, deterministic SHA-256 verification |
| Akash operator | ✅ LIVE | `http://provider.akash-palmito.org:31812` — 3 containers, chain healthy, US$7.85/mo |
| Genesis → 13 Buds | ✅ Ready | Script + architecture doc, needs bud addresses |
| Juno governance proposal | ✅ Submitted | Mar 19, 2026 on juno-1, Proposal #373, VairagyaNodes |
| Frontend | ✅ Built | Chat, DAO, DEX, Updates — 317KB Vite |
| Jake Hartnell endorsement | ✅ | "very cool!", "junoclaw was long overdue", "good chance for Juno to rise again" |

---

## Timeline

| Date | What Happens |
|------|--------------|
| Mar 13, 2026 | ✅ Testnet contracts deployed |
| Mar 16, 2026 | ✅ TEE proof on Azure (Proposal 4, SGX) |
| Mar 17, 2026 | ✅ Junoswap + v3 + Akash + gov proposal — all today |
| Mar 17–24, 2026 | ⏳ Juno community voting period |
| ~Mar 24, 2026 | ⏳ If passes: Root → Genesis address acknowledged |
| ~Mar 25–28, 2026 | ⏳ Genesis deploys mainnet contracts on juno-1 |
| ~Mar 28–31, 2026 | ⏳ Genesis buds → 13 DAO members |
| ~Apr 1–7, 2026 | ⏳ Validator sidecar proposal (TEE-only) |
| ~Q2 2026 | ⏳ JCLAW token launch |

The seed is planted. The soil is prepared. The budding begins when the Genesis Root says so.

Jake said it best: *"Juno is going to be run by an AI soon."* That AI is JunoClaw. The infrastructure is live. The governance proposal is on juno-1. The validators are next.

---

*Written by VairagyaNodes — Juno enthusiast since 30th Dec 2021.*

*All code is open source: [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)*

*Previous articles:*
- *[The First Attestation](link) — How autonomous verification was built*
- *[JunoClaw Closes the TEE Gap](link) — Hardware-attested proofs on Juno*
