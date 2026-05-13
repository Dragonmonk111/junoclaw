# Task-Ledger — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark. Anchor commit: `a22e496` on origin/main. Files read in full: `src/contract.rs` (683 lines), `src/state.rs` (35 lines), `src/error.rs` (36 lines), `src/msg.rs`, plus `TaskRecord`, `Constraint`, `evaluate_all` in `contracts/junoclaw-common/src/lib.rs`.*

---

## 0. Surface summary

Task-ledger is the **central coordinator** in the JunoClaw stack. It tracks the lifecycle of every agent task, fires atomic cross-contract callbacks to `escrow` (Confirm/Cancel) and `agent-registry` (IncrementTasks), and evaluates v7 hook constraints (`pre_hooks` / `post_hooks`) at completion time.

| Area | Lines | Functions |
|------|-------|-----------|
| Entry points | 18-145, 621-682 | `instantiate`, `execute`, `query`, `migrate` |
| Submit | 147-249 | `execute_submit` (proposal-id branch + agent-id branch + reserved-id branch) |
| Lifecycle | 251-396, 398-482, 484-508 | `is_authorized`, `execute_complete`, `execute_fail`, `execute_cancel` |
| Operator mgmt | 510-544 | `execute_add_operator`, `execute_remove_operator` |
| Admin / config | 546-619 | `execute_update_config`, `execute_update_registry` |
| Read-side | 621-672 | 7 query branches incl. `GetTaskByProposal` reverse index |

**State surface:**

| Storage item | Key | Value | Notes |
|---|---|---|---|
| `CONFIG` | singleton | `Config` (~300 bytes) | admin + operators Vec + registry |
| `TASKS` | u64 | `TaskRecord` (~variable) | up to ~1KB with hooks |
| `TASKS_BY_AGENT` | u64 | `Vec<u64>` | unbounded per agent; F3 |
| `TASKS_BY_SUBMITTER` | `&Addr` | `Vec<u64>` | unbounded per submitter; F3 |
| `TASKS_BY_PROPOSAL` | u64 | u64 | reverse index; one entry per WavsPush task |
| `NEXT_TASK_ID` | singleton | u64 | starts at 1 |
| `LEDGER_STATS` | singleton | `LedgerStats` (24 bytes) |  |

**No prefix collisions.** ✅

---

## 1. Atomic Callback Topology — the central design property

Task-ledger's most consequential design decision is the **single-tx atomicity** of completion / failure: a `CompleteTask` execute appends sub-messages to fire `escrow::Confirm` and `agent-registry::IncrementTasks`. CosmWasm's default `ReplyOn::Never` means any sub-msg revert kills the whole tx — including the `TASKS.save(Completed)` write at line 311. Same property holds for `FailTask` → `escrow::Cancel` + `IncrementTasks(success: false)`.

The implication is enormous: **the chain enforces that "task is Completed in task-ledger" and "obligation is Confirmed in escrow" and "agent.successful_tasks++ in registry" are either all-true or all-false.** No window where one is true and the others aren't. The audit-trail layer this contract anchors is therefore consistent by construction, not by post-hoc reconciliation.

This is the right design. Several findings below are about preserving this property as the contract evolves.

---

## 2. Failure Mode Enumeration

### 🟡 F1 — `CancelTask` leaves orphaned escrow obligations

**Location:** `execute_cancel` lines 484-508. No escrow callback is fired on cancel.

**The deliberate omission.** The code pattern is intentionally asymmetric: `FailTask` fires `escrow::Cancel` (lines 468-478), `CompleteTask` fires `escrow::Confirm` (lines 358-368), but `CancelTask` fires nothing. The implicit reasoning (not commented in the code) is **debt-evasion prevention**:

> If `CancelTask` fired `escrow::Cancel`, the submitter — who CAN cancel their own task (line 493) — could submit a task that triggered an Authorize-side obligation, then cancel their own task to void the obligation, all in two txs from the same wallet.

The `FailTask` path is safe because it requires operator/admin/agent-company auth (line 414) — third-party verdict, not self-declaration. Same for `CompleteTask` (line 274). `CancelTask` is the only finalisation path the submitter can trigger.

**The unintended consequence.** Cancelling a task with a live obligation leaves the obligation **stranded in `Pending` state forever**. Reproducer:

1. `agent-company` executes `WavsPush` proposal P with `escrow_amount > 0`.
2. Sub-messages fire: `task-ledger::SubmitTask(proposal_id: P, agent_id: 0)` + `escrow::Authorize(task_id: P, payee: admin, amount: X)`.
3. Task is Running. Escrow obligation is Pending.
4. Days pass. The DAO decides the task is no longer needed. `agent-company` (the submitter) calls `task-ledger::CancelTask(task_id)`.
5. Task → Cancelled. **Obligation still Pending. Forever.**

**Severity: LOW-MEDIUM** — not exploitable for theft, but a real lifecycle hole. Stranded obligations corrupt the audit trail and confuse downstream readers (a Pending obligation with no Running task is incoherent).

**Suggested fix.** Two-step:

1. **Document the asymmetry.** Add a doc-comment at `execute_cancel` explaining why escrow.Cancel is not fired here, with the debt-evasion reasoning made explicit.

2. **Add an admin-gated cancel path.** Introduce `ExecuteMsg::AdminCancelTask { task_id }` that *does* fire `escrow::Cancel`, restricted to operators/admin/agent-company. Submitters lose nothing (they keep the existing CancelTask), but the DAO/operator gains the ability to clean up stranded obligations. ~30 LoC + 2 tests.

Alternative (cleaner but larger): make `CancelTask` admin-only when an associated obligation exists, by querying escrow for `GetObligationByTask` first. Adds a query to the cancel path — minor gas cost.

**Regression test.**
```
fn admin_cancel_clears_stranded_obligation() {
    // wire escrow + agent-company
    // execute WavsPush proposal with escrow_amount > 0
    // call AdminCancelTask
    // assert task is Cancelled AND escrow obligation is Cancelled
}
```

---

### 🟢 F2 — `CancelTask` only rejects `Completed`, not `Failed` / `Cancelled`

**Location:** `execute_cancel` lines 491-503.

**The flaw.** The status check is `if record.status == TaskStatus::Completed { return Err(TaskAlreadyCompleted) }`. This means:

- Cancelling a `Failed` task transitions it to `Cancelled`, losing the Failed history.
- Cancelling an already-`Cancelled` task is silently a no-op (status set to Cancelled again).
- Cancelling a `Pending` task transitions it (but no `Pending` exists today — submit goes straight to Running).

**Concrete impact.** If a task fails (operator marks Failed, escrow.Cancel fires, registry.Increment(success=false) fires, trust_score is unaffected), the submitter can then call CancelTask which transitions the task to Cancelled. The Failed verdict is **lost**. `LEDGER_STATS.total_failed` already incremented, but the task itself no longer reads as Failed when queried.

**Severity: LOW** — data-integrity nit, no value flow exposure, no callback re-fire (cancel doesn't fire escrow). But it lets a submitter rewrite the public record of their failed task.

**Suggested fix (1-line).**
```rust
// Change:
if record.status == TaskStatus::Completed {
    return Err(ContractError::TaskAlreadyCompleted { task_id });
}
// To:
if record.status != TaskStatus::Running {
    return Err(ContractError::TaskNotRunning { task_id });
}
```

Mirrors the same check in `execute_complete` (line 288) and `execute_fail` (line 422).

---

### 🟢 F3 — `TASKS_BY_AGENT` and `TASKS_BY_SUBMITTER` are unbounded Vec maps

**Location:** `state.rs` lines 27-28; `execute_submit` lines 217-227.

**The flaw.** Identical pattern to `agent-registry` F6. Each `SubmitTask` does:
```rust
TASKS_BY_AGENT.update(deps.storage, agent_id, |existing| {
    let mut ids = existing.unwrap_or_default();
    ids.push(task_id);
    Ok(ids)
})?;
```

The full `Vec<u64>` is read, mutated, and written back. At K = 10,000 tasks per agent, that's a 10K-entry Vec read + write per submit (~80KB serialised). Linear gas in K.

**Severity: LOW** — gas naturally throttles; no exploit. But tasks-per-agent in a busy DAO can realistically hit thousands within a year.

**Suggested fix.** Same as agent-registry F6: refactor to composite-key `Map<(u64, u64), ()>` for `TASKS_BY_AGENT` and `Map<(&Addr, u64), ()>` for `TASKS_BY_SUBMITTER`. Append cost becomes O(1). Iteration cost in queries becomes the same as today (K db_reads). Migration: write a one-shot migrate handler that converts on next deploy. ~50 LoC + migration + 2 tests.

This is a v1 refactor, not a v0 fix.

---

### 🟢 F4 — `escrow_key = task.proposal_id.unwrap_or(task_id)` couples two id namespaces

**Location:** `execute_complete` line 356; `execute_fail` line 472.

**The collision.** When a task has `proposal_id = Some(P)`, escrow callbacks key the obligation by `P`. When `proposal_id = None`, they key by the local `task_id`. **Both namespaces share the same u64 keyspace in escrow.**

Concrete collision scenario (constructed, not currently exploitable):

1. agent-company executes proposal **P=5** → submits task with `task_id=1, proposal_id=5`. Escrow obligation is keyed by `5`.
2. Operator submits an unrelated task → `task_id=5, proposal_id=None`. If escrow.Authorize were called for this task, it would key by `5` — clashing with proposal P's obligation.

**Why it's not currently exploitable.** Only `agent-company::WavsPush` triggers `escrow.Authorize` today. There is no path where an operator-submitted task fires Authorize (operators just submit tasks; escrow Authorize is governance-only). So the namespace overlap is theoretical.

**Why it's worth flagging.** The first time someone adds a non-governance Authorize path (e.g. "operator can fund their own task"), the namespace collision becomes real. Defensive depth: prefix the keys.

**Severity: LOW** — currently safe by other invariants, but a fragile coupling.

**Suggested fix.** In escrow, key obligations by `enum ObligationKey { Task(u64), Proposal(u64) }` instead of raw u64. Encode the variant tag explicitly in the storage key. Same fix in task-ledger when it asks escrow for an obligation. ~40 LoC across both contracts + migration + 2 tests.

---

### 🟢 F5 — `agent_id == 0` sentinel coupling

**Location:** `execute_submit` line 174; `execute_complete` line 378; `execute_fail` line 448.

**The implicit assumption.** `agent_id == 0` is treated as "no specific agent" (governance-initiated WavsPush tasks). Three sites skip the registry callback when `agent_id == 0`:

- `execute_complete`: `if task.agent_id != 0 { ... IncrementTasks ... }`
- `execute_fail`: same pattern
- `execute_submit`: `if agent_id == 0 && !is_operator { Err(ReservedAgentId) }`

**Why it's safe today.** `agent-registry::instantiate` sets `NEXT_AGENT_ID = 1u64`, so agent 0 cannot be created. The sentinel is unambiguous.

**Why it's fragile.** If a different agent-registry contract gets wired via `UpdateRegistry` — e.g. a v2 agent-registry that uses 0-indexing — agent 0 might be a real agent, and the skip would silently drop legitimate `IncrementTasks` calls.

**Severity: NONE** (currently safe by construction) but worth flagging.

**Suggested fix.** Replace the sentinel with `Option<u64>`. `TaskRecord.agent_id: Option<u64>` instead of `u64`. None means "no agent bound". Explicit, no magic-value coupling. ~30 LoC + migration handler that maps stored `agent_id == 0` to `None`. Documentable as "v1 cleanup."

---

### 🟢 F6 — `post_hook` self-query semantics

**Location:** `execute_complete` lines 318-339; `Constraint::TaskStatusIs` evaluation in `junoclaw-common/src/lib.rs:344-370`.

**The claim in the comment** (line 322-324):
> "post_hooks see: the task's new `Completed` status (visible via self-query)"

**The concern.** A `post_hook` containing `TaskStatusIs { task_ledger: <self_address>, task_id: <same_id>, status: Completed }` issues a `WasmQuery::Smart` to the contract's own query handler, which reads `TASKS.may_load(task_id)`. The question is whether the mid-handler write at line 311 (`TASKS.save(deps.storage, task_id, &record)?`) is visible to the subsequent self-query at line 330.

**CosmWasm semantics (from `cosmwasm-vm`'s `query_chain` plumbing):** `deps.querier.query()` for `WasmQuery::Smart` invokes the target contract's `query` entry point with read-only `Deps` over the **same in-progress transaction's storage**, including writes already made in the parent handler. So mid-handler writes ARE visible to subsequent self-queries.

**Verdict.** The comment is correct. The post_hook can see the new `Completed` status when self-querying. ✅

**Why I'm flagging it.** This is a non-obvious semantic. A future maintainer who refactors the post_hook flow to use `add_submessage` with `ReplyOn::Always` (e.g., to depend on escrow.Confirm having succeeded) will lose this property — replies fire AFTER the parent returns, in a separate transaction frame. Document this invariant in the post_hook block comment.

**Severity: NONE** (correct by design) but worth a doc-comment to harden future refactors.

---

### 🟢 F7 — Operator list is `Vec<Addr>` with linear scan auth

**Location:** `state.rs` line 12; `is_authorized` lines 251-253; `execute_add_operator` line 520.

**The pattern.** `config.operators: Vec<Addr>`, scanned linearly via `.contains()`. For N operators, every `execute_*` call pays O(N) auth cost.

**Severity: NONE** — N is realistically ≤ 10 (Operators are daemon wallets). Even at 100 operators, the scan cost is ~5K SDK gas. Not worth refactoring to a Map.

---

### 🟢 F8 — `execute_fail` does an extra `TASKS.load` after `TASKS.update`

**Location:** lines 420-438.

**Cosmetic.** `TASKS.update(...)` returns `()` on the cw-storage-plus signature used here, so `task.agent_id` and `task.proposal_id` are re-fetched via a second `TASKS.load(task_id)` at line 438. One extra db_read (~3K SDK gas).

**Severity: NONE / Cosmetic.** Could capture in the update closure to avoid the second read. Not worth a PR on its own.

---

### 🟢 F9 — `execute_submit` does no upper bound on `pre_hooks` / `post_hooks` length

**Location:** `execute_submit` lines 156-157, 210-211.

**The flaw.** A submitter can attach an arbitrarily long `Vec<Constraint>`. At completion time, each constraint that triggers a cross-contract query costs ~15K-30K SDK gas. With 100 hook constraints, completion costs a few million SDK gas.

The cost is paid by the *completer* (operator/admin/agent-company), not the submitter. So a malicious submitter could attach 1,000 hooks to grief operators with completion costs.

**Reproducer.**
1. Operator wallet has X gas budget configured per call.
2. Attacker submits a task with 1,000 trivial hooks (e.g. `BlockHeightAtLeast { height: 0 }`).
3. Operator tries to CompleteTask. Tx exceeds gas budget. Operator pays gas-up-to-limit and the tx reverts.
4. Repeat. Operator's gas budget is drained.

**Severity: LOW** — operator-griefing, not theft. Mitigated by the fact that submission requires ownership of an active agent (line 191), which costs gas to register. So the attacker pays per-attack.

**Suggested fix.** Cap hooks at a reasonable bound (say 16 each):
```rust
const MAX_HOOKS: usize = 16;
if pre_hooks.len() > MAX_HOOKS || post_hooks.len() > MAX_HOOKS {
    return Err(ContractError::TooManyHooks { /* ... */ });
}
```
~5 LoC + 1 test.

---

### 🟢 F10 — `proposal_id` is trusted, never validated against agent-company

**Location:** `execute_submit` lines 167-173.

**The flaw.** When `proposal_id: Some(pid)` is supplied, the only check is `is_agent_company` (sender == configured agent_company addr). The `pid` itself is not validated against agent-company's storage — task-ledger trusts that any pid agent-company sends is real.

**Why it's safe today.** Only `agent-company::execute_execute_proposal::WavsPush` constructs the `SubmitTask` call, and it always passes the just-executed proposal's id. So pid is always a real proposal.

**Why it's fragile.** Future code paths in agent-company (or a misconfigured agent_company contract) could submit tasks with arbitrary pids. Task-ledger has no way to detect this.

**Severity: NONE** (currently safe by other invariants) but defensive-depth worth noting.

**Suggested follow-up (not a fix).** If we ever want defence-in-depth, task-ledger could query `agent-company::GetProposal { id: pid }` to confirm the proposal exists and was executed. Adds ~15K SDK gas to submit. Optional; current design is fine for v0.

---

## 3. Gas Trace — hot paths

### 3.1 `execute_submit` (operator path, no proposal_id)

| Step | Op | Gas est. |
|---|---|---|
| `CONFIG.load` | 1 db_read | ~3,500 |
| Auth check | linear scan over operators | ~50N |
| `NEXT_TASK_ID.load` + save | 2 db | ~5,000 |
| `TASKS.save` | 1 db_write | ~5,000 |
| `TASKS_BY_AGENT.update` | 1 read + 1 write | ~5,000 + O(K×50) |
| `TASKS_BY_SUBMITTER.update` | 1 read + 1 write | ~5,000 + O(M×50) |
| `LEDGER_STATS.update` | 2 db | ~5,000 |

**Total at K=M=0:** ~28K SDK → ~84K with multiplier. Cheap. ✅

### 3.2 `execute_submit` (public agent-id path)

Adds 1 cross-contract query to agent-registry (~20K SDK) + the ownership check.

**Total:** ~48K SDK → ~144K with multiplier. Fine. ✅

### 3.3 `execute_complete` (with H hooks + escrow + registry callbacks)

| Step | Gas est. |
|---|---|
| Parent handler base | ~15K SDK |
| `evaluate_all(pre_hooks)` | H × ~20K SDK (per cross-contract query hook) |
| `evaluate_all(post_hooks)` | H × ~20K SDK |
| escrow.Confirm sub-msg dispatch | ~5K + downstream escrow gas |
| registry.IncrementTasks sub-msg dispatch | ~5K + ~10K downstream |

**Total at H=0:** ~35K SDK → ~105K with multiplier. ✅
**Total at H=8 (4 pre + 4 post):** ~195K SDK → ~585K with multiplier. ✅
**Total at H=32:** ~675K SDK → ~2M with multiplier. Approaching the budget — see F9.

---

## 4. Determinism Proof

| Concern | Status |
|---|---|
| No floats | ✅ |
| No HashMap iteration | ✅ — all `Map<>` from cw-storage-plus (BTreeMap-backed) |
| No `std::time` | ✅ — uses `env.block.time` |
| Cross-contract queries are deterministic | ✅ — within-block CosmWasm semantics |
| Hook constraint evaluator | ✅ — `evaluate_all` iterates `&[Constraint]` in declaration order, returns first violation. Deterministic regardless of HashMap. |
| Sub-message ordering | ✅ — Response.messages dispatched in append order |
| Serde stability | ✅ — `cw_serde` + `#[serde(default)]` on `proposal_id`, `pre_hooks`, `post_hooks` (additive) |

**All clear.** ✅

---

## 5. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | **LOW-MEDIUM** | Add admin-gated `AdminCancelTask` that fires escrow.Cancel; document the submitter-cancel asymmetry | ~30 LoC + 2 tests |
| F2 | LOW | Tighten `CancelTask` status check to `!= Running` | 1-line change + 1 test |
| F3 | LOW | Refactor `TASKS_BY_AGENT` / `TASKS_BY_SUBMITTER` to composite-key Maps | ~50 LoC + migration + 2 tests |
| F4 | LOW | Prefix escrow obligation keys with namespace tag (Task/Proposal) | ~40 LoC + migration + 2 tests |
| F5 | LOW | Replace `agent_id == 0` sentinel with `Option<u64>` | ~30 LoC + migration |
| F6 | NONE | Add doc-comment hardening the post_hook self-query invariant | docs only |
| F7 | NONE | — | — |
| F8 | NONE | (cosmetic) capture task in update closure to skip extra read | 5 LoC |
| F9 | LOW | Cap `pre_hooks` / `post_hooks` length at 16 each | ~5 LoC + 1 test |
| F10 | NONE | — | — |

**Recommendation.** Land **F1 + F2 + F9** in a single follow-up PR (`task-ledger-v0.2`). F3/F4/F5 are v1 refactors with migration. F6 is docs-only. F7/F8/F10 are no-ops or aesthetics.

---

## 6. Comparative summary across the JunoClaw stack

| Contract | Audit status | Headline finding | Severity |
|---|---|---|---|
| `agent-company` | DONE (`a22e496`) | Vote weights not snapshotted at proposal creation | **HIGH** |
| `agent-registry` | DONE (`a22e496`) | Registration fees trapped (no withdraw path) | **MEDIUM** |
| `task-ledger` | DONE (`a22e496`, this doc) | CancelTask leaves orphaned escrow obligations | **LOW-MEDIUM** |
| `escrow` | pending | TBD | TBD |
| `zk-verifier` | pending (production on uni-7) | TBD | TBD |
| `moultbook-v0` | written deterministically from day 0 | None | None |

**Cross-cutting finding.** Three of three audited contracts have the same bug class: **`Map<&Addr, Vec<T>>` or `Map<u64, Vec<u64>>` indices that grow unboundedly per key**. agent-registry F6, task-ledger F3, and the same pattern is likely in escrow. Worth a single sweep PR (`migrate-vec-indices-to-composite-keys`) once we confirm the escrow shape.

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark. This audit will be re-anchored as a Moultbook entry once Moultbook v0 is on devnet, citing `task-ledger` commit `a22e496` as the audit subject.*
