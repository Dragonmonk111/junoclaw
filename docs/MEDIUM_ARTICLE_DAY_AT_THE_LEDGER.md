# A Day at the Task Ledger

## Tier 1-slim, Tier 1.5, and the quiet bit of tooling in between — everything JunoClaw's task ledger gained in one afternoon, and how we intend to find out whether any of it was worth it.

[IMAGE 1 — *Hand-drawn pencil, vintage.* A low-ceilinged scribe's chamber seen at three-quarter view: a long oak desk, a single beeswax candle in a pewter holder, a half-finished ledger page, a stone floor worn smooth by generations of boots. The pencil work is all cross-hatching and soft graphite, edges of the page foxed with age, the single flame picked out with a sharper point. No color implied; a naturalist's sketch from the long afternoon before lamplight.]

---

> *This is an engineering diary entry. Everything described here lives in the
> JunoClaw repository; the contracts compile, the workspace test suite passes
> 149 / 149, and the Tier 1.5 code is now **live on `uni-7` at code_id 75** at
> the new task-ledger `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm`.
> All three on-chain smoke tests — `TimeAfter`, `BlockHeightAtLeast`, and
> `EscrowObligationConfirmed` — have passed against the live contract. The
> piece is meant to be read alongside the audit reference in
> `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md`, the testnet run ledger in
> `@junoclaw/docs/TIER15_TESTNET_RUN.md`, and the field observation tracker in
> `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`.*

---

## Where the day began

Before this morning, the JunoClaw `task-ledger` on `uni-7` stood at code_id 70 — a v6.1 build carrying the scars of the Rattadan hardening pass. Four testnet regressions (F1 through F4) had been closed against live chain state. The submitter could no longer self-complete their own task; `DistributePayment` had been gated to admin and DAO members; `builder-grant` would reject a duplicate `work_hash` before writing it; `junoswap-pair` would reject a rogue native denom in `info.funds`. All of that is in `@junoclaw/docs/V6_TESTNET_RUN.md`, and none of it changed today.

What did change was a slightly quieter gap. Even with the v6.1 fixes in place, `CompleteTask` still worked by admission. An authorised caller said "completed", the escrow callback fired, the registry trust score moved, and the chain wrote. What the chain did not do — what the chain had never done — was *check* anything before writing. The tightening we got in v6 was about who was allowed to swear. What stayed open was what they were allowed to swear *about*.

[IMAGE 2 — *Hand-drawn pencil, vintage.* A single page from a medieval cartulary, open at a charter of manumission. The wax seal is drawn with a darker pencil, intact but cracked across its face; the chancery hand above it is rendered as faint cross-hatching, barely legible, the way very old script looks to modern eyes. The seal is the point — it is what makes the page weigh something.]

---

## Tier 1-slim: the seven words

The piece that landed first was a small, declarative trust primitive. Submitters can attach a `Vec<Constraint>` to a task at submission time, one for the `pre_hooks` list and another for the `post_hooks` list. The `task-ledger` evaluates the pre-hooks before the status transitions from `Running` to `Completed`, and the post-hooks after the transition but before the escrow and registry callbacks fire. Any failure returns a `ConstraintViolated` error, which — because CosmWasm discards every storage write and every sub-message when an entry-point returns `Err` — reverts the whole completion atomically. Nothing partially settles. The task remains `Running`, the escrow remains `Pending`, the registry trust score remains at its pre-call value.

The vocabulary itself is seven variants. Four shipped in the Tier 1-slim pass:

- **`AgentTrustAtLeast { agent_id, min_score }`** — refuses completion if the agent's `trust_score` in `agent-registry` has fallen below `min_score`. Useful for proposals that route work to a named agent and want to auto-revoke if that agent is slashed between proposal-create and proposal-execute.

- **`BalanceAtLeast { who, denom, amount }`** — a bank-module query. A task attached to a treasury cannot be completed in a block where the treasury has been drained below some floor.

- **`PairReservesPositive { pair }`** — "this task must not complete in a block where the liquidity pool has been emptied." Narrow but pointed; the kind of check that matters the afternoon someone else's arbitrage loop drains your quote token.

- **`TaskStatusIs { task_ledger, task_id, status }`** — a cross-task precondition. Task B with `TaskStatusIs { task_id: A, status: Completed }` cannot settle until Task A has. This is how a DAG of tasks gets expressed without a separate orchestration contract.

The deep narrative on why a declarative enum rather than a user-supplied callback — and why a bounded vocabulary rather than unlimited expressiveness — is in `@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md`. The short version is that every word in the enum is spelled out in the audited source tree, and the audit delta is bounded by the enum. A callback-address pattern turns every future user into an audit dependency; an enum does not.

Tier 1-slim was pre-existing scope when the day started. What was new today was everything downstream of it.

---

## Tier 1.5: three more words

After the Tier 1-slim four were sitting quietly in the repository, the gaps they did *not* close became more legible. Three specific invariants came up in design conversation often enough to deserve their own variants:

- *"This task cannot complete before a wall-clock timestamp."* Timelocks. Vesting windows. Grace periods.
- *"This task cannot complete before a block height."* IBC acknowledgement windows. Finality invariants. Anything denominated in block counts rather than seconds, because block intervals move.
- *"This task cannot complete until its obligation in `escrow` has moved to `Confirmed`."* The payment invariant. The single gap between v6 coherence and full payment-completion coherence.

These three became `TimeAfter`, `BlockHeightAtLeast`, and `EscrowObligationConfirmed`. Each is a single `Constraint` variant. Each adds exactly one query path — `env.block.time` for the first, `env.block.height` for the second, and a cross-contract `GetObligationByTask` to the existing `escrow` contract for the third. No new cross-contract dependencies; `escrow` already exposed that query in v6. No new state; the three variants are read-only, like the four before them.

The code change required one small disciplining: the evaluator's signature had to grow from `evaluate(deps, agent_registry)` to `evaluate(deps, env, agent_registry)`, because two of the three new variants needed `env.block`. Threading `&Env` through `evaluate` and `evaluate_all` was the only breaking API change in the commit — and because the helper functions were only ever called from `task-ledger::execute_complete`, there were exactly two call-sites to update.

[IMAGE 3 — *Hand-drawn pencil, vintage.* Three small medallions in the margin of a folio, each labelled in a tight copperplate: a pocket-watch for *TimeAfter*, a block of standing stones for *BlockHeightAtLeast*, and a small strongbox with its lid ajar for *EscrowObligationConfirmed*. The medallions are shaded in fine pencil; the surrounding folio is plain, unlined. An ex libris stamp in the upper corner, partly obscured.]

Three new regression tests joined the suite, one per variant. Each test uses the same pattern: submit a task with the hook, try to complete it, observe that the chain rejects with a specific error string, advance the state (clock, height, or escrow obligation status) past the threshold, retry the completion, observe success. The same task is used in both halves of each test — which is only possible because atomic revert guarantees the task is still `Running` after the failed attempt. Twelve hook-bearing regression tests across the four variants, now seven. The workspace total climbed from 145 to 148, every one of them green.

---

## A quiet bit of tooling

The three new variants would have been worth nothing if the path to `uni-7` was unclear. An audit-clear design with an unclear deploy is a design waiting to be shipped by someone who has to guess. So the afternoon was spent writing the ramp.

Two new scripts landed in `@junoclaw/deploy/`:

**`migrate-tier15.mjs`** is the migration executor. It loads the Parliament wallet named in `$env:PARLIAMENT_ROLE`, connects to `uni-7`, reads the live `task-ledger` address from `deployed.json`, and — before broadcasting anything — calls `client.getContract(addr)` to assert the sender is the contract's wasmd-level admin. A mismatch produces a console diagnostic and a clean exit; no tx fee is burned on a misconfigured env. If the admin check passes, the script uploads `task_ledger_opt.wasm` (386.8 KB, binaryen-optimised from the 457.5 KB raw release build), captures the new `code_id`, sends an empty `MigrateMsg`, and queries `GetConfig` to sanity-check the contract still answers. Both `code_id`s — the new one and the prior one — are written to `deployed.json` so rollback is a one-command operation. `DRY_RUN=true` and `SKIP_UPLOAD=true` env flags cover the two most common re-entrant cases.

**`smoke-tier15.mjs`** is the on-chain validator. Three tests: T1 exercises `TimeAfter` with a 60-second threshold, T2 exercises `BlockHeightAtLeast` with a six-block threshold, and T3 exercises `EscrowObligationConfirmed` by authorising a Pending obligation on the live escrow, submitting a ledger task that references it, observing the `ConstraintViolated` revert, flipping the obligation to `Confirmed`, and re-attempting the completion. The same atomic-revert property that makes the regression tests correct in `cw-multi-test` makes them correct on a real chain — the task is still `Running` after each failed attempt. Results land in `smoke-tier15-results.json` with tx hashes and error strings for every step.

Neither script has been run against the live chain yet. That is a single human decision — the wasmd-level admin's signature — not a code change. The reason to hold is not uncertainty in the code; the reason to hold is that the step *after* "we migrated" is "we decided it was observable enough to be worth migrating", and we owed that decision a proper thinking-through.

---

## How we'll know if it worked

The temptation in a piece of instrumentation work is to ship the most elaborate telemetry you can stand to maintain. That is usually wrong. It is wrong because the metrics you gather before you know what question you are answering are the metrics you end up apologising for later, and because every metric is a piece of state someone has to look at and ignore.

What we actually want to know, in the first eight weeks after migration, is three things.

1. Which of the seven variants, in practice, get used.
2. Of the ones that get used, which ever fire their violation path.
3. Which invariants users ask for that none of the seven express.

All three are answerable from tx logs with a grep pattern. A reverted `CompleteTask` produces a cosmjs error whose message is literally `ConstraintViolated: pre_hook: hook[0]: TimeAfter: block time 1745190000 < required 1745200000`. Count occurrences of that string over a window, parse out the variant name, tally by day. Zero contract change. Zero event-watcher code. Every block explorer on Juno already surfaces the string on the reverted-tx page. The integration cost is almost literally nothing.

[IMAGE 4 — *Hand-drawn pencil, vintage.* A Victorian telegraph operator's notebook, open flat. On the left page, a column of neat short-hand entries — each one a few words, each one timestamped to the minute. On the right page, a magnifying lens rests across a folded telegram. Graphite shading deep along the gutter; a pale wash of sepia implied on the paper. The drawing has the calm of someone who trusts their own handwriting.]

That is Phase 1 — and it is where we will begin. But it is worth being clear-eyed about what Phase 1 can and cannot carry, because the shape of the failure modes tells us exactly what Phase 2 needs to look like.

**The grep approach is blind on the success side.** CosmWasm discards the entire `Response` — every `add_attribute`, every event — when an entry-point returns `Err`. That is the invariant that makes atomic revert safe, and we are not going to wish it away. But it means violations are observable in failure logs and successes are not observable at all without a separate signal. A tally of violations is a useful number. A *ratio* — violations over hook-bearing completions — is a much more useful number. The ratio requires a denominator. The denominator is the thing grep cannot give us.

**The grep approach is fragile to format drift.** The error string is produced by three Rust layers: the per-variant `format!` inside each `Constraint::evaluate` arm, the `hook[i]:` wrapper in `evaluate_all`, and the `pre_hook:` / `post_hook:` wrapper at the call-site. A refactor to any of them could silently change the output. The fix for this is trivial — an explicit regression test that asserts the full string shape for each variant — and should be made mandatory before any dashboard is built on it. A test that fails on format drift is a test that lets a dashboard sleep.

**The grep approach is coupled to wasmd's log format.** Juno has upgraded wasmd several times across its lifecycle. Each upgrade has, at some point, touched `tx_result.log` shape, event-attribute prefixing, or the way cosmjs surfaces revert reasons. A grep-based dashboard pinned to today's format will silently break on the next wasmd minor. Contingency: pin the RPC to a known wasmd version during the observation window, or run a monthly integration check that exercises grep against a real `uni-7` revert.

**The grep approach is coupled to archive retention.** Public RPCs retain transaction data for a limited window. Four weeks with Polkachu is fine; eight weeks starts pushing it; anything longer will need either our own archive node or periodic snapshots of the failure-log grep output.

**The grep approach does not scale indefinitely.** A few hundred hook-bearing transactions per day is a grep problem. A few thousand is a Tendermint-log-indexer problem. A few hundred thousand is a full tx-indexing stack. The breakpoint is somewhere around 10,000 txs/day before grep is materially in the way; JunoClaw is several orders of magnitude under that and will be for a while. If we ever climb past it, the decision to build an indexer will have been made months earlier for reasons unrelated to hooks.

**Phase 2, deferred.** The cheap fix for the success-side denominator is two `.add_attribute` calls on the success-path `Response` in `execute_complete` — `pre_hooks_count` and `post_hooks_count`. Zero state change, zero authorisation change, zero revert-behaviour change. Every CosmWasm indexer in the ecosystem already knows how to aggregate event attributes. The cost is that adding them means a re-migrate, which enlarges the Tier 1.5 migration delta by one more readable line. Our stance is to defer Phase 2 and re-evaluate at the two-week check-point. If the grep data is producing a tally where we wanted a ratio, we ship the attributes. If the tally is clearly enough, we save the re-migrate.

**Phase 3, parked.** The real future-proof answer is a storage counter inside `task-ledger` — a `HOOK_USAGE: Map<String, u64>` incremented per variant on every successful `evaluate_all`. That makes both successes and violations observable from contract storage via a dedicated query, entirely independent of tx logs, event formats, RPC uptime, archive retention, and indexer availability. It costs a small amount of gas per complete and a small amount of storage per variant. It is also a real state-layer change, which makes it a real audit delta. The correct moment to ship it is when Phase 1+2 are no longer enough for the question we are asking — probably not before traffic passes 10,000 hook-bearing txs per day, probably not before Q3 2026 on current trajectory.

The discipline here is: *ship the cheapest signal that answers the current question, and carry a written record of the step we will take when the current signal fails.* That record lives in `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`, which is what lets us know when grep has stopped being enough.

---

## What this is not, again

It is worth saying, because the scope of the day was deliberately narrow:

- **Not an intent-solver.** An intent says *what* the user wants; a constraint says *what must hold at the moment of completion*. Adjacent ideas, not the same idea. Epoch Protocol and the EVM intent-ledger designs are much larger commitments; we have not made them.
- **Not an observer registry.** An observer pattern would let user contracts register as callbacks on task lifecycle; the hook vocabulary is the opposite, evaluated internally against an audited `enum`, with no external code in the trust boundary.
- **Not a timelock-custody contract.** `TimeAfter` gates completion; it does not hold funds. A timelocked task with an `EscrowObligationConfirmed` hook composes to a timelocked escrow settlement, but the pieces are orthogonal.
- **Not a WAVS replacement.** WAVS proves *how* work was done — TEE-bound attestation of provenance. Constraints prove *under what conditions* the work is accepted as done. Provenance and admissibility are different claims.

The boundary matters because what makes the primitive small is what makes it audit-stable. Every shape we decided not to ship today is a shape the audit delta did not have to carry.

---

## The day, in file-level summary

For the reader who wants the raw inventory:

**Contract code (modified)**
`@junoclaw/contracts/junoclaw-common/src/lib.rs` — three new `Constraint` variants, `&Env` threaded through `evaluate` / `evaluate_all`, typed-query shims added for `EscrowQuery` / `PoolReservesView`.
`@junoclaw/contracts/task-ledger/src/{msg,contract,error}.rs` — pre/post hook plumbing on `ExecuteMsg::SubmitTask`, pre-then-post evaluation inside `execute_complete`, new `ConstraintViolated` error variant.
`@junoclaw/contracts/task-ledger/src/tests.rs` — three Tier 1.5 integration tests plus a `StubEscrow` contract helper.

**Tooling (new)**
`@junoclaw/deploy/migrate-tier15.mjs` — uploads new wasm → migrates live contract → sanity-checks → records to `deployed.json`; admin pre-flight rejects misconfigured signers before broadcasting.
`@junoclaw/deploy/smoke-tier15.mjs` — three on-chain integration tests for `TimeAfter`, `BlockHeightAtLeast`, and `EscrowObligationConfirmed`, each exercising both violation and success paths on a single task.

**Docs (new or modified today)**
`@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md` — the long-form narrative piece on the primitive itself; deep technical detail, risk matrix, observability contingencies.
`@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md` — canonical engineering reference with the audit loop checklist, security-measures inventory, migration + rollback procedures, testing matrix, and deferred / parked items.
`@junoclaw/docs/TIER1_SLIM_OBSERVATION.md` — field-ops tracker for the observation window; copy-pasteable build / migrate / smoke commands, escalation criteria, log section.
`@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md` — this piece.

**Build artefact**
`C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger_opt.wasm` — 386.8 KB, binaryen-optimised, within wasmd's 800 KB upload limit.

**Test posture**
Workspace: 149 / 149 green (up from 148 after the same-day error-string shape regression test). `task-ledger` alone: 28 / 28 green, up from 24. Zero tests were weakened; every v6 invariant from the Rattadan hardening pass remains in force. `cargo clippy --workspace --all-targets` returns zero errors on the new code; the only warning is the pre-existing MSRV-guarded `is_multiple_of` note in `zk-verifier`.

**State on `uni-7`**
New Tier 1.5 `task-ledger` live at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` under code_id 75, with the wasmd-level migrate admin properly set on the deployer wallet. The `agent-registry` was rewired to point at the new address; the `escrow` and `agent-company` contracts are reused unchanged. The v6 `task-ledger` at `juno17aq…` is preserved in `deployed.json` as a frozen audit record (it has no migrate admin and was retired in place). All three Tier 1.5 variants pass on chain, after one round of iteration on the smoke harness itself — see below.

[IMAGE 5 — *Hand-drawn pencil, vintage.* A wooden printer's type case, half-drawn out of a cabinet. The compartments hold individual letters in pewter, each one shaded to suggest weight. A single letter — a lowercase 'n' — is set out on the page beside the case, as if the artist paused mid-compose. A draughtsman's rule at the edge of the frame. Cross-hatching dense in the cabinet's interior, soft pencil on the page edges. The scene is still; the type is ready.]

---

## The deployment, and what the chain actually said

The signature happened. The chain answered. What we found was not what we expected, which is usually the most useful finding.

### The lock that had no key

The migrate dry-run refused to proceed. `migrate-tier15.mjs` read the live contract's metadata via `client.getContract(juno17aq…)` and reported back:

```
Contract admin on chain: (none)
```

The v6 `task-ledger` instantiated on 2026-04-18 had been created **without a wasmd-level migrate admin**. The line that sets the admin is the sixth argument to cosmjs's `client.instantiate`, and yesterday's `deploy.mjs` at line 240 omitted it. The contract was, in wasmd's terms, **immutable** — not by design, by oversight. Nobody could migrate it. Not us, not anybody.

This is the kind of finding that is more educational than painful. Admin-at-instantiation is a three-character addition to a line of code; the cost of having it is nothing; the cost of not having it is a permanently frozen contract. It is also the kind of finding that only surfaces when you try to do the thing you had not yet had reason to do. We had never tried to migrate before. The first attempt to migrate was the first check of whether we could.

The escrow and agent-company contracts, we later discovered, were instantiated with the same omission. They are all frozen at their v6 addresses. None of this matters for correctness — the contracts work, the tests pass, the agents can use them — but it means any future improvement to those contracts requires a fresh instantiation, not a migrate.

[IMAGE 7 — *Hand-drawn pencil, vintage.* A locksmith's bench at the end of a long day: a wooden plane, a hand-wound drill, a dozen iron keys on a leather thong, a brass padlock lying open with a single broken bolt next to it. The shaving-pile from a draw-knife is rendered in pale graphite scatter; the padlock's interior ward is drawn with the exaggerated clarity of a trade-manual illustration. "Locksmith's Manual, Plate 14." The drawing has the calm fatalism of a craftsperson acknowledging a mistake.]

### What yesterday's playbook said

We looked at how the v6 deploy was actually done — `@junoclaw/docs/MEDIUM_ARTICLE_V6_HARDENING.md`, "Beating the Bounds", published the afternoon of 2026-04-18. It is a record of a **fresh deploy**, not a migrate. Every v6 contract was uploaded and instantiated from scratch; Step 6 of `deploy.mjs` then wired `agent-registry.registry.task_ledger` to point at the freshly-instantiated task-ledger so its `IncrementTasks` callback would be accepted.

That pattern applied almost exactly to our situation. We needed a fresh `task-ledger` at the Tier 1.5 code_id, with the wasmd admin set this time, and we needed to re-point the existing `agent-registry` at it. The existing `escrow` could be reused unchanged — the T3 hook only queries it, doesn't need to update it.

A small focused script, `deploy-tier15-fresh.mjs`, came out of that reading. Four steps: upload the new wasm, instantiate with `{ admin: sender }` as the sixth arg (the line yesterday forgot), `UpdateRegistry` on the agent-registry to re-point its `task_ledger` field, and update `deployed.json` — moving the old entry to `task-ledger-v6-frozen` so the audit history was preserved but the primary pointer advanced to the new contract.

The dry-run of that script confirmed all the preconditions: balance, admin match, registry internal-admin, wasm present. The real run, a few minutes later, produced:

| Step | Tx | Artefact |
|---|---|---|
| Upload task-ledger Tier 1.5 wasm | `B7E0D1750FA6CCE0A7D1D6038B382F39F8A8DE7DDFEBC583A1FC6C4CB83290C5` | **code_id 75** |
| Instantiate new task-ledger **with wasmd admin** | `9F247E8BB0D41F2F6F245F2D362FB5E932A9077502D1D1ADF1FC3D2F85E29DE7` | `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` |
| `UpdateRegistry` on `agent-registry` → new task-ledger | `243B0BD1CFCB3053865ECCFFB376D04A13C89A524D55ADC23462EA019C4A6E78` | agent-registry now knows the new task-ledger |

The new contract's wasmd admin was read-back-verified on chain as `juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`, the deployer. Future migrations of the Tier 1.5 task-ledger can be signed by that wallet. The pattern that failed us this morning has been broken, at least for the one contract that matters most.

[IMAGE 8 — *Hand-drawn pencil, vintage.* A wax seal being pressed into hot wax with a signet ring; the motif on the ring is a small stylised claw, the wax dark and glossy, the hand holding the ring rendered only in partial — a wrist, a cuff, the suggestion of sleeve. A drop of wax has fallen onto the parchment beside the seal; a quill and a half-finished line of chancery script above. The drawing has the quiet weight of an act that can be looked up later.]

### The smoke tests, and the clock that wasn't

With the new task-ledger live, `smoke-tier15.mjs` ran the three Tier 1.5 integration tests: `TimeAfter`, `BlockHeightAtLeast`, and `EscrowObligationConfirmed`. Each test submits a task with a hook, attempts completion before the condition is met (expecting a revert), advances the chain state to cross the threshold, and retries (expecting success). The atomic-revert guarantee means the same task can be reused for both attempts — a failed completion leaves the task `Running`.

**T2 passed cleanly.** The block-height-at-least hook refused completion at height 12929504 (hook required ≥ 12929508), the script polled until the chain reached the threshold, and the retry went through. `Constraint violated: pre_hook: hook[0]: BlockHeightAtLeast: block height 12929504 < required 12929508` on the first attempt; `AE47ECB125…` for the successful second attempt.

**T3 passed cleanly.** The script created a `Pending` obligation on the existing v6 escrow at `juno17vrh77…`, attached an `EscrowObligationConfirmed` hook to a task referencing that obligation, tried to complete (rejected with `task 716000782 obligation is Pending, expected Confirmed`), then `Confirm`-ed the obligation and retried. Successful completion at `AE47ECB125C9B0561A937DCC33DAEA57E557C9F4DDBAC2B20A3EB0CFF2DE0DD2`. This also demonstrated something worth noting: a v6 escrow (non-migratable, unchanged) and a v7 task-ledger (fresh, Tier 1.5) compose correctly. The escrow has no idea what a hook is. It doesn't need to.

**T1 failed. And then passed.** This is the part worth writing down.

The first run of T1 rejected the early attempt with the expected `TimeAfter: block time X < required Y`. The script then slept for 70 seconds (hook threshold was `Date.now()/1000 + 60`). The retry was still rejected — with the same error, citing a chain block time 881 seconds earlier than we thought it should be.

That ratio was the finding. `uni-7`'s reported block time at the test moment was **14 minutes and 41 seconds behind our local wall-clock**. Our test's threshold was set against *local* time; the chain's `env.block.time` is set by the Tendermint block header, which lives by its own schedule; there is no reason the two should agree, and on public testnets they very often don't.

The evaluator had done its job perfectly. `env.block.time.seconds() >= *unix_seconds` is a correct comparison against a correctly-reported chain time. The error message was precise, timestamped, and printed the exact values it was comparing. The test was wrong. Not by design — by unexamined assumption.

[IMAGE 9 — *Hand-drawn pencil, vintage.* A marine chronometer in its gimballed brass case, open on a navigator's desk next to a church-bell pocket-watch. The two are plainly running at different times; the gap between their hour hands is rendered in sharp pencil. A sextant at the edge of the frame, a log-book open to a half-filled page. "Comparison with the town clock, 4th Bell." The scene is calm; the navigator is writing down the disagreement, not troubled by it.]

The fix was a four-line patch to `smoke-tier15.mjs`. A new helper, `getChainTimeSec(client)`, reads the latest block header's time and returns seconds since epoch. A new poller, `waitUntilChainTime(client, target, maxWait)`, polls it until the chain has crossed the threshold. T1 now computes its threshold against *chain* time rather than local time, and waits for chain time to reach it. The retry, on a re-run of T1 only, crossed the threshold in the expected ~60 seconds of real chain time and the completion went through: `C85744602D8CABC4D2D0E4D15FC92237C2C8AA8AA92DD7CDA90A4CAE98C89777`.

There are three things this small bug taught us, in ascending order of importance:

First, that the contract is correct. The evaluator read the right value, compared it correctly, and produced a non-lossy diagnostic. If our assumption had been hidden inside the contract, we would have shipped a bug. Because the contract's side of the pipeline is simple and correct, the bug surfaced entirely in the test harness, where it belongs.

Second, that the observation window we have planned is going to see similar finger-on-the-scale issues. Any dashboard that counts `TimeAfter` violations on `uni-7` will show inflated numbers unless it normalises against chain-reported block time. This is exactly the sort of contingency that the tx-log-grep discussion in the Constraints article was pointing at — a format or timing convention that drifts, silently, between the chain and the things that watch it.

Third, and most importantly, that the *test infrastructure itself* needs the same audit discipline as the contract. Our smoke tests are how we know the chain is behaving. A smoke test that depends on an unexamined clock assumption is a smoke test that will eventually lie to us. The patch that landed today belongs not only in the test file but in the discipline: *the thing that tells you the system is working must itself be subject to the same rigour as the system.*

### What now lives on uni-7

```
agent-registry   juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep   code_id 69  (reused, now points at new task-ledger)
task-ledger      juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm   code_id 75  (Tier 1.5, wasmd-admin-enabled)
task-ledger-v6   juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn   code_id 70  (frozen, no admin, retired)
escrow           juno17vrh77vjrpvu6v53q94x4vgcrmyw57pajq2vvstn608qvs5hw8kqeew3g9   code_id 71  (reused, unchanged)
agent-company    juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw   code_id 72  (reused, unchanged)
```

The old v6 `task-ledger` at `juno17aq…` still exists on chain and still serves historical queries, but `agent-registry.registry.task_ledger` no longer points at it; any `CompleteTask` sub-message it tries to send is rejected. It is a frozen archive, not a live contract. The full audit trail is in `@junoclaw/docs/TIER15_TESTNET_RUN.md`.

---

## Shipping posture

We did not ship, today, the intent-solver architecture. We did not ship an observer registry. We did not ship the storage-counter observability. We did not even ship the two success-path event attributes that would make the success/violation ratio cheap. Some of those we may ship in a month or a quarter; some of them we may never ship. The question of which is decided by evidence from the observation window, not by speculation today.

What we did ship is seven words the ledger can say no to, the three-variant extension that closes the timelock and payment-coherence holes, the fresh-deploy ramp that put them onto `uni-7` with a wasmd admin properly set, the three-test on-chain validation (one of which required fixing an unexamined clock assumption in the test harness), a frozen audit record of the retired v6 task-ledger, and four layered documents — the primitive, the architecture, the testnet run, and the observation plan.

A thin ring around an already-hardened core is worth more than an ambitious ring that redraws the perimeter. That has been the Rattadan posture since v5. Nothing in today's work asks it to change, and the one thing we found that we did not expect — the frozen-admin oversight on the v6 contracts — is now an explicit, written-down reason to treat admin-at-instantiation as a non-negotiable part of the deploy-script discipline going forward.

The next concrete event is the first real hook-bearing `CompleteTask` from one of the live agents — not from our smoke harness, but from the agent-daemon pipeline that the WAVS operators already run. After that, we watch.

---

## After the chain, before the commit

The chain was done by early evening. What remained was the quiet work of closing the loops that the chain had opened.

Two small changes landed before the day's commit.

The first was a one-line fix per instantiate-site in `@junoclaw/deploy/deploy.mjs`. The v6 contracts were frozen because every `client.instantiate` call had omitted its 6th argument — the wasmd-level admin. That was a silent mistake the chain had been patiently waiting to surface. It surfaced today, at the wrong moment, in the form of a migration script that could not migrate. The fix is the obvious one: `{ admin: address }` passed as the 6th argument to every instantiate call, now with a comment above the first one that points a future reader at `@junoclaw/docs/TIER15_TESTNET_RUN.md` so the history of *why this comment is here* does not have to be re-derived from scratch. The frozen v6 `escrow` and `agent-company` remain frozen on chain — we cannot un-freeze what we cannot migrate — but the next fresh deploy will not repeat the oversight.

The second was a test. `test_v7_constraint_violated_error_string_shape_is_stable`, in `@junoclaw/contracts/task-ledger/src/tests.rs`. It locks down all four layers of the `ConstraintViolated` error-string prefix that observability downstream will eventually depend on: the outer `Constraint violated: ` from the `thiserror` Display impl, the `pre_hook:` or `post_hook:` wrapper from `execute_complete`, the `hook[i]:` index from `evaluate_all`, and the `VariantName:` prefix from each per-arm `format!`. It uses `TimeAfter` (pre_hook path) and `BlockHeightAtLeast` (post_hook path) as the two exemplars because the wrapper layering is variant-agnostic — if it is correct for those two it is correct for all seven. A silent refactor that changes the prefix shape will now fail this test loudly instead of silently breaking a dashboard.

Neither change is interesting on its own. What is interesting is that both belong to the same discipline: *the thing that surprises us once should, by the end of the same day, have a written-down reason not to surprise us again*. The frozen-admin oversight is now caught by a comment in `deploy.mjs` and a wasmd-admin readback at the end of `deploy-tier15-fresh.mjs`. The error-string shape is now caught by a test that runs on every `cargo test`. The clock-drift assumption is now caught by chain-time helpers in `smoke-tier15.mjs` and a `§10.5` paragraph in the architecture document that says, plainly, do not trust the wall. None of these are heroic engineering. All of them are the cost of treating a surprise as a permanent lesson, and none of them would have landed in a week that closed at `all three smoke tests passing`.

The workspace test count went from 148 to 149. The `task-ledger` test count went from 27 to 28. And the backlog from the post-mortem section earlier in this piece shrank by two items, marked `[x]` rather than `[ ]`, in the observation tracker. That is what a working day finished properly looks like.

---

*Today's code lives in the JunoClaw repository at `@junoclaw/contracts/junoclaw-common/src/lib.rs` and `@junoclaw/contracts/task-ledger/src/`. The deployment path is `@junoclaw/deploy/deploy-tier15-fresh.mjs` (and the v6-compatible migrate path in `@junoclaw/deploy/migrate-tier15.mjs`). The on-chain smoke harness is `@junoclaw/deploy/smoke-tier15.mjs`. The canonical engineering reference is `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md`; the testnet run ledger is `@junoclaw/docs/TIER15_TESTNET_RUN.md`; the field-operations tracker is `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`. The full regression suite stands at 149 passing tests across the workspace (up from 148 after the same-day error-string shape regression test). The v6 invariants from `RATTADAN_HARDENING.md` remain in force. The new Tier 1.5 `task-ledger` at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` (code_id 75) is migratable; the frozen v6 copy at `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` (code_id 70) is not, and never will be.*

[IMAGE 10 — *Hand-drawn pencil, vintage.* The same scribe's chamber from the opening and mid-day scenes, now at nightfall: the candle has been snuffed, a thin thread of smoke still rising; the ledger is closed with its brass clasp fastened; a clean sheet of parchment lies beside it for tomorrow, weighted by a small brass ink-stand. Through the window, the first three stars. The pencil work is at its deepest at the base of the walls, softest around the thread of smoke. A working day, properly finished.]
