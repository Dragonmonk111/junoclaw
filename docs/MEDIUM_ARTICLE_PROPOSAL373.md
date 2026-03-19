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

**VairagyaNodes** — solo Juno validator since December 30, 2021. Day one of mainnet. Four years of blocks, zero downtime events, zero governance abstentions. The wallet that submitted this proposal has been staking since genesis.

JunoClaw is built by one person, in the open, with AI assistance for the off-chain tooling. The on-chain contracts — the parts that actually hold state and enforce rules — are hand-reviewed Rust with 86 passing tests across 7 contract crates.

The proposal was reviewed and co-edited by **Jake Hartnell**, co-founder of Juno and architect of WAVS at Layer.xyz. His endorsement:

> *"Who knows, AI might actually be able to get JunoSwap working."*

He framed the mainnet deployment as "an experiment." That's honest. That's what this is.

The testnet we launched from was built by **Dimi**, who will be the first of 13 genesis governance members.

One developer. One validator. One proposal. Thirteen seats at the table.

[IMAGE 2 — TEAM/ORIGIN: use prompt B below]

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

Thirteen members. Every decision requires a vote. Normal proposals need **7 of 13** (51% quorum). Code upgrades need **9 of 13** (67% supermajority). The genesis address distributes all weight to the 13 buds and retains only symbolic power (3/10000) plus wasmd admin for emergencies.

**Genesis loses voting power after budding.** That's not a future promise — it's baked into the architecture.

The governance uses a **soulbound trust-tree**: 13 buds — non-transferable governance credentials, each bound to one wallet address. Each bud holder can bud once, creating a branch. The tree is recursive and prunable via `BreakChannel` if trust breaks.

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

[IMAGE 3 — ARCHITECTURE: use prompt C below]

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
- Governance: 7-of-13 quorum, 9-of-13 supermajority
- Multisig: 5-of-13 for deploy and fund operations
- Any of the 13 can sunset — pass the bud first, then leave
- Genesis can re-enter as #13 only if a sitting member offers a bud

*A governance model is only credible if the creator has already left. It's only human if they can be invited back.*

[IMAGE 3B — BUDDING: use prompt C2 below]

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

[IMAGE 4 — TEE/ENVELOPE: use prompt D below]

---

## What Gets Verified (Five Autonomous Workflows)

The WAVS operator runs five verification workflows — completely autonomous, no human in the loop:

**Swap Verification** — Every trade on Junoswap v2 is re-checked. The operator independently recomputes the constant-product math, checks the invariant, measures price impact, and flags manipulation (>5% slippage).

**Governance Verification** — Watches for suspicious voting patterns: concentration of power, rapid vote flips, quorum gaming.

**Whale Detection** — Flags large trades that could manipulate prices. Tracks concentration risk across wallets.

**Contract Migration Monitoring** — Detects unauthorized code upgrades. If someone tries to change the smart contract code without proper governance approval, the system catches it.

**IBC Channel Health** — Monitors cross-chain channels for timeouts, stuck packets, and relay failures.

[IMAGE 5 — MONITORING: use prompt E below]

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

Juno has been quiet. Neutron didn't deliver. The original Junoswap is gone. But the chain is alive, the validator set is active, and the infrastructure works.

JunoClaw runs its entire operator stack on **Akash Network** — decentralized compute — for approximately $8.76 per month. No AWS. No Google Cloud. Decentralized AI verification on decentralized infrastructure.

---

## What This Proposal Asks

Proposal #373 is a **signaling proposal**. It does not execute code. It does not request funds.

It asks the Juno community five things:

1. Recognize JunoClaw as ecosystem infrastructure
2. Support the Junoswap revival through verifiable agent verification
3. Endorse Akash as the decentralized coordination layer
4. Acknowledge the CodeUpgrade governance framework with 67% supermajority
5. Support the validator sidecar proposal for distributed TEE attestation

---

## The Honest Risks

Transparency requires stating what this is not. Verbatim from the proposal:

**Not audited.** The contracts have 86 unit tests across 7 crates but have not undergone a formal third-party security audit. An audit is planned post-mainnet deployment using DAO funds or community support. Unit tests verify logic correctness but do not substitute for adversarial review.

**Testnet only.** Everything described is deployed on uni-7. Mainnet deployment follows if this proposal passes. Testnet tokens have no monetary value.

**Solo developer.** JunoClaw is built by one person. Portions of the off-chain codebase were written with AI assistance. The dependency tree has been trimmed — four unnecessary crates removed in the March 2026 cleanup, net reduction of 231 lines. All code is open source and subject to community review.

**Single operator.** Currently one WAVS operator instance on Akash. The architecture supports multiple operators. The validator sidecar proposal would distribute attestation across the validator set.

**TEE attestation proven on Azure.** Production attestation will come from validator sidecars or Akash TEE containers — neither exists in production yet.

**No liquidity.** Junoswap v2 pairs exist but have no real liquidity. Liquidity provision is a post-deployment community effort.

**Smart contract risk.** Smart contracts on any blockchain carry inherent risks: logic bugs, upgrade power (the DAO has root-level control via CodeUpgrade), and upstream dependency vulnerabilities.

**Experimental.** This is bleeding-edge infrastructure. Use at your own risk. The Apache 2.0 license applies. See `docs/LEGAL_CAVEATS.md` in the repository for full risk disclosure.

Nothing in this article, the code, the proposal, or any related communication constitutes financial, legal, or investment advice.

[IMAGE 6 — TRANSPARENCY: use prompt F below]

---

## The Code

Open source. Apache 2.0. Auditable now.

- **7 contracts**: agent-company, agent-registry, escrow, task-ledger, junoswap-factory, junoswap-pair, junoclaw-common
- **86 tests** passing across all contract crates
- **11 workspace dependencies** for the entire off-chain stack (after cleanup)
- **Deployed on uni-7 testnet** with verified contract addresses

**GitHub:** [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)

**Full proposal:** [hackmd.io/s/HyZu6qv5Zl](https://hackmd.io/s/HyZu6qv5Zl)

---

## Vote

Proposal #373 is in voting period now. Voting ends **March 24, 2026 at 00:08 UTC**.

**Vote on ping.pub:** [ping.pub/juno/gov/373](https://ping.pub/juno/gov/373)

[IMAGE 7 — CLOSING: use prompt G below]

---

*Submitted on the new moon. Built in the open. Verified by hardware. Governed by trust.*

---

## Midjourney Prompts

For Medium, images should be wide format. Add `--ar 16:9 --s 200 --v 6.1` to all prompts.

**Prompt A — Hero Image:**
`A massive ancient Japanese torii gate standing at the edge of a dark ocean under a new moon, faint bioluminescent waves, a single glowing claw mark carved into the gate, cinematic lighting, muted indigo and gold tones, epic scale, no text --ar 16:9 --s 200 --v 6.1`

**Prompt B — Team/Origin:**
`A solitary monk sitting cross-legged before a vast cosmic console, holographic star maps and blockchain nodes orbiting around them, one hand raised with 13 glowing seeds floating above the palm, dark temple interior, volumetric light from above, contemplative mood --ar 16:9 --s 200 --v 6.1`

**Prompt C — Architecture (Three Layers):**
`Three translucent layers stacked vertically in space: bottom layer is a stone council table with 13 seats, middle layer is flowing liquid gold streams between vessels, top layer is a crystalline lattice of connected nodes pulsing with light, each layer connected by luminous threads, dark cosmic background, architectural diagram feel --ar 16:9 --s 200 --v 6.1`

**Prompt D — TEE / Tamper-Evident Envelope:**
`A glowing tamper-evident envelope made of silicon and light, sealed with a crystalline Intel chip as the wax stamp, the envelope floating above an ancient stone altar, faint mathematical equations visible through the translucent material, dramatic chiaroscuro lighting, photorealistic detail --ar 16:9 --s 200 --v 6.1`

**Prompt E — Monitoring / The Eye:**
`A massive ethereal eye made of interconnected blockchain nodes and data streams, watching over a miniature cosmos of orbiting chains and bridges, five distinct beams of light scanning different sectors, dark observatory setting, cyberpunk meets ancient astronomy, teal and amber palette --ar 16:9 --s 200 --v 6.1`

**Prompt F — Transparency / Honest Risks:**
`An open book made of transparent glass lying on a rough wooden table, every page visible through every other page, a single candle illuminating the text from within, scattered test tubes and measuring instruments around it, the mood of honest science not salesmanship, warm muted tones --ar 16:9 --s 200 --v 6.1`

**Prompt C2 — Budding / Trust-Tree:**
`A single ancient tree growing in zero gravity, its trunk splits into exactly 13 primary branches each tipped with a glowing bud, from each bud a secondary tree begins to sprout, the root system below is made of golden circuitry that fades into darkness, one branch has a clean cut where it was pruned and the wound glows with amber light, dark cosmic void background, bioluminescent bark, the feeling of designed impermanence --ar 16:9 --s 200 --v 6.1`

**Prompt G — Closing / New Moon:**
`A new moon rising over the Juno landscape, 13 small fires burning in a circle on the ground below, a single claw mark glowing in the dark sky like a constellation, the feeling of something beginning not ending, cinematic wide shot, deep indigo and warm amber --ar 16:9 --s 200 --v 6.1`
