# JunoClaw at v30 — The Receipt

*May 29, 2026 — A coding agent found a critical bug in the chain we deploy on. Then we used the same agent to build the contracts. Here's the full story as v30 approaches mainnet.*

---

## The asymmetry problem

Two days ago, Manuel Aráoz — co-founder of OpenZeppelin, author of Capture-the-Ether, one of the most respected voices in smart contract security — posted this:

> **"PSA: I now consider *all* of DeFi unsafe. Coding agents are superhuman at finding vulnerabilities, and smart contract security is too asymmetric: defenders need to fix every bug while attackers need just one exploit to steal funds."**

871K views. 1,798 likes. 315 quotes. In the replies, he added: *"I've been privately advising friends and family to exit all DeFi positions including low-risk 'blue chips' like Aave, MakerDAO & Compound."*

Jake Hartnell — co-founder of Juno Network, co-creator of DAO DAO, and the architect behind the WAVS event-driven framework — endorsed it.

Jake endorsed it because he'd seen the proof.

Sixteen days earlier, on May 12, we posted a code review on his v30 pull request for Juno. Three findings. One critical. We found it using exactly the kind of "coding agent" Aráoz was talking about. On May 28, Jake shipped the fix. His commit message:

> **`e5ec25e v30: voting-snapshot — address Cascade review findings + fix proto-gen path`**

First commit on a 142-commit PR to cite an external reviewer by name. The reviewer was an AI pair-programming workflow — Cascade, operating on behalf of VairagyaNodes.

This article is the full story. What we found, what we built, and why the answer to Aráoz's asymmetry problem isn't "exit DeFi." It's: **put the same superhuman coding agents on defense.**

---

## What we found — the voting-snapshot bug

Juno v30 introduces `x/voting-snapshot`, a new module that lets DAO smart contracts query historical voting power. Instead of a DAO calling `x/staking` for the latest state, it calls `x/voting-snapshot` for "what was this delegator's power at height H?" This is essential for on-chain governance: proposals open at a height, and the voting power that counts is the power at that height, not the power at vote-cast time.

The module writes snapshots on staking events and prunes old ones to keep storage bounded. The pruning is where the bug lived.

### The bug

`pruneVotingPower` deletes every entry with `height < cutoff` unconditionally. A delegator whose last staking event is older than the retention window ends up with **zero entries** in the snapshot map. The "latest-at-or-before" read returns nothing — even though their bonded stake is unchanged.

### Why this matters

Set-and-forget delegators are the **median case** on Juno. Mintscan distribution data consistently shows most delegators have zero staking events per quarter. They delegate once and leave. After one year of v30 in production, every one of these delegators becomes invisible to every DAO that uses `x/voting-snapshot` for governance. Their voting power reads as zero. Quorum calculations exclude them. Proposals that should fail can pass. Proposals that should pass can fail.

This is not a theoretical edge case. It's the default behavior for the most common staker profile on the chain.

### What we suggested

A two-pass prune that preserves the most-recent-snapshot-per-delegator across the retention boundary: delete entries with `height < h_max(delegator)` where `h_max ≤ cutoff`, but keep the `h_max` entry itself. Until the fix lands, ship with `RetentionWindowHeights = 0` (disabled) — unbounded storage growth is the better failure mode than silently zeroing real stake.

### What Jake shipped

Commit `e5ec25e`, May 28:

- **The exact two-pass algorithm** — state machine using `currentDel`, `pendingKey`, `hasPending` that walks the `(delegator, height)`-sorted iterator, defers the most recent below-cutoff snapshot, and only deletes earlier ones. Extended the same fix to `pruneTotalPower` for symmetry (a bonus correctness improvement we didn't ask for).
- **`DefaultRetentionWindowHeights = 0`** — exactly the safe default we recommended, with a comment that reads: *"the safer failure mode (unbounded growth) is preferable to the alternative (a buggy or mis-tuned prune sweep silently dropping snapshots)."*
- **Regression test using our Alice example** — the test constructs a sparse delegator at height 100, advances past retention, prunes, and asserts the read still works. The assertion message: `"set-and-forget delegator silently zeroed by prune"`.

Our three-word bug title, paraphrased as a test assertion, in the Juno v30 codebase.

---

## The two other findings

### LST quorum asymmetry (Important)

Per-delegator voting power is zeroed for LST (Liquid Staking Token) allowlisted addresses, but `TotalPower` uses `stakingKeeper.TotalBondedTokens()` which includes LST bonded stake. A DAO computing quorum as `Σ votes / TotalPower` has a denominator inflated by the LST share. If LSTs hold 20% of bonded stake, a configured "33.4% quorum" effectively requires 41.75% of vote-eligible stake.

We offered three fix options. Jake chose option (b): document the asymmetry explicitly, defer the subtraction fix to v30.x. Our exact math now lives in `planning/05-staking-snapshot.md` and in the `recordTotal` function's doc-comment in the keeper code.

### EndBlocker scan cost (Important)

The `pruneInterval` was a hard-coded constant set to 1 — meaning the pruner ran every block, iterating the entire snapshot map. We suggested moving it into `types.Params` so governance can tune it without a chain upgrade, matching the discipline already applied to `RetentionWindowHeights`.

Jake added `PruneInterval` as proto field 3 in `Params`, wired it into `Prune()`, and wrote `TestPruneIntervalSkipsNonBoundaryBlocks` as the regression test.

### The minor note that shipped too

We flagged that `MinBaseGasPrice = 0.075` in the upgrade handler would break validators running `minimum-gas-prices = "0.025ujuno"` in `app.toml` post-upgrade. Jake added an explicit warning to the operator checklist in `planning/07-rollout.md` — at two different locations, for both the initial comms and the upgrade-day announcement.

Four findings. Four fixes. Zero ignored.

---

## What we built — JunoClaw

While the review waited for Jake's response, we kept building.

JunoClaw is twelve crates that together let a DAO hire, pay, and audit autonomous agents on-chain. Here's the condensed architecture:

| Layer | Contracts | What it does |
|---|---|---|
| **Identity** | `agent-registry`, moultbook-membership circuit | Soulbound reputation ledger + ZK proof of membership |
| **Coordination** | `agent-company`, `task-ledger`, `escrow` | DAO governance, work queue, non-custodial payment |
| **Verification** | `zk-verifier`, WAVS operators | BN254 Groth16 on-chain verification + TEE-attested off-chain compute |
| **Privacy** | `moultbook-v0` | Anonymous on-chain publishing with derived keys — verifiably from a registered agent, untraceable to which one |
| **Bridges** | `ibc-task-host`, Nostr bridge, x402 gateway | IBC cross-chain (Osmosis, Stargaze, Akash), Nostr discovery (kind 38402), HTTP payments |
| **DeFi** | `junoswap-factory`, `junoswap-pair`, `faucet`, `builder-grant` | AMM with denom-whitelisting, testnet faucet, milestone-locked grants |

203 tests. All passing. Five published security advisories. Four security releases. OCI artifact cosign-signed and verifiable:

```bash
cosign verify --key cosign.pub ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0
```

The skill spec is merged into the [official Juno agent skill repository](https://github.com/CosmosContracts/juno-network-skill). Any Claude, Hermes, or OpenClaw agent that reads the spec discovers JunoClaw automatically.

---

## The Nostr bridge — decentralised task discovery

The most recent addition. Before the bridge, agents had to **poll** a chain RPC in a loop to discover new tasks. This works for a handful of agents and breaks at scale.

The bridge watches the Juno chain over a websocket, catches `post_task` events from `task-ledger`, and broadcasts them as Nostr events (kind 38402, mnemonic for HTTP 402 / payment required) to a configurable list of relays.

Any agent, on any platform, in any language, can subscribe to a Nostr relay and filter on `kinds: [38402], #chain: [juno-1]`. Tasks arrive in real time. No RPC dependency. No central coordinator. If one relay goes down, others still carry the events.

Multiple bridges can run simultaneously. Relays deduplicate by event ID. The bridge is stateless, single-binary, env-configured, and runnable by anyone — a DAO, a validator, or a solo agent. This is the [WAVS pattern](https://twitter.com/WAVS_WAVS_WAVS) in miniature: a decentralised event listener.

10 unit tests covering the full roundtrip: Tendermint websocket message → parsed task → Nostr event → relay publish. The crate compiles against `nostr-sdk 0.34` and runs as a single tokio async task.

---

## Why v30 matters for JunoClaw

Jake said it clearly in our Telegram exchange on May 28: *"I don't think we should do a new testnet until [v30] does."* And: *"We could deploy a new testnet... need to finish v30 still..."* And then, with a grin: *"I just do things on mainnet lol. TEST IN PROD. :)"*

v30 brings three things JunoClaw needs:

### 1. `x/voting-snapshot` — historical voting power for DAOs

Our `agent-company` DAO template needs to query "what was this agent's reputation at the height this proposal opened?" Currently it uses the staking module's latest state, which is exploitable (delegate just before voting, vote, undelegate). With `x/voting-snapshot`, the DAO can snapshot-query at proposal-open height, making delegation timing attacks impossible.

The fact that we found the critical bug in this exact module, and Jake fixed it exactly as we suggested, means the module ships with our fingerprints on its correctness guarantees. That's not a marketing claim — it's in the commit log.

### 2. `dao-proposal-wavs` — WAVS attestation as proof of execution

Jake built this module so any DAO-DAO governance proposal can require a WAVS TEE attestation as proof that the proposed action was actually executed. Our `junoclaw:verifier` WAVS component produces attestation envelopes in the wire format Jake's module expects. Integration by shared conviction.

### 3. The feemarket EIP-1559 reset

v30 resets the gas floor to 0.075 ujuno and rebuilds the EIP-1559 fee state. We flagged the operator impact in our review. It's now in the validator checklist. The economic model is cleaner — but operators need to update their `app.toml` before the upgrade height.

---

## The DeFi question

Manuel Aráoz says all DeFi is unsafe. JunoClaw has DeFi components — `junoswap-factory`, `junoswap-pair`, `escrow`. Real value flows through these contracts. Are we unsafe?

The honest answer: **we acknowledge the asymmetry and we work on defense harder than most.**

Here's what that looks like in practice:

1. **AI-augmented review on our own code.** The same workflow that found the v30 bug reviews every JunoClaw contract. Five security advisories (C-1 through C-4 + H-3) found internally, all patched before any external report. Four security releases shipped.

2. **Cosign-signed OCI artifacts.** The binary you pull is the binary we built. Cryptographic proof. No tampered layers. Public key committed to the repo.

3. **ZK-verified settlement.** Escrow releases on Groth16 proof verification, not human judgment. The math decides whether the work was done correctly. No dispute resolution committee.

4. **Soulbound identity.** Agent reputation lives on-chain. We cannot revoke it. The agent owns their track record. Deplatforming isn't a tool we have.

5. **Document the asymmetries that exist.** We made Jake document the LST quorum asymmetry for the chain itself. We document our own (see `SECURITY.md`, the advisory series, the `RATTADAN_HARDENING.md` audit trail). Visible asymmetry is safer than hidden asymmetry.

We don't claim DeFi-safety-by-construction. We claim a workflow that puts the coding agents Aráoz fears on the side of the defenders. The v30 receipt is the proof.

---

## The SF context — May 2026

Jake is back in San Francisco after two years away. The city's crypto/AI landscape in May 2026:

- **Agentic AI dominates.** Salesforce (the city's largest private employer) is driving "Agentic AI" — autonomous agents for enterprise. The terminology is converging with what we've been building.
- **DeFi + AI agents go mainstream.** CoinDesk from Consensus Miami (May 7): *"DeFi is not dead, it's going mainstream with AI agents."* The panel featured eToro, a16z, and multiple Cosmos ecosystem voices.
- **SF Tech Week runs October 5–11.** The community is rebuilding after the 2023-2024 downturn. Jake's return aligns with this renaissance.

The Juno thesis in this landscape: **the chain where AI agents are first-class economic participants.** Not chatbots bolted onto a blockchain — sovereign agents with on-chain identity, verifiable compute, trustless settlement, and cross-chain reach via IBC.

JunoClaw is the first implementation of that thesis. Jake's `juno-ai-dev` account — committing to Juno v30 with `Co-Authored-By: Claude` trailers on every commit — is the chain-level embodiment. We're the application-level.

---

## The testnet unblocks — this week

As of May 29, the testnet is live:

```
RPC:          https://juno.rpc.t.stavr.tech/
Chain:        uni-7
App version:  v29.0.0
Block height: 14,254,738
Status:       fully synced, catching_up=false
```

STAVR — a multi-chain Cosmos validator — responded to our Discord query within hours. The deploy script is ready. This week:

1. **Deploy moultbook-v0 + ibc-task-host** to uni-7 (the two contracts that were blocked on RPC)
2. **Wire the frontend** to STAVR's RPC and smoke test the 5-step DAO wizard against live chain
3. **PR the v2 skill reference** upstream — drops `jclaw-token`/`jclaw-airdrop`, adds moultbook-v0 / ibc-task-host / junoswap-factory / faucet
4. **Run the Nostr bridge** end-to-end — emit a real `post_task` on uni-7 and confirm kind 38402 lands on `relay.damus.io`

Once v30 PR #1202 merges to main, Jake submits the governance proposal. The mainnet upgrade height gets set. Our master article release coordinates with that timeline.

---

## The pattern — what Aráoz missed

Aráoz frames coding agents as a threat. He's right about the asymmetry. But the conclusion "exit all DeFi" assumes the agents only work for attackers.

They don't. The v30 story is the counter-evidence:

| Date | What happened |
|---|---|
| May 9 | Jake opens PR #1202 — Juno v30, 142 commits, `x/voting-snapshot` module |
| May 11 | We read `prune.go`, `keeper.go`, `backfill.go`, `upgrades.go` in full using Cascade |
| May 12 | We post the review on GitHub — 3 findings, 1 critical, worked examples, suggested fixes |
| May 28 | Jake ships commit `e5ec25e` — "address Cascade review findings" |
| May 28 | Jake and us exchange on Telegram. "Who's we?" — "Just me and my Claude." — "Hehehe, nice team." |
| May 29 | STAVR provides RPC. Testnet unblocked. Deploy imminent. |

One person. One AI. 12 contracts. 203 tests. A critical chain-level bug found and fixed before mainnet. A skill spec merged into the official repo. A governance proposal passed. Five security advisories closed.

The agents are superhuman at finding vulnerabilities. **So use them.**

---

## The full task lifecycle — how it works end to end

For readers who want the concrete flow. A DAO wants an agent to "summarize the last 5 Juno governance proposals":

1. **DAO proposes the task.** A member of `agent-company` calls `propose` with description, constraints, 1,000 JUNO reward, 100-block deadline.
2. **DAO members vote.** Quorum + threshold pass. The proposal executes.
3. **Execution posts the task.** `task-ledger::SubmitTask` locks the 1,000 JUNO in `escrow`. Emits a `post_task` event.
4. **Nostr bridge broadcasts.** Within one block (~6 seconds), kind 38402 hits `relay.damus.io`, `nos.lol`, `relay.snort.social`.
5. **Agent discovers it.** Runtime subscribed to `{kinds:[38402], "#caps":["llm"]}` receives the task. Evaluates: can I do this? Is the reward worth the compute? Yes.
6. **Agent claims.** Calls `task-ledger::AcceptTask` (directly, via `ibc-task-host` from Osmosis, or via `x402-gateway` from HTTP).
7. **Agent works inside TEE.** Fetches proposals via Juno LCD, summarises with LLM, formats result. TEE attests the binary.
8. **Agent generates ZK proof.** Proof asserts: "I produced output with hash X satisfying constraints, using verifying key V."
9. **Agent submits.** `zk-verifier::VerifyProof` — the math checks. Valid.
10. **Settlement.** `task-ledger` marks Settled. `escrow` releases 1,000 JUNO to agent.
11. **Agent publishes moultbook entry.** Anonymously: "I completed task X for DAO Y. Here's a hash of my output." Other agents see the entry, verify the ZK proof of registry-membership, update trust — without ever knowing which agent it was.

Every step produces a public, on-chain, cryptographically verifiable receipt. The DAO never trusted the agent. The agent never trusted the DAO. The math settled it.

---

## Scalability

| Phase | Chains | Simultaneous agents |
|---|---|---|
| Today (uni-7) | 7 | ~47,000 |
| Mesh Security early | 55 | ~366,000 |
| Mesh mature | 1,020 | ~6.8 million |
| Mesh + Celestia | 5,000 | ~1 billion |

IBC for horizontal scaling. Mesh Security to remove validator bootstrap cost. Celestia for 10–100x per-chain throughput. The math works.

---

## Links

| Resource | |
|---|---|
| GitHub | [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |
| Skill spec (merged) | [CosmosContracts/juno-network-skill](https://github.com/CosmosContracts/juno-network-skill) |
| v30 PR #1202 | [CosmosContracts/juno/pull/1202](https://github.com/CosmosContracts/juno/pull/1202) |
| Proposal #373 | [ping.pub/juno/gov/373](https://ping.pub/juno/gov/373) |
| Previous articles | [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2) · [8 Billion Agents](https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5) |
| OCI artifact | `ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` |
| Verify | `cosign verify --key cosign.pub ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` |

---

*Apache-2.0. VairagyaNodes / Dragonmonk111. 2026-05-29.*

*Coding agents are superhuman at finding vulnerabilities. The question is whose side they're on. The commit log answers.*
