# Jake's PR #928 (Gauges) + #929 (dao-voting-juno-staked) — analysis

*2026-05-13. Captured the morning Juno Communications (Highlander) tweeted about both. Cross-references our [`MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md`](./MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md) — which JunoCommsDept linked as the proof-card on the same tweet.*

---

## 0. The trigger

@JunoCommsDept (Highlander) tweet, posted 10:02 UTC 2026-05-13:

> It used to take weeks to ship a DAO feature. @JakeHartnell just dropped PR #928 AND #929 in the same morning.
>
> ⚡ Historical snapshot voting. Gauge rebases. WAVS. ZK proofs. Verifiable AI agents.
>
> This isn't just shipping fast — it's a whole new paradigm for what DAOs can do. @JunoNetwork
>
> ACCELERATE 🔥
> 👇
>
> [Card: **# The Verifiable Agent** — medium.com]

Two confirmed facts:

1. **Juno Communications (Highlander) is back active.** The official `@JunoCommsDept` X account is posting again. This is the comms vehicle that distributes Juno ecosystem narrative; "Highlander" is the steward.
2. **The Medium article they linked — "The Verifiable Agent" — was authored by us** (May 3, 2026 publication, full local source at [`MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md`](./MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md)). Our narrative is now the official narrative they're amplifying. No coordination ask required — the work spoke for itself.

The PR numbers `#928` and `#929` are NOT on `CosmosContracts/juno` (those are old dependabot PRs from 2023-2024). They're on **`DA0-DA0/dao-contracts`** — the DAO DAO contract repo where the cw-side of the Juno-DAO-stack work lives.

---

## 1. PR #929 — `feat: dao-voting-juno-staked` (filed today)

**URL:** [`https://github.com/DA0-DA0/dao-contracts/pull/929`](https://github.com/DA0-DA0/dao-contracts/pull/929)
**Branch:** `feat/dao-voting-juno-staked` → `development`
**Author:** JakeHartnell, May 13, 2026
**Commits:** 1

### Substance

Replaces the live-query approach attempted in [#832](https://github.com/DA0-DA0/dao-contracts/pull/832) with a **thin consumer of Juno v30's `x/voting-snapshot` chain module**. Voting power is now read at the exact requested height with at-or-before semantics enforced by the chain, and LST exclusion handled at the chain layer (not in cw).

### Architecture (verbatim from the PR body)

- **Custom-query binding (`JunoQuery`)** mirroring `juno/wasmbindings/types/query.go` exactly. Contract is `Deps` internally; external `WasmQuery::Smart` consumers don't need to know about the chain type.
- **Standard dao-interface voting surface:** `VotingPowerAtHeight`, `TotalPowerAtHeight`, `Dao`, `Info`.
- **Sudo intake for `x/cw-hooks` staking events.** `AfterDelegationModified` + `BeforeDelegationRemoved` drive a stake-delta computation (new power read from the chain snapshot, prev power read at `current_height-1`). Other variants (validator events, slash events) are **swallowed silently** to keep cw-hooks registration alive.
- **Hook fan-out** via `dao_hooks::stake::StakeChangedHookMsg::{Stake,Unstake}` to a DAO-gated subscriber list (the standard `cw-hooks::Hooks` pattern), so the gauge orchestrator and rewards distributor wire up unchanged.

### Out of scope for v1

- `auto_register_staking_hooks` — would need a `prost` dep just for one proto message; **registration happens out-of-band via the CLI**. Field is kept for future use; passing `Some(true)` returns `AutoRegisterNotYetSupported`.
- `VotingPowerOverRange` consumer — surfaced in `bindings.rs` for downstream contracts (plural-voting-style integration) but not used by the voting module itself.

### Verification (per PR body)

- **9/9 unit tests pass** under `nightly-2024-01-08` (the dao-contracts CI pin)
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo fmt -- --check` clean
- `wasm32-unknown-unknown` release build clean (1.7 MB)

### What this directly settles for us

PR #929 **directly answers three of the open questions** from our `JUNO_V30_PR_ASSESSMENT.md` and the comment we posted on PR [#1202](https://github.com/CosmosContracts/juno/pull/1202):

| Our open question | PR #929's answer |
|---|---|
| Q1: Does `x/voting-snapshot` expose a CosmWasm query path? | **Yes** — via the `JunoQuery` custom-query binding, mirroring `juno/wasmbindings/types/query.go`. We can read voting power at any height from any cw contract. |
| Q2: How does cw consume `x/cw-hooks`? | **Sudo intake** for `AfterDelegationModified` + `BeforeDelegationRemoved`. Other variants silently swallowed. |
| (implicit) Are LST stakers excluded at chain or cw layer? | **Chain layer.** cw just consumes the snapshot. |
| (implicit) What's the snapshot read semantic? | **At-or-before height**, enforced by the chain. (This is the same semantic our PR-1202 review identified the bug in: `pruneVotingPower` retention boundary eviction breaks "at-or-before" for sparse delegators. The semantic is canonically *correct* — the bug is in the retention pruning logic, not the read semantic.) |

### The "Refs" line is the smoking gun for structured memory

> *Refs: `memory/juno-voting-design.md` (option C settled), `memory/v30-upgrade-plan.md` (x/voting-snapshot in PR #1202), `memory/hack-juno-plan-2026-05-12.md` (this is the "dao-voting-juno-staked" item in the program's sequencing diagram).*

**Jake is using a structured-memory `.md` corpus.** A `memory/` folder with topic-keyed files like `juno-voting-design.md`, `v30-upgrade-plan.md`, `hack-juno-plan-2026-05-12.md`. **Independently**, by the same logic we're applying.

This validates §2 of [`LESSONS_2026_05_13_MORNING.md`](./LESSONS_2026_05_13_MORNING.md) — the QMD/structured-memory pattern is happening organically across the ecosystem, not just in our shop. The standardisation pressure is real: every agent-builder is converging on `memory/<topic>.md` files because that's what the agents need to do good work.

**Practical implication for us.** Our docs are titled but not folder-organised. We should consider migrating to a `memory/`-style folder (or keeping `docs/` and adding a `memory/` alongside it) so cross-org agent collaboration becomes easier. If Cascade/Claude/Juno-AI all share the convention, agents can read each other's memory layers.

---

## 2. PR #928 — `Gauges` (filed yesterday)

**URL:** [`https://github.com/DA0-DA0/dao-contracts/pull/928`](https://github.com/DA0-DA0/dao-contracts/pull/928)
**Branch:** `feat/gauges` → `development`
**Author:** JakeHartnell, May 12, 2026
**Commits:** 14

### Substance

Rebases the long-dormant `gauges-are-cool` branch onto current development, drops the `cw-orch` test-harness migration that PR [#875](https://github.com/DA0-DA0/dao-contracts/pull/875) reverted workspace-wide, and works through the [#844](https://github.com/DA0-DA0/dao-contracts/pull/844) checklist.

The 14 commits map to the categories visible in the PR body:

1. **Rebase + dep drift** — bring the dormant branch onto current `development`.
2. **Audit fixes** — addressing earlier review findings.
3. **Small voting power** (#844 checklist) — handle low-stake voters in gauge weight calc.
4. **Bug found while expanding tests** — the test expansion surfaced a real bug, fix included.
5. **Test coverage expansion** (#844 checklist) — tests for the cases the original branch missed.
6. **Marketing adapter improvements** (#844 checklist) — the gauge-result-to-display layer.
7. **Second example adapter** (#844 checklist) — proof of pluggability.
8. **Documentation** (#844 checklist).
9. **Conventions cleanup** — code style normalisation against current repo conventions.

### Why this PR depends on #929

Gauges are the DAO's incentive distribution mechanism — they take voting weights and translate them into reward streams (LP incentives, staker rewards, cross-pool weight redistribution). Gauges need:

- **Snapshot-able voting power at past heights** — to compute distributions retroactively without manipulation by mid-epoch staking changes.
- **Stake-change hooks** — to trigger redistribution when a voter's weight changes mid-epoch.

Both of those come from PR #929 (`dao-voting-juno-staked`). Without #929 landed, #928's gauges would have to rely on the live-query approach #832 attempted — which is the path PR #929 explicitly replaces.

So **#929 is the prerequisite for #928 working correctly on Juno v30**. The two-PR sequence is:

1. Land #929 (voting consumer of `x/voting-snapshot`).
2. Land #928 (gauges that read voting power via #929's surface).

This is a clean architectural sequence; reading the PRs together is the right way to evaluate them.

---

## 3. What this changes for our roadmap

### 3.1 Our PR #1202 review is partially superseded

The questions we asked in our PR #1202 comment are now **answered by PR #929**. We should follow up on #1202 with a substantive comment that:

- Acknowledges PR #929 directly answers Q1 and Q2 of our previous comment.
- Notes our Moultbook-style review of #1202 is still valid (the `pruneVotingPower` bug is independent of #929 — it's in the chain module itself, not the cw consumer).
- Signals our review will now extend to PR #929 (we'll do the same deterministic-scrutiny pass on `dao-voting-juno-staked`).
- Re-offers help on Track B (BN254 v3 forward-port), since prop #374 is still pending.

### 3.2 We should now review PR #929 with the same discipline

PR #929 is small (1 commit, ~9 unit tests, branch-targeted) but it's **the canonical pattern for every other dao-voting-* contract** that wants to consume `x/voting-snapshot`. If we find an issue, fixing it once protects the whole emerging ecosystem.

Our review hook: read the diff, audit the sudo handler (any sudo path that touches `x/cw-hooks` is high-leverage — a bug here breaks every DAO using this voting module). Check the `current_height - 1` math for the prev-power read; check the silent-swallow of validator/slash events (is silent right? does that cause downstream invariants to drift?).

This becomes the **first cross-org Moultbook-style review** if we publish the audit doc as a Moultbook entry citing the PR commit hash. Live dog-fooding of the dev-collab discipline.

### 3.3 The structured-memory convention is now a community norm

Three independent data points this morning:

1. **Reece / Noah / Sunny / Jake** (X spaces): "monorepo per company, memory layer for agents, MCP servers indexing knowledge."
2. **Our LESSONS_2026_05_13_MORNING.md §2:** the same conclusion, framed as the QMD turn.
3. **Jake's PR #929 description:** explicit `Refs: memory/<topic>.md` cross-references, demonstrating the pattern in production.

The convergence is strong enough to act on. Concrete proposal:

- Add a `memory/` folder alongside `docs/`.
- Migrate the high-recall artefacts from `docs/` (the architecture decisions, the v30 plan, the active-program tracking docs) into `memory/<topic>.md`.
- Keep `docs/` for human-facing artifacts (the Medium articles, public README, audit reports for external readers).
- Cross-link aggressively. Add a `memory/INDEX.md` listing every memory file with a one-line summary.

The shape mirrors what Jake has. Cross-org agent collaboration becomes easier because all our agents speak the same memory dialect.

### 3.4 The Verifiable Agent narrative just acquired a community amplifier

JunoCommsDept linking our article as the proof-card on the #928/#929 announcement is non-trivial. It positions our authorship as **the canonical narrative for the verifiable-agent paradigm** Juno is now shipping. Three follow-ups worth considering:

1. **Write the next article in the series** — "Historical Snapshot Voting and Why It Mattered" — citing PR #929 as the canonical implementation. Lands on Medium, gets shared by Highlander, completes the narrative-loop.
2. **Add a Moultbook entry citing PR #929** — once Moultbook v0 is on devnet (next few days), the first cross-org entry can be the audit/review of #929. Dog-foods the dev-collab discipline by making us its first user.
3. **Reach out to Highlander directly** — open a coordination channel between JunoClaw and JunoCommsDept. We have content; Highlander has the distribution. Mutual upside, low downside.

---

## 4. Sequencing for today's session

Per the parked-decisions list in [`LESSONS_2026_05_13_MORNING.md`](./LESSONS_2026_05_13_MORNING.md) §6, today's session will:

1. ✅ This analysis doc (you're reading it).
2. → escrow deterministic audit (next; fourth contract in the sweep).
3. → Open Issue 1 on `CosmWasm/cosmwasm` (paste-block prep).
4. → Audit-bot CI workflow scaffolding.
5. → Track B forward-port setup (BN254 patches → wasmvm v3.0.x; just the rebase initialisation, full work spans 3-5 days).
6. → Update `LESSONS_2026_05_13_MORNING.md` with the new findings, commit everything.

PR #929 substantive review is **next session** (not in scope for today — read the diff carefully, write an audit-style review, optionally publish as a Moultbook entry).

---

*Apache-2.0. Captured 2026-05-13 morning, after the JunoCommsDept tweet that confirmed Juno's official narrative is amplifying our work and that Jake is independently using the same structured-memory pattern we are.*
