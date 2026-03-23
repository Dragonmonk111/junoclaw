# The Bud Has Passed

## Testnet governance transferred while the vote continues. What happens next is up to the DAO.

---

On March 21, 2026, while Proposal #373 runs at 89% YES with 47% turnout, something unusual happened on Juno testnet.

The deployer transferred admin control.

Not after the vote passed. Not after mainnet launched. During the voting period. Before the outcome was official.

Two contracts. Two transactions. One message: **the DAO decides what happens next.**

---

## What Transferred

**Agent-company v3** — the core governance contract that coordinates AI agents, manages proposals, and enforces the budding model.

**Junoswap factory** — the DEX factory that spawns trading pairs and routes swaps through the constant-product AMM.

Both contracts now have a new admin: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt`

That address belongs to Dimi — the validator operator who built the testnet that JunoClaw deployed on, a 1.2M JUNO staker, and bud #1 in the 13-seat governance tree.

The transactions are on-chain:
- Agent-company: `09EB9BAC204628D7E3658DC2313ED96342D533AE6C0933EEE7CA5E579387EC29`
- Junoswap factory: `9A293E0297881B23781328FD19765DAD77028A9B6BBBEA0072CD3BE24F894D5A`

Anyone can verify them on [Mintscan](https://testnet.mintscan.io/juno-testnet).

---

## Why Before the Vote?

Most projects hand off governance after deployment. After mainnet. After the product is "done."

JunoClaw is doing it backwards.

**Testnet governance transferred first.** Mainnet deployment comes second. The DAO votes on the plan.

Here's why that matters:

### The Traditional Sequence
1. Founder deploys to mainnet
2. Founder hands off governance
3. DAO inherits what was already built
4. DAO has no input on deployment decisions

### The Governance-First Sequence
1. Founder hands off testnet governance
2. DAO forms and votes on mainnet deployment plan
3. DAO decides: audit first? deploy Monday? liquidity strategy?
4. Founder executes what the DAO votes for (or DAO executes it themselves)

**The difference:** In the first model, the DAO inherits decisions. In the second model, the DAO makes them.

---

## What the DAO Can Do Now

**This weekend:**
- Dimi submits a `WeightChange` proposal on testnet
- Governance weight distributes from the deployer to 13 seats
- Buds #2-#13 get onboarded through a soulbound trust chain
- Each bud holder personally vets the next

**After Prop #373 passes Monday:**
- The DAO votes on mainnet deployment plan
- Options on the table:
  - **Deploy immediately** — contracts go live, liquidity providers coordinate
  - **Audit first** — DAO allocates funds, auditors review, deploy after clearance
  - **Phased deployment** — contracts first, liquidity later, validator sidecars follow

**The 13 decide together.** Not the deployer alone.

---

## The Budding Model

JunoClaw's governance isn't a multisig. It's a **soulbound trust tree.**

**The 13** (depth 1) — full governance weight AND infrastructure co-stewardship. They deploy, upgrade, operate.

**Branches** (depth 2+) — governance weight only. Infrastructure access requires a vote from the 13.

Each bud is non-transferable, bound to a single wallet address. The chain of custody propagates through personal vetting:
- Genesis seals bud #1 → Dimi
- Dimi seals bud #2 → [TBD]
- Bud #2 seals bud #3 → [TBD]
- ...until all 13 seats are filled

If a bud holder disappears or trust breaks, the DAO can vote to `BreakChannel` that branch and re-assign the seat. The tree heals itself.

**After the 13 are seated, the deployer has symbolic weight (3/10000)** — enough to submit a proposal, not enough to pass one. And if the 13 decide to invite the deployer back as bud #13, that's their call, not the deployer's.

---

## What This Proves

**You can build AI systems where the power structure is as transparent as the source code.**

Every layer open:
- **Contracts** — 7 crates, 86 tests, Apache 2.0
- **Compute** — WAVS operator on Akash ($8.76/month, no cloud lock-in)
- **Verification** — TEE attestation, hardware-signed proofs on-chain
- **AI agents** — Off-chain logic in WASI components, code hash published
- **Governance** — 13-seat trust-tree, weight distribution is an on-chain transaction
- **Admin keys** — Transferred on-chain, verifiable by anyone

**Open source all the way up.** Vertically. Every layer.

Not just "anyone can read the code" but "anyone can verify the power structure."

---

## What Others Can Build

The architecture isn't proprietary. It's a pattern:

- **Any DAO** can add TEE-verified AI agents with cryptographic proof of execution
- **Any DEX** can add independent swap verification — every trade re-checked by hardware
- **Any CosmWasm chain** can deploy the same governance model — soulbound trust-tree, automatic founder sunset
- **Any validator** can run a sidecar producing hardware attestations for the network
- **Any community** can fork the budding model — 13 seats, linear chain of trust, DAO decides who holds them

The code is Apache 2.0. The contracts are on GitHub. The attestation TX is on-chain. The governance structure is documented in public.

Take it. Build on it. Make it better.

---

## The Numbers

**Testnet handoff (March 21, 2026):**
- Contracts transferred: 2 (agent-company v3, junoswap factory)
- New admin: Dimi (bud #1)
- Governance seats: 13 (1 filled, 12 pending)
- Deployer voting power after budding: 0

**Prop #373 status (ongoing):**
- YES: 89.49%
- Turnout: 47.39% (quorum: 33.4%)
- No with Veto: 0%
- Ends: March 24, 2026 at 00:08 UTC

**Mainnet deployment:**
- Awaiting DAO vote after Prop #373 passes
- Timeline: DAO decides
- Strategy: DAO decides
- Liquidity: DAO coordinates

---

## What Happens Monday

If Prop #373 passes (currently 89% YES), the DAO votes on mainnet deployment.

The options are theirs to choose. The timeline is theirs to set. The strategy is theirs to decide.

The testnet proved the stack works. Three independent deployments — the original, Rattadan's instance, and the 7-member parliament demo. The code is open. The verification is on-chain. The governance model is live.

**Mainnet is the next experiment.** But the DAO runs it, not a single deployer.

---

## A Note on Trust

Most blockchain projects talk about decentralization. JunoClaw is testing whether it's possible to build it from the start.

Not "decentralize later." Not "trust us until the foundation forms." Not "the multisig will hand off eventually."

**Decentralize first. Then deploy.**

The testnet governance transferred before mainnet went live. The DAO forms before the product launches. The 13 decide the deployment plan before contracts hit mainnet.

This is the experiment: **Can you build powerful systems where nobody holds the button?**

Not because there's a promise to hand it off later. Because the button was destroyed before the system went live.

The testnet admin is gone. The mainnet plan is in DAO hands. The mnemonic will be destroyed after mainnet deployment completes.

**No backdoor. No recovery key. No "just in case."**

Either the DAO governs, or the system doesn't work. There's no middle ground.

---

## Verify Yourself

**Testnet transactions:**
- Agent-company transfer: `09EB9BAC204628D7E3658DC2313ED96342D533AE6C0933EEE7CA5E579387EC29`
- Junoswap factory transfer: `9A293E0297881B23781328FD19765DAD77028A9B6BBBEA0072CD3BE24F894D5A`
- Block explorer: https://testnet.mintscan.io/juno-testnet

**Code:**
- GitHub: https://github.com/Dragonmonk111/junoclaw
- License: Apache 2.0
- Tests: 86 passing across 7 contract crates

**Proposal:**
- Prop #373: https://daodao.zone/dao/juno/proposals/373
- HackMD: https://hackmd.io/s/HyZu6qv5Zl

**Previous coverage:**
1. [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2) — The original design
2. The First Attestation — WAVS pipeline end-to-end on Juno testnet
3. JunoClaw Closes the TEE Gap — Intel SGX proven, TX on-chain
4. JunoClaw Ships — Full stack live: Junoswap v2, Akash operator, 5 workflows
5. [What If AI Agents Had to Prove They're Honest?](https://medium.com/@tj.yamlajatt/junoclaw-what-if-ai-agents-had-to-prove-theyre-honest-4f8e5c5f9b3) — Proposal #373 announcement

---

*Built in the open. Verified by hardware. Governed by trust. Released by choice.*

**The bud has passed. The DAO decides what happens next.**

🌱
