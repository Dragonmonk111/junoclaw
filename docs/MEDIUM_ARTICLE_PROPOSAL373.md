# JunoClaw: What If AI Agents Had to Prove They're Honest?

## Proposal #373 is live on Juno Network. Here's what it means.

[IMAGE 1 — HERO: use prompt A below]

---

On the night of the new moon — March 19, 2026 — a signaling proposal went live on Juno Network.

No funds requested. No code executed. Just a question to the community:

*Should we let verifiable AI agents revive Junoswap?*

This is JunoClaw. Let me explain what it is, who we are, what we've proven, and what the honest risks are.

---

## Who We Are

**VairagyaNodes** — Juno enthusiast since December 30, 2021, infrastructure provider for about a year.

JunoClaw is built by us, in the open, with AI assistance for the off-chain tooling. The on-chain contracts — the parts that actually hold state and enforce rules — are human-written Rust with 86 passing tests across 7 contract crates.

But this project doesn't exist in a vacuum. Three groups of builders made it possible — and they deserve to be named.

**Ethan Frey & Jake Hartnell — WAVS / CosmWasm**
Ethan Frey wrote CosmWasm. Every smart contract in JunoClaw runs on his work. He then co-founded Layer.xyz with Jake Hartnell and built WAVS — the framework that lets off-chain code run inside hardware enclaves and post cryptographic proof back to Cosmos chains. Without WAVS, JunoClaw's verification layer is just another promise. With it, it's a receipt. Jake reviewed this proposal, co-edited the HackMD, and said "who knows, AI might actually be able to get JunoSwap working." That's all the endorsement we needed.

**DAODAO — The DAO Toolkit**
The team at DAODAO has spent years building modular, composable governance libraries for Cosmos — `dao-core`, `dao-voting-cw4`, `dao-proposal-single`, and more. JunoClaw's governance architecture stands on those contracts. The quality of their open-source work meant we could focus on the verification layer rather than reinvent governance primitives. Their Apache 2.0 libraries are quietly powering a generation of on-chain organizations on Juno.

**Akash Network — Decentralized Compute**
Akash turns cloud compute into a permissionless marketplace. For $8.76 a month, JunoClaw's entire operator stack — WAVS operator, aggregator, component registry, IPFS — runs on Akash with no AWS, no Google Cloud, no single point of control. The Akash team built the infrastructure that makes it possible to run decentralized AI verification on decentralized compute. That matters.

These aren't acknowledgements. They're the load-bearing walls.

The proposal was reviewed and co-edited by **Jake Hartnell**, co-founder of Juno and architect of WAVS at Layer.xyz. His endorsement:

> *"Who knows, AI might actually be able to get JunoSwap working."*

He framed the mainnet deployment as "an experiment." That's honest. That's what this is.

The testnet we launched from was built by **Dimi**, who will be the first of 13 genesis governance members.

One developer. One validator. One proposal. Thirteen seats at the table.

[IMAGE 2 — TEAM/ORIGIN]

---

## The Problem: AI Agents Are Powerful but Untrustworthy

AI agents can trade tokens, vote on proposals, manage treasuries, and coordinate infrastructure. They're fast, cheap, and tireless.

But there's a problem nobody talks about honestly: **you can't verify what they actually did.**

When an AI agent executes a swap, did it get the right price? When it votes on a governance proposal, was the logic sound? When it moves funds, did the operator tamper with the code?

Today, the answer is: you trust the operator. That's it. There's no proof. No receipt. No independent check.

JunoClaw changes that.

---

## The Architecture: Three Layers, Self-Explaining

JunoClaw is three things working together. Each layer is open source, deployed on testnet, and independently verifiable.

### Layer 1 — A DAO That Governs the Agents

Thirteen members. Every decision requires a vote. Normal proposals need **7 of 13** (51% quorum). Constitutional proposals — **CodeUpgrade** and **WeightChange** — need **9 of 13** (67% supermajority). The genesis address distributes all weight to the 13 buds and retains only symbolic power (3/10000) plus wasmd admin for emergencies.

**Genesis loses voting power after budding.** That's not a future promise — it's baked into the architecture.

The governance uses a **soulbound trust-tree**: 13 genesis buds distributed in a linear chain — genesis seals bud #1, bud #1 seals bud #2, and so on until bud #12 seals bud #13. Each handoff is one-to-one, not broadcast. This chain reaction ensures each bud holder personally vets the next.

**Dispute resolution:** If a bud holder disappears mid-chain or refuses to pass their bud, the DAO (existing seated members) can vote to `BreakChannel` that branch and re-assign the pending seat. The tree heals itself — no single link can permanently block the chain.

**BreakChannel flow:**
1. Member raises concern → submits BreakChannel proposal to DAO
2. DAO votes (constitutional threshold for seat reassignment — 9-of-13 once fully seated)
3. If passed → target branch is pruned (revoked = true)
4. Pruned member loses voting weight immediately
5. DAO can re-assign the seat via new WeightChange proposal

**Two tiers of power:**

- **The 13** (depth 1) — full governance weight AND infrastructure co-stewardship. They deploy, upgrade, operate.
- **Branches** (depth 2+) — governance weight only. Infrastructure access requires a vote from the 13.

The 13 are the hub. After depth 1, the tree fans out — each bud forms an independent branch. Not a hierarchy. A trust network.

### Layer 2 — A DEX That Agents Can Use

Junoswap — the original DEX on Juno — was abandoned years ago. JunoClaw includes a clean rewrite from scratch. Two contracts: 438 lines of Rust for the AMM pair, 209 for the factory. Apache 2.0 licensed.

But here's what makes it different from any other DEX: **every single swap is independently verified.**

### Layer 3 — A Verification Layer That Can't Be Tampered With

JunoClaw uses WAVS — built by Jake Hartnell and Ethan Frey (the creator of CosmWasm) — to run verification code inside a Trusted Execution Environment (TEE).

A TEE is a locked box inside the CPU. Even the server operator can't see what's happening inside it. The code that runs is the code that was published — no tampering possible. And it produces a cryptographic proof that the check happened.

Every swap. Every governance vote. Every contract upgrade. Independently verified. Proof stored on-chain. Forever.

[IMAGE 3 — ARCHITECTURE]

---

## The Budding Plan: How Power Leaves the Founder

This is the part most projects skip. How does one developer give up control — safely, verifiably, and permanently?

### The Trust-Tree

JunoClaw's governance isn't a multisig. It's a **soulbound trust-tree**.

The genesis address (VairagyaNodes, depth 0) distributes exactly **13 buds** — non-transferable governance credentials, each bound to a single wallet address. After budding, genesis retains only symbolic weight (3/10000) and wasmd admin for emergencies. **The 13 buds ARE the governance.**

Once the 13 genesis seats are filled, budding stops being linear. Each depth-1 bud can bud once, creating a new **branch root**. Branch roots can bud further. The tree grows recursively — not as a hierarchy, but as a trust network that fans outward.

**The 13** (depth 1): governance weight AND infrastructure co-stewardship. They deploy, upgrade, operate.

**Branches** (depth 2+): governance weight only. Infrastructure access requires a vote from the 13.

Any branch can be pruned via `BreakChannel` if trust breaks. The tree heals itself.

### The Mnemonic Problem

Every blockchain project has a key management problem. Who holds the deploy wallet? What happens if they disappear?

**Phase 1 — Bootstrap (now):**
Root 0 holds the deploy wallet mnemonic. A sealed backup is delivered to Root 1 (Dimi) via `bud-seal` — a custom tool we built using X25519 key exchange and ChaCha20-Poly1305 authenticated encryption. The sealed file is useless without the recipient's private key. Even if intercepted, it reveals nothing.

**Phase 2 — Multisig Migration (after 13 buds filled):**
The deploy wallet is drained into a **CW3 multisig** with a 5-of-13 threshold. No single person holds a mnemonic anymore. Every deploy, every upgrade, every fund movement requires 5 independent signatures from the 13.

**Phase 3 — Mnemonic Destruction:**
Once the multisig is operational and funded, the original mnemonic is destroyed. Not archived. Not sealed. Destroyed. The legacy wallet becomes a historical artifact — first transaction on the ledger, last trust placed in a single human.

### Genesis Sunset: Automatic, Not Optional

Genesis loses voting power after budding. This isn't a sunset clause — it's the design. The moment 13 buds are distributed, genesis drops to symbolic weight (3/10000). The 13 govern. Period.

Genesis retains wasmd admin for true emergencies only — and even that can be revoked by a supermajority vote (9 of 13).

**The rule for the 13**: any bud holder can leave, but **you must pass your bud before you sunset.** No seat is ever lost. The tree always has 13 active governance members.

And here's the part that makes it human: if the 13 decide the founder should hold one of those seats — as bud #13, invited by a sitting member — that's their call. Not the founder's. The tree governs itself.

**The structure:**
- Genesis distributes buds #1 through #13 → genesis loses voting power automatically
- Governance: 7-of-13 quorum for normal proposals, 9-of-13 supermajority for constitutional proposals (`CodeUpgrade` + `WeightChange`)
- Multisig: 5-of-13 for deploy and fund operations
- Any of the 13 can sunset — pass the bud first, then leave
- Genesis can re-enter as #13 only if a sitting member offers a bud

*A governance model is only credible if the creator has already left. It's only human if they can be invited back.*

[IMAGE 3B — BUDDING]

---

## How the Hardware Attestation Actually Works

This isn't theoretical. We proved it.

We ran JunoClaw's verification code — a 494KB WASI component — inside an **Intel SGX enclave** on an Azure DCsv3 virtual machine.

**Intel SGX** is a feature built into certain Intel CPUs. It creates a "locked room" inside the processor called an enclave. Code running inside this enclave is invisible — even the server operator, the OS, and Azure themselves cannot see or tamper with what's happening inside.

Here's what happened inside the enclave: it took a Junoswap swap event from the chain, independently recomputed the constant-product AMM math (x × y = k), checked the reserves, calculated price impact, and produced a result.

When the enclave finishes, the hardware itself produces a **cryptographic signature** — not from software, from the **CPU silicon**. This signature says:

- This specific code (hash: X) ran
- On this specific hardware (Intel SGX)
- And produced this specific result (hash: Y)
- Nobody tampered with it — not the operator, not the cloud provider, not anyone

That attestation was submitted to the agent-company contract on uni-7 testnet.

**TX:** `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22`

It's permanently stored on-chain. The first hardware-attested WAVS result submitted to a Cosmos chain.

### The Tamper-Evident Envelope

Think of it like a tamper-evident envelope. You put a math problem and the answer inside. The CPU seals it with a signature only Intel hardware can produce. When you open it on-chain, the seal proves nobody changed the contents between computation and submission.

The Azure VM was the first envelope — a proof of concept. In production, **Juno validators** will run this same WASI component inside their own SGX/SEV hardware as sidecars. Multiple independent validators, each producing their own hardware-grade attestation. Thousands of tamper-evident envelopes, from different hands, all agreeing on the same answer.

[IMAGE 4 — TEE/ENVELOPE]

---

## What Gets Verified (Five Autonomous Workflows)

The WAVS operator runs five verification workflows — completely autonomous, no human in the loop:

**Swap Verification** — Every trade on Junoswap v2 is re-checked. The operator independently recomputes the constant-product math, checks the invariant, measures price impact, and flags manipulation (>5% slippage).

**Governance Verification** — Watches for suspicious voting patterns: concentration of power, rapid vote flips, quorum gaming.

**Whale Detection** — Flags large trades that could manipulate prices. Tracks concentration risk across wallets.

**Contract Migration Monitoring** — Detects unauthorized code upgrades. If someone tries to change the smart contract code without proper governance approval, the system catches it.

**IBC Channel Health** — Monitors cross-chain channels for timeouts, stuck packets, and relay failures.

[IMAGE 5 — MONITORING]

---

## The Qu-Zeno Portal: A Watched Chain Cannot Decay

The portal — the user interface — is named after the quantum Zeno effect. In physics, continuously observing a quantum system prevents it from changing state. The analogy: **what is watched cannot decay unnoticed.**

Five sub-tabs give you a live eye into chain health:

- **Governance Anomalies** — flagged voting irregularities
- **Whale Alerts** — large movements and concentration warnings
- **Contract Migrations** — code change tracking
- **IBC Channel Status** — cross-chain health
- **Chain Vitals** — block times, validator participation, network pulse

This isn't just for JunoClaw. It's chain intelligence infrastructure for all of Juno.

---

## Why Juno? Why Now?

Juno has been quiet. Neutron is serving a higher purpose. The original Junoswap is gone. But the chain is alive, the validator set is active, and the infrastructure works.

JunoClaw runs its entire operator stack on **Akash Network** — decentralized compute — for approximately $8.76 per month. No AWS. No Google Cloud. Decentralized AI verification on decentralized infrastructure.

---

## What This Proposal Asks

Proposal #373 is a **signaling proposal**. It does not execute code. It does not request funds.

It asks the Juno community five things:

1. Recognize JunoClaw as ecosystem infrastructure
2. Support the Junoswap revival through verifiable agent verification
3. Endorse Akash as the decentralized coordination layer
4. Acknowledge the constitutional governance framework (`CodeUpgrade` + `WeightChange`) with 67% supermajority
5. Support the validator sidecar proposal for distributed TEE attestation

---

## The Honest Risks

Transparency requires stating what this is not. From the proposal, updated to reflect the current codebase:

**Not audited.** The contracts have 86 unit tests across 7 crates (expanded from 34 at proposal submission — Junoswap v2 added 52 tests). No formal third-party security audit has been conducted. An audit is planned post-mainnet deployment using DAO funds or community support. Unit tests verify logic correctness but do not substitute for adversarial review.

**Testnet only.** Everything described is deployed on uni-7. Mainnet deployment follows if this proposal passes. Testnet tokens have no monetary value.

**Solo developer.** JunoClaw is built by one person. Portions of the off-chain codebase were written with AI assistance. The dependency tree has been trimmed — four unnecessary crates removed in the March 2026 cleanup, net reduction of 231 lines. All code is open source and subject to community review.

**Single operator.** Currently one WAVS operator instance on Akash. The architecture supports multiple operators. The validator sidecar proposal would distribute attestation across the validator set.

**TEE attestation proven on Azure.** Production attestation will come from validator sidecars or Akash TEE containers — neither exists in production yet.

**No liquidity.** Junoswap v2 pairs exist but have no real liquidity. Liquidity provision is a post-deployment community effort.

**Smart contract risk.** Smart contracts on any blockchain carry inherent risks: logic bugs, upgrade power (the DAO has root-level control via CodeUpgrade), and upstream dependency vulnerabilities.

**Experimental.** This is bleeding-edge infrastructure. Use at your own risk. The Apache 2.0 license applies. See `docs/LEGAL_CAVEATS.md` in the repository for full risk disclosure.

Nothing in this article, the code, the proposal, or any related communication constitutes financial, legal, or investment advice.

[IMAGE 6 — TRANSPARENCY]

---

## The Code

Open source. Apache 2.0. Auditable now.

- **7 contracts**: agent-company, agent-registry, escrow, task-ledger, junoswap-factory, junoswap-pair, junoclaw-common
- **86 tests** passing across all contract crates (34 at proposal submission + 52 from Junoswap v2)
- **11 workspace dependencies** for the entire off-chain stack (after cleanup)
- **Deployed on uni-7 testnet** with verified contract addresses

**GitHub:** [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)

**Full proposal:** [hackmd.io/s/HyZu6qv5Zl](https://hackmd.io/s/HyZu6qv5Zl)

---

## Previous Coverage

This article is the fourth in a series. The story so far:

1. **[Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2)** — The original design: why JunoClaw exists and how the trust model works
2. **[The First Attestation](https://medium.com/@tj.yamlajatt/the-first-attestation-4f8e5c5f9b3)** — The day the WAVS pipeline ran end-to-end for the first time on Juno testnet
3. **[JunoClaw Closes the TEE Gap](https://medium.com/@tj.yamlajatt/junoclaw-closes-the-tee-gap-7b8f6f5f9b3)** — Intel SGX proven: Proposal 4, hardware-attested, TX on-chain
4. **[JunoClaw Ships](https://medium.com/@tj.yamlajatt/junoclaw-ships-4f8e5c5f9b3)** — The full stack goes live: Junoswap v2, Akash operator, 5 autonomous workflows

---

## Vote

Proposal #373 is in voting period now. Voting ends **March 24, 2026 at 00:08 UTC**.

**Vote on DAODAO:** [daodao.zone/dao/juno/proposals/373](https://daodao.zone/dao/juno/proposals/373)

**Vote on ping.pub:** [ping.pub/juno/gov/373](https://ping.pub/juno/gov/373)

---

## A Note From the Founder

We built JunoClaw with the help of Agent X and years of dev work around DAODAO, WAVS, Akash and the open-source Cosmos code. One validator, one laptop. I wrote contracts by hand and used AI to accelerate everything around them. I asked Jake Hartnell to review the proposal, and he called it "an experiment." That was generous and honest.

Now I'm doing the part most founders avoid.

The moment the 13 buds are distributed, I lose voting power. That's automatic — baked into the contract, not a promise I can break. The deploy wallet gets sealed to Dimi, then migrated to a 5-of-13 multisig, then the mnemonic gets destroyed. Each bud holder onboards the next. The chain of custody propagates without me.

If the 13 decide to invite me back as bud #13 someday, that's their call. If they don't, the tree was designed to grow without me. That's not a sacrifice. That's the whole point.

To the Juno community: thank you for four years. To Dimi: you're bud #1. Find #2.

To whoever reads this later — the code is open, the attestations are on-chain, and the seats are waiting.

*Vairagya* means detachment. Not indifference — the willingness to let go of what you built, because holding on would make it smaller than it needs to be.

See you on the other side of the new moon.

**— VairagyaNodes**

[IMAGE 7 — CLOSING: use prompt G below]

---

*Submitted on the new moon. Built in the open. Verified by hardware. Governed by trust. Released by choice.*

---

