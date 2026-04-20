# The Ledger That Says No

## How JunoClaw learned to refuse work it could not yet verify — a small, declarative trust primitive added to the task ledger.

[IMAGE 1 — *Hand-drawn pencil, vintage.* A low stone gate with an iron latch, mist drifting across heather; cross-hatched shading on the masonry, soft graphite texture in the sky. The kind of sketch you'd find in a Victorian field naturalist's notebook, edges foxed, corners slightly curled.]

---

> *A short engineering note, not an audit. This piece describes a Tier 1-slim change to the `task-ledger` contract that landed after the v6 pre-audit hardening pass. The code compiles, tests pass, and the change is deliberately additive — but it has not yet been exercised on mainnet, and it does not replace independent review.*

---

There is a moment in the old Welsh law-books where a man brings a horse he has sold to the border of a cantref, and before he can hand it over, the *cynghellor* — the court officer — asks him to swear that the horse is sound. If the horse has a foot-rot the man did not disclose, the oath writes his liability into the roll. The swearing does not *cause* the horse to be sound. It only names the condition the sale was understood to meet.

A lot of contract code is exactly this. A task is marked Completed, an escrow settles, a registry increments a trust score. In each of those micro-transactions, the chain is, in effect, recording an oath: *this work was done to this standard*. And for most of them, nothing on-chain actually *checks* the standard. The submitter swears. The chain writes.

JunoClaw has worked this way since v4. The submitter says "Completed", the callback fabric fires, the trust score moves. Version 6 closed the biggest identity-forgery hole in that pattern — you can no longer submit a task you do not own, and you can no longer self-attest a completion. But the tightening was *about who swears*, not *about what gets sworn*.

Tier 1-slim is about what gets sworn. It adds a small vocabulary the submitter can use to attach an invariant to the completion — a condition the chain will *check*, on-chain, at the moment of finalisation. If the condition does not hold, the completion reverts atomically. Nothing partially settles.

It is a ledger that has learned to say no.

[IMAGE 2 — *Hand-drawn pencil, vintage.* A latched wooden door with carved runes cut into the oaken lintel; the grain of the planks in careful graphite cross-hatching, the pitted iron of the latch shown with burin-dark accents. Detail of hinge and nail-head picked out in sharper pencil, as though drawn from life by an Edwardian archaeologist on tour in Ynys Môn.]

---

## What changed

One file in `junoclaw-common` got a new `enum Constraint` with seven variants. One field on `TaskRecord` became two — `pre_hooks` and `post_hooks`, each a `Vec<Constraint>`. One function in `task-ledger::execute_complete` evaluates the pre-hooks before the state transition, and the post-hooks after. A new `ContractError::ConstraintViolated { reason }` variant carries the diagnostic back to the caller, which — because CosmWasm treats `Err` as "discard all storage writes in this tx" — causes the escrow callback, the registry callback, and the status flip to unwind together.

That is almost the entire change. It is smaller than the v6 hardening pass. It is substantially smaller than the v5 coherence pass. It is deliberately small.

Here is the full vocabulary, in `junoclaw-common/src/lib.rs`:

```rust
pub enum Constraint {
    AgentTrustAtLeast { agent_id: u64, min_score: u64 },
    BalanceAtLeast    { who: Addr, denom: String, amount: Uint128 },
    PairReservesPositive { pair: Addr },
    TaskStatusIs      { task_ledger: Addr, task_id: u64, status: TaskStatus },
    TimeAfter         { unix_seconds: u64 },
    BlockHeightAtLeast { height: u64 },
    EscrowObligationConfirmed { escrow: Addr, task_id: u64 },
}
```

Seven sentences a task can refuse to be completed without.

Four of them (the first four) shipped in the Tier 1-slim pass. Three of them (the last three) were added in a follow-on Tier 1.5 pass once the first four proved themselves in regression tests. The seven together cover the invariants we have wanted at `CompleteTask` time since the callback fabric first existed in v4.

---

## Why it is a *declarative* primitive, not a hook

There is a natural-looking alternative to this design: expose a callback address on each task, call that address at completion time, let it return `true`/`false`. That is what most smart-contract platforms end up building first. It is also what you end up regretting, for two reasons.

The first is the **audit surface**. A callback target is arbitrary code. The moment `task-ledger` is willing to call a foreign wasm module and let its return value gate settlement, every audit of `task-ledger` becomes an audit of every possible callback target, because the callback target is now in the trust boundary. The correct answer *for an audit-minded product* is to not give out that power. Give out the vocabulary of questions the core contract knows how to answer, and let the submitter pick a subset.

The second is **determinism of review**. A constraint like `AgentTrustAtLeast { agent_id: 42, min_score: 75 }` is a sentence. It has a truth value. A reviewer reading a governance proposal sees the sentence, understands exactly what will be checked, and knows the check is implemented once in one file that has its own regression tests. A `callback_addr: juno1xxxxx` in the same proposal is a black box. It requires the reviewer to *also* audit `juno1xxxxx` — which, the moment that address is user-settable, becomes an infinite regress.

So the vocabulary is bounded, and every word in it is spelled out in `junoclaw-common`. The cost of expressiveness is paid, once, in the `enum`.

---

## Atomicity is what makes it a trust primitive

The usual failure mode of "check at settlement time" designs is partial settlement. The check happens, the check passes, and then a downstream sub-message fails on a different invariant — and now you have a Completed task but no escrow release, or an incremented trust score against a task the escrow never confirmed. The v5 coherence pass fought this battle at length; the scars are still in `@c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\RATTADAN_HARDENING.md`.

The Constraint evaluator inherits the v5 resolution. CosmWasm's execution model is "all storage writes in this entry-point execute together, or not at all". A constraint that returns `Err` — anywhere between the start of `execute_complete` and the return of the `Response` — causes every storage write performed inside `execute_complete` to be discarded, and every sub-message to be not-fired. The task is still Running. The escrow is still Pending. The trust score still reflects the pre-call state. The operator sees a `ConstraintViolated { reason }` in the tx log and knows exactly which hook tripped and why.

In other words: the Constraint enum turns a bag of independent invariants into a *single commitment*. Either all of them hold and the task settles coherently, or one of them does not, and nothing about the task moves. There is no fractional state that a future observer could misread.

---

## The seven words

A brief tour through the vocabulary, with the use-case each one was designed for.

### `AgentTrustAtLeast { agent_id, min_score }`

Cross-contract query to `agent-registry`. Refuses completion if the agent's `trust_score` has fallen below `min_score`. The practical use is governance proposals that dispatch work to a named agent: attach `AgentTrustAtLeast { agent_id: me, min_score: 50 }` and the proposal's own task becomes self-revoking if the agent is slashed between proposal-create and proposal-execute.

### `BalanceAtLeast { who, denom, amount }`

Bank-module query. Refuses completion unless a named address holds at least the stated balance in the stated denom. The use-case is treasury-funded work: attach `BalanceAtLeast { who: treasury, denom: "ujuno", amount: 10_000_000 }` and a task cannot be marked Completed in a block where the treasury has been drained below the threshold. (This is a *liveness* invariant, not a *payment* invariant — the payment invariant is `EscrowObligationConfirmed`.)

### `PairReservesPositive { pair }`

Queries a Junoswap-style pair's `Pool {}` response and refuses completion if either reserve is zero. "This task must not complete in a block where the liquidity pool has been emptied." Narrow, but the kind of narrow that saves an evening when you eventually deploy a DEX-adjacent agent and discover that someone else's arbitrage loop just drained your quote token.

### `TaskStatusIs { task_ledger, task_id, status }`

Cross-task dependency. Refuses completion unless another named task is in the stated status. This is the primitive that gives you a DAG of tasks: Task B with `TaskStatusIs { task_id: a, status: Completed }` cannot settle until Task A has settled. No separate orchestration contract is needed.

### `TimeAfter { unix_seconds }`

Reads `env.block.time`. Refuses completion until the block wall-clock reaches the threshold. A timelock. The use-case is vesting-style obligations: a proposal commits to a work-output that is only valid after a grace period during which the work-output can still be revoked.

### `BlockHeightAtLeast { height }`

Reads `env.block.height`. The block-count analogue of `TimeAfter`. Exists as a separate variant because some invariants belong in block-space rather than wall-clock: IBC acknowledgement windows, for example, are naturally denominated in blocks. Using `TimeAfter` as a proxy would break the moment validators adjust block intervals.

### `EscrowObligationConfirmed { escrow, task_id }`

Cross-contract query to `escrow::GetObligationByTask`. Refuses completion unless the obligation for the named task is in `Confirmed` status. This is the **payment invariant**: it closes the last remaining hole in the v6 coherence story by letting a task's completion become conditional on the payment journal having agreed.

An `EscrowObligationConfirmed` hook on the task and a parallel escrow `Confirmed` status are the same statement, made twice, from two different ledgers. Consistency by over-specification is the Cosmos dialect of defensive programming.

---

## What this is not

It is not an intent ledger. An intent says *what* the user wants, and leaves *who* and *how* to a solver network. A constraint says *what must hold at the moment of completion* and leaves everything else unchanged. These are adjacent ideas, not the same idea. The intent-ledger pattern — as Epoch Protocol and others have built it on EVM — is a much larger architectural commitment, and JunoClaw has deliberately not made that commitment yet.

It is not an observer registry. An observer pattern lets a contract enumerate third-party wasm modules that will be called at a point in a task's lifecycle. The pre/post hook vocabulary here is the opposite: the `task-ledger` itself evaluates every constraint, against its own `junoclaw-common` definitions, with no external code in the trust boundary.

It is not a timelock contract. A timelock contract is a separate deployment that holds funds and releases them after a condition. `TimeAfter` is a sentence on a task record; it gates completion, not custody. The two compose — a timelocked task with an `EscrowObligationConfirmed` hook effectively becomes a timelocked escrow — but the pieces are orthogonal.

And it is not a replacement for the WAVS attestation path. WAVS proves *how the work was done* (by hashing inputs and binding them to a TEE attestation). Constraints prove *under what conditions the work is accepted as done*. The first is about provenance; the second is about admissibility.

---

## Audit posture and security measures

The honest audit note on this change is that it is *deliberately thin*. A small ring around an already-hardened core is worth more than an ambitious ring that redraws the perimeter. Each of the measures below is a specific thing an auditor can check, not a general feeling of safety.

**Additivity.** Every new field on `TaskRecord` is `#[serde(default)]`; every new field on the public `ExecuteMsg::SubmitTask` is `#[serde(default)]`. Pre-v7 JSON payloads deserialise unchanged. Pre-v7 stored records deserialise unchanged. The `MigrateMsg` is empty; the migrate entry-point does nothing but bump the cw2 version tag after asserting the contract name matches — no storage rewrite, no read/write race with in-flight tasks.

**No new trust boundary.** The seven `Constraint` variants were chosen specifically because each of them queries something a v6 contract already exposed — `agent-registry::GetAgent`, the bank module, `junoswap-pair::Pool`, `task-ledger::GetTask`, `env.block.*`, and `escrow::GetObligationByTask`. Zero new cross-contract dependencies. The audit delta is the evaluator and the dispatch; the things it dispatches to were in scope already.

**Read-only evaluator.** None of the new `Constraint` variants mutate state. None of them fire sub-messages. None of them can loop or recurse. They each perform a single deterministic query and compare the answer to a literal from the `TaskRecord`. The worst an errant hook can do is return `Err` and cause a revert; the chain's invariants remain whatever they were before.

**Atomic revert, inherited.** The most important security property is not new code — it is the v5 coherence guarantee that every storage write and every sub-message inside a single entry-point either all commit or all revert together. `ConstraintViolated` is an `Err`. A CosmWasm `Err` discards every storage write in the current entry-point plus every sub-message that would have been dispatched by its `Response`. That means a hook-bearing `CompleteTask` that violates a post-hook cannot leave the task `Completed`, cannot fire the escrow-`Confirm` sub-message, and cannot increment the registry trust score. The three ledgers either all move together or none of them move. This is the single property that makes the primitive a real trust primitive.

**Error surface is non-lossy.** The `ConstraintViolated { reason }` variant wraps every evaluator diagnostic in a layered prefix — `pre_hook:` or `post_hook:` from the call-site in `execute_complete`, then `hook[i]:` from `evaluate_all`, then the variant name and comparison values from each `Constraint::evaluate` arm. A tripped `TimeAfter` during pre-hook evaluation surfaces as `Constraint violated: pre_hook: hook[0]: TimeAfter: block time N < required M`. The first-failure-wins regression test asserts the index prefix surfaces correctly even when multiple hooks would fail. An operator reading a reverted tx knows *which* hook tripped, *where* in the lifecycle, and *why* — not just that something did.

**Bounded vocabulary, not user code.** The seven variants live in `junoclaw-common` and are spelled out in the audited `enum`. There is no user-supplied callback address, no user-supplied wasm module, no sandbox. The cost of this is that expressiveness is bounded by the `enum`; the benefit is that the audit is bounded by the `enum`.

**Migration is admin-gated.** The live `task-ledger` at `juno17aq…` was instantiated with `The Builder` as its wasmd-level admin. A migrate tx signed by anyone else is rejected at the chain layer before the contract is even touched. The `deploy/migrate-tier15.mjs` script pre-flights the admin match with `client.getContract(addr)` and hard-fails with a diagnostic before broadcasting — so an operator misconfigures a local env var, they learn about it in a console message, not in a burned tx fee.

**Rollback is cheap.** The migrate script records the prior `code_id` in `deployed.json` as `pre_tier15_code_id`. If the Tier 1.5 code_id ever needs to be rolled back, it is a second migrate to the prior code_id, and pre-v7 records are untouched because nothing in the v7 code wrote anything new to them.

**Test surface, concrete.** Eleven regression tests cover the new surface: four for the Tier 1-slim variants, three for Tier 1.5, two for revert-on-failure semantics, one for first-failure-wins diagnostic indexing, and one for hook-bearing `SubmitTask` round-trip through storage. All eleven run inside `cw-multi-test`, which simulates the same atomic-revert semantics wasmd runs in production. The suite stands at 148 tests across the workspace, 27 of them on the contract this change touches.

**What this does not remove.** None of the above replaces an external auditor. It places the delta close to the baseline — a small ring around an already-hardened core — but the ring still needs to be read by someone who did not write it.

---

## Risk matrix and mitigations

Eight risks the change exposes, in rough order of blast radius, each paired with the thing we did about it.

**1. Hook evaluator misreads `env.block`.**
Would cause: `TimeAfter` / `BlockHeightAtLeast` tasks to unlock at the wrong moment — either early (funds released before timelock) or never (funds trapped).
Mitigation: two regression tests explicitly advance `app.block.time` and `app.block.height` with `app.update_block`, submit the task *before* advancement, try completion (expect revert), advance, retry (expect pass). Proves the evaluator reads `env.block.*` fresh at each `execute_complete`, not cached from submit.

**2. Escrow stub drift in production.**
Would cause: `EscrowObligationConfirmed` to pass when it should fail, or fail when it should pass, because our test stub's query shape drifted from the real escrow's.
Mitigation: the `StubEscrow` in tests responds to the exact `GetObligationByTask { task_id }` → `Option<PaymentObligation>` shape defined in `escrow::msg::QueryMsg` and `junoclaw_common::PaymentObligation`. Both the stub and the evaluator deserialise into the same `PaymentObligation` struct from `junoclaw-common`, so any future schema drift on the real escrow would cause a compile-time error in the task-ledger before it could cause a runtime lie in production.

**3. Gas exhaustion through hook chain.**
Would cause: a task with many hooks could exceed the block gas limit and revert every time, becoming un-completable.
Mitigation at the primitive level: each hook is O(1) — a single cross-contract query or a single read of `env.block`. The `Vec<Constraint>` is stored in the task record at submit time, so gas cost is visible at submit. Operational mitigation: submitters are expected to keep hook lists short; observation window will surface the real distribution.

**4. Submitter griefing via hook that can never pass.**
Would cause: submitter attaches `BalanceAtLeast { who, denom, amount: u128::MAX }`, task becomes permanently un-completable, agent's trust score can't increment from it.
Mitigation: `CancelTask` remains available to the submitter. A task that cannot be completed can still be cancelled, at which point the agent's stats are unaffected. The submitter pays the submit tx fee for the privilege of self-griefing; no one else is harmed.

**5. Cross-contract dependency where the queried contract is migrated out from under us.**
Would cause: `AgentTrustAtLeast` queries `agent-registry` by an address stored at submit time; if the registry is migrated to a schema that changes `GetAgent`'s response, evaluator starts returning stale-or-wrong answers.
Mitigation: two-layer. (a) `agent-registry` is under the same admin; a migration there would be coordinated with a task-ledger migration if it changed `GetAgent`'s shape. (b) Every query in the evaluator uses `.map_err(|e| format!(...))?` rather than `unwrap` or `expect` — any query failure surfaces as a `ConstraintViolated` with a diagnostic that names the failing contract and the failing call, not a contract panic. Failure mode is loud, not silent.

**6. Re-entrancy via a Constraint that queries a contract that queries task-ledger.**
Would cause: a hypothetical hook that queries contract X, which itself queries task-ledger during its response handler, could in principle produce an unexpected read.
Mitigation: all seven variants' queries are `query_wasm_smart` (read-only, no sub-message fan-out). CosmWasm's read-only query entrypoints cannot mutate state, and the task-ledger's queries are all idempotent reads of `TASKS`. Even a malicious target contract in the query path can only observe, not mutate.

**7. The `proposal_id` field on `SubmitTask` and the hook fields interact with `agent_company` uniqueness checks.**
Would cause: if the v6 proposal-id gating and the new hook fields disagreed about uniqueness, a proposal could be double-consumed.
Mitigation: `TASKS_BY_PROPOSAL` is still the sole index for `proposal_id` uniqueness, enforced on the `execute_submit` path before hooks are even parsed. Hooks play no role in submission admission; they only gate completion. Keeping these concerns separated is deliberate.

**8. Schema round-trip failure on migrate.**
Would cause: a pre-v7 stored `TaskRecord` with no `pre_hooks` / `post_hooks` fields fails to deserialise after the migrate, bricking the contract for all pre-existing tasks.
Mitigation: every new field is `#[serde(default)]`, which in cosmwasm-schema's serde configuration instantiates as `Vec::new()` on absence. The Tier 1.5 tests include a submit-with-no-hooks path that round-trips through storage. On uni-7, `migrate-tier15.mjs` step 3 queries `GetConfig` immediately after migrate and asserts the v6 `agent_company` field is still populated — proving serde round-trip held.

---

## On observability, and what tx log grep can and cannot carry

The observation window is worth nothing if we can't see what happened. The current instrumentation plan is to grep transaction failure logs for `ConstraintViolated` strings. This is the cheapest possible starting point, and it is also a plan with real limits. We should be clear-eyed about where those limits bite.

**What works about the grep approach.**

The error string surfaces the pre/post position, the hook index, the variant, and the comparison values that tripped, e.g. `Constraint violated: pre_hook: hook[0]: TimeAfter: block time 1745190000 < required 1745200000`. This is already enough to build a dashboard with three things on it: count of `Constraint violated:` per day, distribution across the seven variant names, and the ratio of violated to successful hook-bearing completions. Every cosmjs client already surfaces this string on a failed tx via `error.message`. Every public explorer (Mintscan, ping.pub) already shows it. The integration cost is zero.

**What does not work about it, and where it fails first.**

*Success-side blindness.* CosmWasm discards the `Response` (including every `add_attribute`) when an entry-point returns `Err`. That is the invariant that makes atomic revert safe in the first place, and we should not wish it away. But it means *violations are observable only in tx failure logs, and successes are not observable at all* without a separate signal. You can count the tasks that failed their hooks. You cannot, with this approach alone, count the tasks that *had* hooks and passed them. Which means you can't compute the denominator — and a ratio without a denominator is just a tally.

*Log-format fragility.* The error string is produced by three layers: the per-variant `format!` inside each `Constraint::evaluate` arm, the `hook[i]: {inner}` wrapper in `evaluate_all`, and the `pre_hook:` / `post_hook:` wrapper at the call-site in `execute_complete`; the outer `Constraint violated: ` prefix is added by the thiserror `Display` impl on `ContractError::ConstraintViolated`. Any refactor to any of those four layers could silently change the string shape. The mitigation is easy and is now in place (see `test_v7_constraint_violated_error_string_shape_is_stable` in `@junoclaw/contracts/task-ledger/src/tests.rs`): the regression test asserts the full layered prefix `Constraint violated: pre_hook: hook[0]: TimeAfter:` and `Constraint violated: post_hook: hook[0]: BlockHeightAtLeast:` and the detail substrings. A test that fails on format drift is a test that lets a dashboard sleep.

*RPC / wasmd coupling.* Juno has upgraded wasmd several times across its lifecycle. Each upgrade has, at some point, touched the shape of `tx_result.log`, the event-attribute prefixing, or the way cosmjs surfaces revert reasons. A grep-based dashboard coupled to today's format is a dashboard that will silently break on the next wasmd minor. Contingency: either pin the RPC to a fixed wasmd version during the observation window, or add an integration test that exercises the grep against a real `uni-7` revert output monthly.

*Archive retention.* Parsing tx failure logs for the 4-8 week window is fine if we own an archive node; public RPCs retain a limited window. If we rely on Polkachu and the window extends, we lose the historical tail. Contingency: either stand up our own archive node for the window, or periodically snapshot the failure-log grep to our own store.

*Scale.* A few hundred hook-bearing txs a day, grep is fine. A few thousand, we are building a Tendermint log indexer. A few hundred thousand, we are Mars Protocol and we have a full tx-indexing stack. The scale breakpoint is probably around 10,000 txs/day before grep becomes a materially bad idea; JunoClaw is nowhere near that, and if it ever is, the constraint-primitive design will not be the bottleneck — the decision to indexerise will have been made months earlier for other reasons.

**The cheap instrumentation that closes the success side, without touching trust.**

Add `.add_attribute("pre_hooks_count", …)` and `.add_attribute("post_hooks_count", …)` to the success-path `Response` in `execute_complete`. This is a one-line additive change. It mutates no state, changes no authorisation, touches no revert behaviour. Its entire effect is to emit two integers into the event log on every successful completion, which every CosmWasm indexer in the ecosystem already knows how to aggregate. With this in place, the success denominator becomes free, and the ratio becomes real.

It does have a cost — it is a contract change, which means a re-migrate, which means the Tier 1.5 migration delta grows by one more attribute. The question is whether the cost of one more line in the audit delta is lower than the cost of not knowing the denominator for eight weeks. Our current answer is: defer for Tier 1.5 shipping, re-evaluate after two weeks of observation. If we find ourselves using the grep output and wishing we had the denominator, we add the attributes then. If the grep data is clearly sufficient, we save the re-migrate.

**The future-proof move, parked.**

The real future-proof answer is a storage counter: a `HOOK_USAGE: Map<String, u64>` inside `task-ledger` that increments per variant on every successful `evaluate_all`. This makes both success and violation observable from contract storage via a dedicated query, independent of tx logs, event formats, RPC uptime, archive retention, and indexer availability. It costs a small amount of gas per complete and a small amount of storage per variant. It is also a larger audit delta than a simple `add_attribute`. It is the right answer at the scale where we have enough traffic that tx-log grep is actively in the way; until then, it is over-engineered and we should not ship it.

The audit discipline here is: *ship the cheapest signal that answers the current question, and carry a written record of the step we will take when the current signal fails*. That record lives in `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`, which is what lets us know when grep has stopped being enough.

[IMAGE 3 — *Hand-drawn pencil, vintage.* A draughtsman's geometric instrument — a pair of brass dividers — resting on a ledger page, the page ruled faintly with graphite lines; a smudge of soot from a finger, an inkwell with a single feather quill alongside. Caption-style: "To measure is to admit what cannot be measured." Pencil shading deep along the edges of the dividers, the brass of the joint catching a single highlight.]

---

## What happens next

The code is in the repository, the wasm is built, the migration script is written, and the on-chain smoke test is ready to run. What has not yet happened is the migrate tx itself — that requires the wasmd-level admin's signature, which is a human decision, not a code change. Once that decision is taken, the next phase is observation: migrate `task-ledger` on `uni-7` to the Tier 1.5 code_id, migrate the two live agents to use hook-bearing submissions for the specific work they already do (the attestation pipeline is the obvious first candidate), and sit with the telemetry for four to eight weeks.

The things we will be watching for, in priority order:

- **Which hooks, in practice, fire**. A constraint that is never used in a real submission is a constraint that does not need to exist. We will prune what does not pay rent.
- **Which hooks surface failures**. A constraint that consistently fires on its violation path is, by definition, catching something — and is therefore *worth* its audit weight. A constraint that never fails is either unnecessary or redundant.
- **Which invariants the vocabulary did not capture**. Real agents will ask for things. Some of those things will be another variant (cheap); some will be a new contract (expensive). The Tier 2 question — intent-ledger? observer-registry? — becomes a question evidence can answer, not a question speculation has to answer.

We are deliberately not adopting an intent-solver architecture today. The argument for it is real but speculative; the cost of it is real and immediate. We are accepting that we may be leaving expressive power on the table for a few weeks, and that that is an acceptable price to pay for knowing what kind of expressive power our users actually want.

Shipping small, shipping audit-readable, shipping things you can take back: the Tier 1-slim discipline.

---

*The Constraint enum ships in `@junoclaw/contracts/junoclaw-common/src/lib.rs`. The evaluator and its tests live in `@junoclaw/contracts/task-ledger/src/`. The full regression suite currently stands at 148 passing tests across the workspace. No tests were weakened to make room for the new surface; every v6 invariant from `RATTADAN_HARDENING.md` remains in force.*

[IMAGE 4 — *Hand-drawn pencil, vintage.* A leather-bound ledger lying closed on a weathered desk, its brass clasp shut; fog lifting over a low hill framed by the leaded panes of a narrow window; a brass oil lamp at the edge of the desk, wick trimmed. Soft graphite for the fog, harder pencil on the brass, fine cross-hatching over the leather grain. The drawing breathes patience.]
