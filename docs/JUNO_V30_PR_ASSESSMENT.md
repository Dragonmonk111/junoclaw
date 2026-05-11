# Juno v30 PR #1202 — Assessment, learning points, and where it puts us

*Owner: VairagyaNodes. Source PR: https://github.com/CosmosContracts/juno/pull/1202. PR opened: 9 May 2026 by `JakeHartnell`, branch `jakehartnell/v30`. Assessment date: 11 May 2026.*

---

## 1. The headline

Jake shared the v30 draft tonight, after reading and approving *The Tenth Contract* (Moultbook integration article). The PR is the actual v30 — 114 commits, code-owners requested for review (`dimiandre`, `niilptr`, `vexxvakan`). The PR description is authored by **Juno AI**, an autonomous agent operating under Jake's mandate with its own GitHub identity (`juno-ai-dev`) and a `Co-Authored-By: Claude Opus 4.7 (1M context)` trailer on every commit. Crucially for us, Jake's PR description names our BN254 work explicitly:

> *"The wasmvm v3 jump is the consensus break that justifies a v30; BN254 precompile lands with it (prop #374)."*

This is the strongest possible public signal that proposal #374 (which passed 80% YES on 5 May) is on the v30 timeline. It also makes the wasmvm forward-port decision (Track A vs Track B in our `POST_VOTE_EXECUTION_PLAN`) suddenly load-bearing.

## 2. Stack: what's actually shipping

From the `go.mod` on the `jakehartnell/v30` branch:

| Component                | Pre-v30 (v29.1)     | v30 (PR #1202)                  |
|--------------------------|---------------------|---------------------------------|
| Go                       | 1.23.x              | **1.25.2**                      |
| Cosmos SDK               | v0.50.x             | **v0.53.7**                     |
| wasmd                    | v0.54.0             | **v0.61.11**                    |
| wasmvm                   | v2.2.4              | **v3.0.4** (major-version jump) |
| ibc-go                   | v8.x                | **v10.6.0**                     |
| cometbft                 | v0.38.x             | v0.38.23                        |

Path-B-deferred-to-v31 in Jake's words: *"The originally-planned Path B (SDK v0.54 / ibc-go v11 / store/v2) is blocked on ibc-apps publishing a /v11 line and is sequenced for v31. Full rationale in planning/02-targets.md."*

**The wasmvm v2→v3 jump is THE consensus break.** That single bump justifies the major version (rather than v29.2). Everything else cascades from it.

## 3. New chain code in v30 (beyond the dep bump)

The PR adds three significant new code paths, and removes two dead ones:

**New: `x/voting-snapshot`** — Jake's own words:

> *"Chain-side historical staking-power queries so DAO DAO voting modules can ask 'what was this address's bonded power at proposal-open height?' and get a stable answer that doesn't drift if voters rage-stake mid-window. Wasmbinding + gRPC + REST surfaces. End-to-end smoke against a local devnet verified hook → snapshot → query round-trips correctly."*

In `app/upgrades/v30/upgrades.go` there is a `BackfillFromStaking` upgrade-time migration that seeds the snapshot store from current bonded-power state. Jake names this as the highest-attention area for external review.

**New: `x/cw-hooks`** — surfaced via CodeQL review comments fixing a determinism issue (sort by `ContractAddress` when materializing genesis slices in `NewGenesisState`). The module exposes contract-side hooks; details require reading the branch.

**New: `x/stream`** — a streaming module with `subscription_registry`. CodeQL flagged log-injection false positives that Juno AI is reimplementing the `SanitizeLog` helper around `strings.NewReplacer` (canonical CodeQL sanitizer) to satisfy.

**Removed: ICS-29 (feeibc) and async-icq (interchainquery)** — store deletions. No live counterparties on juno-1 mainnet per `planning/ASYNC-ICQ-AUDIT-V30.md`; Jake invites external sanity-check.

**Also new (test-config, not consensus):** `consensus.params.block.max_gas` bumped 5M → 25M for wasm-store headroom in interchaintest. `feemarket.MaxBlockUtilization` unchanged at 5M (dynamic-fee target, not consensus cap).

## 4. Where our BN254 work stands relative to PR #1202 — the critical question

Per `POST_VOTE_EXECUTION_PLAN.md` (most recent state):

- **Track A** (wasmvm v2.2.x / cosmwasm v2.2.x):  ✅ **complete**. 10 patches at `wasmvm-fork/patches/v2.2.2/`. `cosmwasm-crypto-bn254` 22/22, `cosmwasm-vm --lib` 311/311. Forward-ported to v2.2.7 on 2026-05-10 (commit `a9dd318`).
- **Track B** (wasmvm v3.0.x / cosmwasm v3.0.x):  ⏸ **deferred**, pending maintainer feedback on which line they want the upstream PR against.

The v30 `go.mod` shows:

```go
github.com/CosmWasm/wasmvm/v3 v3.0.4
```

…with **no `replace` directive** pointing to our fork. So one of three things is true:

**(a)** Juno AI / Jake plans to coordinate an upstream merge of BN254 into wasmvm `v3.0.x` (presumably bumping v30 to whatever release line carries it).
**(b)** Juno AI / Jake plans to add a `replace` directive to our `VairagyaNodes/wasmvm-fork` once Track B is complete.
**(c)** "BN254 lands with v30" is aspirational — meaning *we* are expected to forward-port (Track B) and either get it upstream into wasmvm `v3.0.x` or wire it as a replace directive, before v30 stabilises for mainnet.

Without explicit signal we should assume **(c)**, because that puts us in control of the timeline rather than waiting on coordination that may not happen. **This makes Track B critical-path.** A direct DM to Jake / Juno AI to clarify (a) / (b) / (c) is the first action item.

## 5. Composability gift: Moultbook × `x/voting-snapshot`

This was not on our radar three days ago and is genuinely useful.

`x/voting-snapshot` exposes "what was this address's bonded power at height H?" to smart contracts via wasmbinding. Moultbook entries already carry `posted_at: Timestamp`. The composition: a Moultbook reader can join an entry's `posted_at` height against `x/voting-snapshot`'s power-at-height query to compute a **stake-weighted reading** of any citation DAG — e.g. "of the agents that cited Charles's review, what fraction of bonded JUNO power did they collectively represent at the time of their posts?"

Three concrete patterns this enables post-v30:

1. **Stake-weighted reputation.** `ListByAuthor { author }` returns an author's entry history; `x/voting-snapshot` lets a reader compute that author's average bonded power across their post times. A useful spam filter for agent-fleet outputs that does not require either DENS or admin moderation.
2. **Stake-weighted citation rank.** Two entries that cite an anchor — `ListByRef { ref_id: anchor }` — can be ordered by the citer's bonded power at the time of citation. The orchestrator-level ranking is read-only and can be done off-chain.
3. **Provable governance audit trails.** A governance proposal that references prior agent-output Moultbook entries can simultaneously prove (via `x/voting-snapshot`) that the relevant stakers had power at proposal-open height. The two substrates compose into a single auditable record.

**Action item:** the next-but-one Moultbook article should sketch §5 as a forward use case. *Not* in this assessment doc's scope to design fully; flag it for a follow-up note once Track B is moving.

## 6. Verifiable-agent thesis — confirmed in production

Juno AI's commit signature pattern is, in concrete form, the thesis of *The Verifiable Agent* (3 May article) and *The Tenth Contract* (10 May article) and *The Number That Made It Real* (10 May article):

- The agent has its own identifier (GitHub `juno-ai-dev`) distinguishable from its principal (Jake)
- Every commit carries a `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` trailer that attributes the model behind it
- The agent operates under a publicly-acknowledged mandate (Jake's PR description names it)
- The agent's outputs are routed through standard review tooling (the PR is open to `dimiandre`, `niilptr`, `vexxvakan`) and CodeQL static analysis — not bypassing the human review path

This pattern is what Moultbook is designed to be the on-chain substrate for. Today the trail of Juno AI's work lives in GitHub commit messages and PR comments. Tomorrow it can live in Moultbook entries with `AttestationRef::ZkProof { ... }` proving the analysis was produced inside a TEE or verified pipeline, addressable by `moult:` id, walkable via citation DAG.

For framing purposes: every future Moultbook article should reference Juno AI's identity pattern as the existing real-world precedent. We are no longer making a forward case; we are providing on-chain rails for a pattern Jake is already running.

## 7. Where Jake explicitly wants external eyes (and what we can credibly review)

From the PR description:

1. **`app/upgrades/v30/upgrades.go` — `BackfillFromStaking` migration.** We have read v28→v29 upgrade handlers as part of our v30 design pattern-matching. We can credibly review whether the migration writes are idempotent, whether they degrade gracefully on partial-state replay, and whether the genesis-time vs upgrade-time semantics match.
2. **`x/voting-snapshot/` in full.** Every path is new code. Our credible angle: review the wasmbinding gas charge (Jake names "5000" as an open question), and the snapshot-retention default (Jake names "~1y" as an open question).
3. **ICS-29 + async-icq store deletions.** Lower priority for us; Marius or any IBC-experienced reviewer is the right reviewer here. We can sanity-check that mainnet juno-1 has no live counterparties, which the audit doc already claims.
4. **`planning/SECURITY-REVIEW-NOTES-V2.md` open questions** — snapshot retention default, wasmbinding gas charge, pre-upgrade query semantics. We can offer measured opinions but the operator pool (Dimi, niilptr, vexxvakan) has stronger context for the chain-config calls.

The dev-collab discipline from `MOULTBOOK_DEV_COLLABORATION_NOTES.md` §4 applies here verbatim: when we post a code review, it should be anchored against a specific commit hash so the review is durable and citable.

## 8. What Marius's "no new modules" warning means in context

Our `V30_UPGRADE_HANDLER_DESIGN.md` was drafted around Marius's binding constraint: *"Be careful with the implementation, I cleaned up the code base massively and made it stable. No state migrations beyond RunMigrations, no param changes, no new modules."* That document was useful as a Marius-constrained interpretation of what v30 *could* be — the minimal version we would have authored for Dimi to co-sign.

Jake's actual v30 is **broader**. It does the wasmvm bump (Marius-acceptable), but it also adds three new modules, deletes two dead ones, and contains a non-trivial state migration. This is a Jake-scoped v30, not a Marius-scoped v30.

The implication is not that Marius's constraint was wrong — it was a sound boundary for *us* to propose, given we did not have Jake's backing or Juno-AI engineering capacity. The implication is that **our role on v30 shifts from author to reviewer**. Specifically:

- We do not write `app/upgrades/v30/upgrades.go`. Juno AI has.
- We do not co-author with Dimi as a peer-validator team. Dimi reviews Jake's PR as the code owner.
- We *do* bring BN254 to v30 (Track B), if (c) in §4 is the right reading.
- We *do* provide external eyes on the four areas Jake names in §7.
- We *do* watch the wasmvm v3.0.4 dependency line for downstream BN254 wiring.

That is a different, narrower, but still substantive role. It also makes the dev-collab use case from yesterday's notes immediately actionable — Moultbook (once deployed) becomes the natural place to post the review threads.

## 9. Action items

In rough priority order:

1. **DM Jake / Juno AI to clarify §4 (a)/(b)/(c).** "We saw v30 names BN254 prop #374 — is Track B (wasmvm v3 forward-port) on our side, on yours via upstream wasmvm v3.0.x merge, or are we coordinating a `replace` directive in the v30 `go.mod`?" One sentence; the answer determines the next two days of work. Reply expected within a working day given the warm engagement context.
2. **If §4 (c) confirmed: start Track B.** Forward-port the 10 patches at `wasmvm-fork/patches/v2.2.2/` onto `cosmwasm v3.0.1` and `wasmvm v3.0.4`. Expected timeline: 3–5 working days based on the v2.2.0→v2.2.2 patch-level rebase taking ~6 hours; a major-version rebase is ~10× harder. We pin Rust 1.78 for v2; v3 likely wants a newer toolchain — re-pin in a fresh `rust-toolchain.toml`. Track A patches and verification scaffolding (`rebase-track-a.sh`, the `cosmwasm-vm` 311/311 baseline) carry directly over.
3. **Stand up Moultbook devnet deploy in parallel.** Independent of BN254 / v30 sequencing. Produces concrete gas measurements (the ADR projects 40–60k SDK gas per Post; measurement validates or moves the projection). Build script + deploy script + smoke test land in `devnet/scripts/`. This is the natural "keep building from where we left" continuation of the last session.
4. **Post a Moultbook-style code review on PR #1202** once we have read the actual diff — specifically: `app/upgrades/v30/upgrades.go` (migration idempotence) and `x/voting-snapshot/` (gas charge sanity, retention default). The review goes on GitHub today; once Moultbook is on devnet, the same review is posted as a Moultbook entry with the commit hash as anchor, demonstrating the dev-collab pattern in action.
5. **Draft a "next-Moultbook-article" outline** referencing the §5 composability gift (Moultbook × `x/voting-snapshot`) and §6 verifiable-agent confirmation. Not for publication yet; just the outline saved as a working note. Trigger condition: Moultbook devnet deploy is live and produces a real `moult:` id we can quote.
6. **Update `POST_VOTE_EXECUTION_PLAN.md` Phase 3** with a brief insert noting PR #1202's existence and the implication for Track B timing. One paragraph; do not bloat the doc.

## 10. What is *not* changing

- The Moultbook v0 contract surface. Already complete, tested 12/12, committed. No revision needed in response to v30.
- The framing of `MEDIUM_ARTICLE_MOULTBOOK_INTEGRATION.md` (now published). Stays as-is on Medium; local copy carries the §6 dev-collab pointer.
- The Marius-constrained `V30_UPGRADE_HANDLER_DESIGN.md`. Kept as a record of the constrained version we drafted; mark `Status:` to reflect that Jake's broader v30 has superseded the author-track plan. (A two-line update, in the next commit.)
- The BN254 Track A work. Stays as the production-ready fallback if Track B slips. We do not abandon it.

---

*Apache-2.0. This assessment is a working note and may be split into discrete follow-ups as actions in §9 land. The PR description quoted is public-facing on GitHub under JakeHartnell's authorship; the Juno AI's identity pattern is documented in the PR description itself. Captures my read as of 11 May 2026 21:00 BST; any subsequent commits to `jakehartnell/v30` may move the picture.*
