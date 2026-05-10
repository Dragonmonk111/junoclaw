# The Verifiable Agent

## What we built on Juno, what it solves, and why the way we built it matters more than the code itself.

---

*An overview for anyone — builders, validators, and the curious — who wants to know what JunoClaw actually is, what it does on-chain today, and where it is going next.*

*Written May 3, 2026 — as proposal #374 crosses quorum on Juno mainnet: 33.5% turnout, 61.24% yes, zero no votes. The chain is speaking.*

*Update, May 10, 2026 — the devnet now measures the precompile path empirically. The projected ~187–223k gas range is replaced by a measured **203,266 gas** per `VerifyProof` (5/5 deterministic samples), confirming a **1.823× reduction** vs pure-Wasm. Numbers below are updated in place; the original projection lives in `docs/BN254_BENCHMARK_PROJECTED.md`.*

---

![A glowing robotic figure stands at the edge of a vast digital cliff, below it an endless sea of unverified data fragments drifting like paper boats, question mark formed by drifting constellations overhead.](images/va_01_the_problem.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, late 1980s anime cel art style, a lone glowing robotic figure standing at the edge of a vast digital cliff, below it an endless sea of unverified data fragments drifting like tiny paper boats in fog, ink outlines, muted teal and amber palette, question mark formed by drifting geometric constellations overhead, Akira meets Moebius, VHS grain texture, no text -->

## The problem, stated plainly

Agents — whether AI models, automated market-makers, or cross-chain routers — are about to do more work on blockchains than humans ever will. They will triage insurance claims, verify credentials, route swaps, match grant applicants to funders, settle disputes, watch crops, count attendance. Most of this work happens off-chain, because chains are too slow and too expensive to run it natively.

Off-chain is also where trust breaks down. An agent that runs inside a developer's laptop, or a centralised API, or a cloud VM, can be asked to show its work — but you have to take its word for the answer. "The AI decided" is the modern version of "the dog ate it." When the stakes are a farmer's crop payout, or a student's credential, or a small community's mutual-aid fund, taking the agent's word is not good enough.

The question is: **how do you verify that an agent did what it said it did, without re-running the whole computation on-chain?**

That is the problem JunoClaw is built to solve.

---

![Nine glowing crystalline nodes arranged in a constellation pattern, each connected by luminous circuit traces, a vast cosmic architecture viewed from above.](images/va_02_what_we_built.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, 1980s sci-fi technical blueprint style, nine glowing crystalline nodes arranged in a perfect constellation pattern, each connected to the others by luminous circuit traces of different colors, zoomed-out view of a vast cosmic architecture, muted neon purple and gold ink on deep space background, Moebius cross-section diagram style, extreme detail in the connections, film grain, no text -->

## What we built

JunoClaw is a stack of nine CosmWasm contracts, one off-chain WAVS operator, and a set of cryptographic primitives — currently being proposed on Juno as a BN254 precompile (proposal #374, live as of this writing) — that together let a DAO:

1. **Define a task** — a structured job with constraints: "verify this credential by block height X," "pay out this grant if condition Y is attested by a TEE," "route this swap through Junoswap if quote is better than Z."
2. **Escrow the reward** — funds sit locked until the task is completed or expires.
3. **Let an agent pick it up and execute** — inside a Trusted Execution Environment (TEE) whose hardware attestation reflects exactly what happened inside it and nothing else.
4. **Accept a cryptographic proof that the work was done correctly** — a Groth16 zero-knowledge proof, verified on-chain in constant time regardless of how complex the underlying computation was.
5. **Settle** — the agent is paid, the DAO's reputation ledger updates, and the full receipt is on-chain and public.

The nine contracts implement the plumbing: `task-ledger` (the queue), `escrow` (the vault), `agent-registry` (reputation), `agent-company` (the governance + DAO logic), `zk-verifier` (the cryptographic gate), `junoswap-pair` (a DEX integration, with hardened denom-whitelisting), `builder-grant` (milestone-locked funding), `jclaw-token` (a soulbound trust-tree credential — more on this below), and `jclaw-airdrop` (the genesis distribution).

Together they form what we call an **agent-company**: a DAO that can hire, fire, pay, and audit autonomous agents the same way a real company hires, fires, pays, and audits contractors — except every step is on-chain, every receipt is cryptographically true, and the whole thing is open-source under Apache 2.0.

---

![A lone figure in a glowing glass capsule floating inside a luminous circular river of data, the river closes at a single crystalline chain link that glows at the point of completion.](images/va_03_the_loop.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, late 1980s anime cel art style, a lone figure in a glowing glass capsule floating inside a luminous circular river of data, the river orbits through a constellation of seven floating stations before reconnecting at a single crystalline chain link that glows bright at the closing point, deep space backdrop with nebula clouds, teal and amber palette, Moebius linework, film grain, no text -->

## How it fits together — one task, end to end

A working example that actually runs on our testnet today:

1. A DAO member proposes: *"Verify 50 grant applications by end-of-week. 500 JUNO escrowed per correct verification."*
2. The DAO votes. Adaptive voting shortens the deadline if everyone votes early — a small touch, but it means fast-moving decisions don't wait 100 blocks for the slowest voter.
3. The task is published. An off-chain WAVS operator picks it up — running our LLM pipeline inside a TEE workbox.
4. For each application, the agent:
   - Reads the applicant's submitted credentials.
   - Cross-checks them against on-chain proofs (IBC queries to other chains, stored attestations).
   - Produces a structured verdict (`pass`, `fail`, or `refer-to-human`).
   - Produces a Groth16 proof that the computation ran exactly as specified inside the TEE.
5. The agent submits the verdict and the proof to our `zk-verifier` contract.
6. The contract verifies the proof on-chain. Today, that verification costs ~370,600 gas per proof — enough that we sample, rather than verify, most of them. With BN254 precompiles (prop #374, now empirically measured on our `junoclaw-bn254-1` devnet), the cost drops to **203,266 gas — a measured 1.823× reduction**: **cheap enough to verify every single task**. That is the threshold that turns optional auditing into universal auditing.
7. The contract releases the escrow. The agent is paid. The DAO's ledger records the outcome. The agent's reputation score ticks up.

The whole loop completes in seconds, and every step — the task, the proof, the verification, the payout — is on-chain and queryable.

---

![Split composition: left side a shadowed figure whispering with question marks floating between them representing blind trust; right side a geometric crystalline proof structure glowing with certainty.](images/va_04_different.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, 1980s graphic novel style, split composition, left half shows two shadowed figures facing each other with floating question marks and broken chains between them representing blind trust that fails, right half shows a single geometric crystalline proof structure radiating certainty like a lighthouse, a sharp luminous diagonal divides the two halves, muted purple left side and warm amber right side, cross-hatching ink technique, Moebius meets Frank Miller, film grain, no text -->

## What makes it different

There are plenty of DAO frameworks. There are plenty of zero-knowledge verifiers. There are plenty of TEE platforms. The novelty is the wiring.

**Compute preservation.** The agent does the heavy lifting off-chain. It doesn't re-execute on-chain — it proves it already executed correctly, and the chain verifies the proof in constant time. A task whose logic would cost millions of gas to run natively costs tens of thousands to verify. This is the same trick that lets rollups scale Ethereum; we are applying it at the contract layer on a sovereign Cosmos chain, which means no bridging assumptions and no L2 complexity.

**Universal auditing.** Because verification is cheap, every task is verified. Not a sample. Not a spot-check. Every one. A TEE attestation guarantees the agent *could not have cheated*; the BN254 proof guarantees the chain *actually checked*. Together they close a loop that every off-chain-agent system we know of leaves open.

**Adaptive governance.** Votes don't wait for the laggard. If every member of a small DAO votes within ten blocks, the proposal resolves in thirteen. The chain is optimised for the common case — everyone paying attention — without sacrificing the worst case.

**A soulbound trust tree.** `$JClaw` is not a tradable token. It is a credential passed from one person to one other person — each holder "buds" once, to one trusted collaborator, and that collaborator can bud to their trusted collaborator, and so on. Governance can prune a branch if a trust relationship breaks. Reputation is structural, not numerical.

**Minimal surface, honest taxonomy.** Our `agent-company` contract depends on zero Juno-specific modules today — only standard `cosmwasm-std` and `cw-storage-plus`. Every dependency is a deliberate choice, not an accident of copy-paste. The `MARIUS_ASSESSMENT.md` in the repo surveys the entire Juno module catalog against our surface and notes where integration would help — but nothing ships that isn't needed.

---

## Real-world threads

We have built nine template DAOs into the frontend, each one a realistic use-case, each one tested end-to-end on our testnet:

1. **Community Fund** — a transparent grant DAO with witness verification.
2. **Crop Protection Pool** — parametric insurance for smallholder farmers, with TEE-attested oracle feeds and WAVS-verified satellite data.
3. **Credential Verifier** — a WAVS-first DAO for professional credentials: medical certifications, trade qualifications, academic degrees.
4. **Community Vote** — a general-purpose sortition-free voting DAO.
5. **Mutual Aid DAO** — witness-and-WAVS hybrid for small communities that need both human witnesses and automated fraud checks.
6. **Farm-to-Table Market** — a producer-to-consumer market with traceability proofs.
7. **Citizens' Assembly** — sortition-based governance, with NOIS/drand randomness selecting members.
8. **Skill-Staking Circle** — peer-to-peer skill exchange with reputation-as-stake.
9. **Verifiable Outcome Market** — prediction markets where the outcome is proven, not voted on.

These aren't speculative — they are shipped templates, with working contracts, with a five-step wizard UI that deploys them live. Anyone with a Keplr wallet can spin one up on Juno today.

The unifying idea: **every domain where trust is currently expensive is a domain where cheap verification is transformative**. A farmer can't afford to hire an auditor. A credentialling body can't afford to re-verify every degree. A mutual-aid group can't afford to be defrauded. Cheap proof changes the cost structure of trust itself.

---

![Diverse community members seen from behind — a farmer, a student, a community elder — each looking up at luminous proof chains descending like aurora borealis from a night sky, spring cherry blossoms in the foreground.](images/va_05_real_world.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, late 1980s anime cel art style, diverse community figures seen from behind and side-profile only — a farmer holding a tablet, a student with a glowing credential, a community elder with hands open — all looking up at luminous verification chains descending like aurora borealis from a cosmic night sky, spring cherry blossom branches frame the foreground, warm amber and teal palette, Nausicaä meets Moebius, ink outlines, no faces visible, film grain, no text -->

## How we build — the ethos that matters more than the code

We have been accused, more than once, of moving fast. The accusation is true; the caveat is more interesting. We move fast **on writing**, and slowly on shipping. Every contract has gone through four tagged iteration passes: v4 → v5 → v6 → v6.1 → v7. Every iteration corrected real, demonstrable defects:

- **v5** fixed a supermajority arithmetic bug where abstentions were counted toward quorum in a way that let 51% + enough abstains pass a 67%-required proposal.
- **v6.0** fixed identity holes: unauthenticated task submission, WAVS operator identity overloaded onto the task ledger.
- **v6.1** fixed value-flow holes: submitter-self-confirmation, 1-ujunox griefing attacks on distribution, duplicate work hashes, silently-eaten denoms from rogue tokenfactory mints.
- **v7** added the Tier-1-slim and Tier-1.5 constraint vocabularies — a capability expansion rather than a fix.

These were not discovered by us alone. The Ffern Institute audit produced five findings and five advisories, all remediated in a tagged security release. Dimi's security-patch cadence shaped the v29 chain we build against. Marius's critique of the CosmWasm module surface produced `MARIUS_ASSESSMENT.md`, a 170-line document surveying every module in the `juno v29` generation (current mainnet: v29.1, following a Cosmos Labs security patch) and ranking integration priorities. Rattadan helped us navigate the validator community when we were new to uni-7.

**We name each contributor for what they actually did.** When a coding agent wrote a contract, we say so. When a community member gave validator-ops guidance, we say that — not that they wrote contracts they didn't write. This sounds obvious; it is in fact the hardest discipline in open-source, because the temptation is always to overstate adjacent help. We publish a correction when we get it wrong. Our `MEDIUM_ARTICLE_AFTER_THE_VOTE.md` includes a full attribution correction where we previously misattributed technical work to a community member.

**Every number reproduces.** The pure-Wasm gas baseline is measured on a specific transaction (`F6D5774E…5080F4DA`, block 12,673,217) against a specific contract (`juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`, code_id 64) on `uni-7` — and reproduced on our `junoclaw-bn254-1` devnet at 370,600 gas (5/5 samples, σ=0). The precompile path was originally projected from the EIP-1108 schedule plus a documented 30k SDK-gas overhead ceiling; as of May 10, 2026 it is **measured** at 203,266 gas (5/5 samples, σ=0) on the same devnet — within ~9% of the projection, well under the 5–10% drift band the article promised. Both numbers, the txhashes, and the contract addresses are written into [`docs/BN254_BENCHMARK_RESULTS.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md). Anyone with a clean checkout can rerun either.

**Every change is minimal.** When we add BN254 to the wasm VM, we add exactly the BN254 host functions — nothing else. No refactoring. No "while we're here" cleanups. Marius's work on stabilising the Juno codebase is treated as a hard constraint, not a caveat. The v30 upgrade handler will do one thing and one thing only: bump the wasmvm version and register the new host imports. This is not timidity; it is the correct engineering posture when the cost of a bad change is a chain halt.

---

![A cross-section of an ancient cosmic tree trunk showing concentric growth rings, each ring glowing slightly brighter than the last, deep space root system visible below.](images/va_06_ethos.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, 1980s scientific illustration style, a cross-section of an ancient cosmic tree trunk showing five concentric growth rings each labeled with a faint Roman numeral, each ring glowing progressively brighter from core to edge, the outermost ring emits the most intense light, deep space root system sprawls below the cut surface into starfield, muted teal and gold palette, Moebius botanical plate cross-hatching, film grain, no text -->

## Who we build with

Trust in open-source is built slowly, through small correct acts, over long periods. We are at the beginning of that arc, not the end. But the arc is visible:

**Jake Hartnell**, Juno co-founder and WAVS architect at Layer.xyz, endorsed proposal #373 in March 2026 and said "do it" on the BN254 work. He added the phrase "as an experiment" to the HackMD — a co-edit that captured the right framing better than we had.

**Dimi**, validator and Juno security-patch steward, replied to our first public outreach with "will vote" on #374 and, when asked who would code the eventual chain upgrade, answered: *"Sure, I can help / review where needed."* That is the kind of offer a project either earns or doesn't. We intend to earn it by shipping the upstream CosmWasm PRs cleanly, not by moving fast.

**Marius**, former Juno core contracts developer, supplied the critique that motivates our entire module-surface analysis — and, when we floated the chain upgrade, reminded us: *"be careful with the implementation, I cleaned up the code base massively and made it stable."* This is the most important sentence anyone has said to us. It is now codified as a standing constraint in our development plan.

**Rattadan**, validator-ops collaborator, has shepherded our uni-7 orientation and validator outreach since day one.

**The Ffern Institute**, independent auditors, produced the April 2026 operator-side audit that seeded our v0.x.y-security-1 release.

We name each of these people for the role they actually played — because in a verifiable system, attribution is part of the data.

---

![Four figures standing on a vast cosmic plain at deliberate distances from each other, each one casting a different colored light behind them, all facing a single glowing point on the horizon.](images/va_07_who_we_build_with.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, late 1980s anime cel art style, four solitary figures standing on a vast luminous plain at deliberate distances from each other, each figure casting a different colored light aura behind them — teal, gold, amber, violet — all four facing a single bright point on the distant horizon, the plain reflects their colors like still water, Akira meets Nausicaä, ink outlines, no faces visible, film grain, no text -->

## Where it goes next

**Proposal #374 has crossed quorum.** As this is written: 33.5% turnout, 61.24% yes, zero no votes, 38.76% abstain. The abstentions converted. The chain spoke clearly. The sequence now is:

1. **Patch regeneration** — a clean rebase of our three cosmwasm patches onto latest upstream. One engineering session.
2. **Upstream issues** — we open issues, not PRs, on `CosmWasm/cosmwasm` and `CosmWasm/wasmvm`. Issues get feedback; feedback improves PRs. PRs that land without feedback often don't land at all.
3. **Upstream PRs** — minimal, tested, each accompanied by EIP-1108 test vectors and `cargo test` output. No refactoring. Reviewed by the cosmwasm core team (Ethan Frey, Simon Warta) and — if he has bandwidth — Dimi.
4. **v30 chain upgrade handler** — co-authored with Dimi, pattern-matched on his v28→v29 work, doing one job only: bump wasmvm, register BN254 imports. Tested locally against a v29.1 state dump before any PR opens.
5. **Upgrade governance proposal** — a non-signaling proposal with an explicit `--upgrade-height`, co-proposed with Dimi, rehearsed on uni-7 first.

Beyond BN254, the trajectory Jake sketched informally runs through Junoswap revival (forking Astroport contracts), Neutron protocol forks (Mars Protocol lending, Apollo yield vaults, Drop liquid staking), and WAVS as a replacement for missing Juno chain modules (ICQ, Cron, ContractManager). That roadmap is written up in `NEUTRON_FORK_STRATEGY.md`. It is ambitious; we ship it one piece at a time, with the same discipline we have applied so far.

---

![A path of luminous stepping stones stretching into a vast cosmic ocean, each stone slightly larger and brighter than the last, a small cloaked figure walking the path from behind, distant mountains of crystalline geometric shapes on the horizon.](images/va_08_where_next.png)

<!-- MIDJOURNEY PROMPT: hand drawn 2d illustration, late 1980s anime cel art style, a path of luminous stepping stones stretching across a vast cosmic ocean, each stone slightly larger and brighter than the last, a small cloaked figure seen from behind walks steadily forward along the path, distant mountains made of crystalline geometric shapes rise on the horizon, teal and deep amber gradient sky full of geometric constellations, Moebius linework, film grain, no text -->

## Why this matters for Juno

Cosmos chains have always been able to do what Ethereum could not: sovereign governance, sovereign economics, sovereign execution. What they have historically lacked is a coherent story for **verifiable off-chain compute**. EVM rollups solved this via massive cryptographic machinery; Cosmos chains mostly solved it by trusting validators or oracles.

JunoClaw is a proof that you can have sovereign verifiable compute on a Cosmos chain today, with one precompile, nine contracts, and a WAVS operator. The primitive that makes this possible is BN254; the primitive that makes it cheap is the precompile; the primitive that makes it trustworthy is the TEE; the primitive that makes it governable is the DAO.

Every Cosmos chain that cares about agents — which is every chain that wants to host AI, automated markets, cross-chain logic, insurance, credentialling — needs this. Juno can be the first.

---

## Why this matters for the broader community

We are not the first project to build DAO tooling on Juno. We may not be the last. What we hope to contribute, beyond the code, is a template for **how to build in public** on a chain that deserves it:

- **Ship small.** Every PR is narrow. Every change is tested. Every claim is reproducible.
- **Name people correctly.** Attribute what actually happened, not what sounds generous.
- **Publish corrections.** When we get it wrong, the correction is visible in the commit history.
- **Respect what came before.** Marius's cleanup is a gift to every builder on Juno. We do not undo it by moving fast.
- **Earn trust, don't claim it.** Jake's endorsement is an endorsement, not an authorization. Dimi's offer is an offer, not a partnership. The trust grows through repeated small correct acts, measured across months.

If you are a developer, a validator, a user, a curious observer — the repository is open at `github.com/Dragonmonk111/junoclaw`. The Apache-2.0 license applies throughout. The contracts are deployable. The frontend is runnable. The HackMD is readable. The audit is published. The proposal has passed.

*— VairagyaNodes, with Cascade (coding agent) as co-author of the contracts, and the gratitude of the whole project to Jake Hartnell, Dimi, Marius, Rattadan, the Ffern Institute, and the Juno validator community.*

*May 2026.*
