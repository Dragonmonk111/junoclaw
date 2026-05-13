# Agent-Registry — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark. Anchor commit: `a22e496` on origin/main. Files read in full: `src/contract.rs`, `src/state.rs`, `src/msg.rs`, `src/error.rs`, plus the `AgentProfile` shared type at `contracts/junoclaw-common/src/lib.rs:9-21`. Cargo-level: `agent-registry v0.1.0`.*

---

## 0. Surface summary

This is a **small, focused contract** — 419 lines of `contract.rs`, 27 lines of `state.rs`. Compared to `agent-company` (60K+ lines) it is the simplest contract in the JunoClaw stack.

| Area | Lines | Functions |
|------|-------|-----------|
| Entry points | 15-115, 380-418 | `instantiate`, `execute`, `query`, `migrate` |
| Lifecycle | 117-185, 187-221, 223-252 | `execute_register`, `execute_update`, `execute_deactivate` |
| Task accounting | 254-286 | `execute_increment_tasks` |
| Reputation | 288-311 | `execute_slash_agent` |
| Admin / config | 313-378 | `execute_update_config`, `execute_update_registry` |
| Read-side | 380-408 | 5 query branches: `GetConfig`, `GetAgent`, `GetAgentsByOwner`, `GetStats`, `ListAgents` |

**State surface:**

| Storage item | Key | Value |
|---|---|---|
| `CONFIG` | singleton | `Config` (~200 bytes) |
| `AGENTS` | u64 | `AgentProfile` (~variable, ~250 bytes) |
| `AGENT_BY_OWNER` | `&Addr` | `Vec<u64>` (unbounded, see F6) |
| `NEXT_AGENT_ID` | singleton | u64 |
| `AGENT_STATS` | singleton | `AgentStats` (16 bytes) |

**No prefix collisions.** ✅ (`config`, `agents`, `agent_by_owner`, `next_agent_id`, `agent_stats` all distinct.)

---

## 1. Gas Trace — hot paths

### 1.1 `execute_register`

| Step | Op | Host calls | SDK gas est. |
|---|---|---|---|
| `set_contract_version` skipped (instantiate-only) | — | 0 | 0 |
| `CONFIG.load` | 1× db_read | 1 | ~3,500 |
| `AGENT_STATS.load` | 1× db_read | 1 | ~2,000 |
| Limit / fee checks | pure wasm | 0 | ~100 |
| `NEXT_AGENT_ID.load` + save | 2× db | 2 | ~5,000 |
| `AGENTS.save` (~250 bytes) | 1× db_write | 1 | ~5,000 |
| `AGENT_BY_OWNER.update` | 1× db_read + 1× db_write (Vec append) | 2 | ~5,000 + O(N×50) where N = prior agents owned |
| `AGENT_STATS.update` | 1× db_read + 1× db_write | 2 | ~5,000 |

**Total at N=0:** ~26K SDK gas → ~78K with multiplier.
**Total at N=100 (heavy owner):** ~26K + 100×50 = ~31K SDK → ~93K with multiplier.

Cheap. ✅

### 1.2 `execute_increment_tasks` — task-ledger callback

| Step | Op | Host calls | SDK gas |
|---|---|---|---|
| `CONFIG.load` | 1 | 1 | ~3,500 |
| Auth check (sender == task_ledger or admin) | pure wasm | 0 | ~50 |
| `AGENTS.update` | 1× db_read + 1× db_write | 2 | ~7,000 |

**Total:** ~10.5K SDK → ~32K with multiplier. Hot-path cheap. ✅

### 1.3 `query::GetAgentsByOwner` — N-bounded by owner's count

| Step | Op | Host calls | SDK gas |
|---|---|---|---|
| `addr_validate` | 1× api_call | 1 | ~1,000 |
| `AGENT_BY_OWNER.may_load` | 1× db_read | 1 | ~3,000 |
| `AGENTS.may_load` × K (K = ids.len()) | K× db_read | K | K × ~3,000 |

**At K=10:** ~34K SDK → ~102K. Fine.
**At K=1,000:** ~3M SDK → ~9M. **Still fits in a query gas budget but no pagination at all.** See F5.

---

## 2. Failure Mode Enumeration

### 🟡 F1 — Registration fees are collected but have no withdraw path

**Location:** `execute_register` lines 135-148; absence of any sweep / withdraw handler.

**The flaw.** When `registration_fee_ujuno > 0`, the contract verifies that `info.funds` contains at least the required amount in `cfg.denom`. The funds are **NOT** routed anywhere — they accumulate in the contract's bank balance. There is no `WithdrawFees`, `SweepBalance`, admin-only or otherwise. Once collected, the fee is **trapped**.

**Why this matters.** If the contract is instantiated with `registration_fee_ujuno = 1_000_000` and 100 agents register, 100 JUNO sits in the contract address forever, unspendable. The admin cannot recover it. There is no proposal, no migration handler, no entrypoint.

**Reproducer.**
```
1. Instantiate with fee = 1_000_000 ujunox.
2. Register 5 agents, each sending 1_000_000 ujunox.
3. Query bank balance of contract → 5_000_000 ujunox.
4. Try every existing execute msg → none withdraw funds.
5. Funds are permanent.
```

**Severity: MEDIUM** — value-trap, not exploitable for theft, but renders the fee mechanism broken. A reasonable user would assume fees go to the admin or a treasury.

**Suggested fix.** Add `ExecuteMsg::WithdrawFees { recipient: String, amount: Option<Uint128> }`, admin-only. ~20 LoC.

```rust
ExecuteMsg::WithdrawFees { recipient, amount } => {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin { return Err(ContractError::Unauthorized {}); }
    let recipient = deps.api.addr_validate(&recipient)?;
    let bal = deps.querier.query_balance(env.contract.address, &cfg.denom)?;
    let amt = amount.unwrap_or(bal.amount);
    if amt > bal.amount { return Err(...); }
    Ok(Response::new().add_message(BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![Coin { denom: cfg.denom, amount: amt }],
    }))
}
```

**Regression test.** A new test in `tests.rs`:
```
fn registration_fees_are_withdrawable_by_admin() {
    // instantiate with fee > 0
    // register 3 agents
    // assert bank balance of contract = 3 × fee
    // call WithdrawFees as admin
    // assert bank balance moved to recipient
    // call WithdrawFees as non-admin → Unauthorized
}
```

---

### 🟡 F2 — No uniqueness enforcement on `name` or `capabilities_hash`

**Location:** `execute_register` line 117-185. Neither `name` nor `capabilities_hash` is checked against existing agents.

**The flaw.** Two agents can register with `name = "DataAgent"` and the same `capabilities_hash`. Downstream contracts (task-ledger, agent-company) that resolve agents by name or by capabilities will see ambiguity. A malicious actor can register a name that mimics an existing trusted agent, then send tasks to the impostor.

**Why this matters concretely.** The JunoClaw architecture uses `capabilities_hash` as a content-address for the agent's published capability set. If two agents claim the same capabilities_hash, callers can't distinguish them by capability alone — they have to also verify the `owner` field, which requires extra logic that downstream contracts may skip.

**Severity: LOW-MEDIUM** — depends on whether downstream code resolves by name/hash. Currently no contract in the JunoClaw stack does (everything routes by `agent_id`), so the immediate risk is reputational / UX-confusion only. But the moment a routing system or agent marketplace queries by name, this becomes exploitable.

**Suggested fix.** Add a `NAMES_TAKEN: Map<&str, u64>` reverse index. On register, `if NAMES_TAKEN.has(deps.storage, &name) { Err(NameTaken) }` else `NAMES_TAKEN.save(deps.storage, &name, &agent_id)?`. Same for `capabilities_hash` if we want capability-uniqueness. ~15 LoC.

Alternative (less restrictive): document that `name` is **not** unique and that callers must always verify `(agent_id, owner)` together. Add this to the `AgentProfile` doc-comment in `junoclaw-common/src/lib.rs`.

---

### 🟢 F3 — Excess fee and non-`cfg.denom` funds silently absorbed

**Location:** `execute_register` lines 135-148.

**The flaw.** Same class as `agent-company` F3. If a user sends `1_500_000 ujunox` when the fee is `1_000_000`, the extra `500_000` is silently absorbed (compounds with F1 — non-withdrawable). If a user sends `1_000_000 ujunox + 100 uosmo` (e.g. IBC mix-up), the `100 uosmo` is also absorbed.

**Severity: LOW** — small per-user blast radius, but compounds with F1 to make the value-trap larger.

**Suggested fix (paired with F1).** Either:

1. **Reject mixed funds.** `if info.funds.len() > 1 || info.funds[0].denom != cfg.denom { Err(...) }`.
2. **Refund overage.** Send `BankMsg::Send` for `(sent - required)` back to the registrant. Cleaner UX, +5 LoC.

Option 2 is the minimal-diff fix.

---

### 🟢 F4 — No agent reactivation path

**Location:** `execute_deactivate` lines 223-252; absence of `ReactivateAgent`.

**Behaviour.** Once `is_active = false`, the agent is permanently dead-listed. To resume, the owner must `RegisterAgent` again, paying the fee and receiving a **new agent_id**. All historical task-ledger correlations to the old agent_id are orphaned.

**Severity: LOW** — design choice, not a bug. But:

- It compounds with F1 (the second registration fee is also trapped).
- It makes admin-deactivation (line 232) effectively a soft-permaban with no appeal mechanism.
- Common-case ops mistakes (admin deactivates wrong agent) require a state migration to fix.

**Suggested fix.** Add `ExecuteMsg::ReactivateAgent { agent_id }`, owner-or-admin, only valid if `!profile.is_active`. ~15 LoC + 1 test.

---

### 🟢 F5 — `GetAgentsByOwner` has no pagination

**Location:** `query` lines 385-395.

**The concern.** The handler loads `ids: Vec<u64>` from `AGENT_BY_OWNER`, then iterates and calls `AGENTS.may_load` for each. At K=1,000 agents owned by one address, that's 1,001 db_reads in a single query. Query gas budgets on Juno are generous (multi-million) so this *runs*, but it's a sharp cliff on the read path and adds 3K SDK gas per agent linearly.

**Severity: LOW** — a single DAO with 1,000 agents per owner is unusual; the gas envelope is fine through ~100 agents.

**Suggested fix.** Add `start_after: Option<u64>` and `limit: Option<u32>` to `GetAgentsByOwner`, capped at 50 like `ListAgents` already is. ~10 LoC + 1 test.

---

### 🟢 F6 — `AGENT_BY_OWNER` per-owner Vec is unbounded

**Location:** `state.rs` line 24, `execute_register` lines 168-172.

**The flaw.** Each `RegisterAgent` does `AGENT_BY_OWNER.update(deps.storage, &owner, |existing| Ok(existing.unwrap_or_default().push(id)))`. The Vec is **read in full, mutated, and written back in full** every time. At K agents owned, that's an O(K) read and O(K) write per registration.

**Gas growth.** At K=100 agents per owner, registration cost grows by ~5K SDK gas (~15K with multiplier). Linear in K. At K=10,000, registration costs an extra ~500K SDK = ~1.5M with multiplier — still fits but noticeable.

**Severity: LOW** — gas naturally throttles, no exploit, but the data structure choice is suboptimal for heavy owners.

**Suggested fix (refactor, not a bug fix).** Replace `Map<&Addr, Vec<u64>>` with `Map<(&Addr, u64), ()>` indexed by `(owner, agent_id)`. Append cost becomes O(1). Iteration cost becomes the same as before (K db_reads). Migration: write a one-shot migrate handler that converts old to new on next deploy.

This is a v1-of-this-contract refactor, not a v0 fix.

---

### 🟢 F7 — `max_agents` is a lifetime cap, not an active cap

**Location:** `execute_register` lines 129-133.

**Behaviour.** `if stats.total_registered >= config.max_agents`. Once `max_agents` is hit, no new registrations even if half the existing agents are deactivated.

**Severity: LOW** — design choice, but worth flagging because it surprises users. If `max_agents = 100` and 50 agents are registered + 50 deactivated, the next registration fails with `AgentLimitReached`.

**Suggested fix (if undesired).** Change check to `if stats.total_active >= config.max_agents`. One-character change. Document either way.

---

### 🟢 F8 — `slash_agent` is admin-only and decrements by hardcoded 5

**Location:** `execute_slash_agent` lines 288-311.

**Observation.** Slash is **only** the contract admin. Task-ledger cannot slash even though it is the only entity that knows when an agent fails a task. Result: trust_score only ever decreases via manual admin action.

The decrement is a hardcoded `5`. No graduated penalty. No correlation to severity.

**Severity: NONE** — these are design choices, not bugs. But they limit the reputation system's expressiveness: a single admin must manually adjudicate every slash, and every slash has the same cost.

**Suggested follow-up (not a fix).** v1: add `task_ledger` to the slash auth set; parametrize the decrement amount in `Config` or per-call.

---

### 🟢 F9 — `IncrementTasks` only ever increments trust_score on success

**Location:** `execute_increment_tasks` lines 270-280.

**Behaviour.**
```
profile.total_tasks += 1;
if success { profile.successful_tasks += 1; profile.trust_score += 1; }
```

Failed tasks bump `total_tasks` but **do not** decrement `trust_score`. So a malicious agent that fails 1,000 tasks accrues `total_tasks = 1000, trust_score = 0`. The 0/1000 success ratio is visible only to readers who compare both counters.

**Severity: NONE** — by design (decrement on failure could be punitive when the failure was the submitter's fault, not the agent's). But callers using `trust_score` alone as the reputation signal will be misled. Document this in the `AgentProfile` doc-comment.

---

## 3. Storage Layout Analysis

### Scale projections

| Scenario | Storage impact |
|---|---|
| 1,000 agents, 10 owners | AGENTS: ~250 KB. AGENT_BY_OWNER: ~10 entries × ~800 bytes (100 ids each). Trivial. |
| 100,000 agents, 1,000 owners | AGENTS: ~25 MB. AGENT_BY_OWNER: ~1,000 entries × ~800 bytes. Manageable. |
| 1M agents, 10 mega-owners (100K each) | AGENTS: ~250 MB. AGENT_BY_OWNER: 10 entries × **80 KB Vec each** — F6 bites here, register cost ~5M SDK per call. Pruning warranted. |

**Pruning.** None implemented. Deactivated agents stay in storage forever (correct: the audit trail requires it). At realistic scale (1K–10K agents per chain), this is fine.

---

## 4. Determinism Proof

| Concern | Status |
|---|---|
| No floats | ✅ |
| No HashMap iteration | ✅ — only BTreeMap-backed `Map<>` from cw-storage-plus |
| No `std::time` | ✅ — uses `env.block.time` |
| Deterministic queries | ✅ — `Order::Ascending` over storage keys |
| Serde stability | ✅ — `cw_serde` + `#[serde(default)]` discipline already followed |

**All clear.** ✅

---

## 5. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | **MEDIUM** | Add `WithdrawFees` admin entrypoint | ~20 LoC + 2 tests |
| F2 | LOW-MEDIUM | Add `NAMES_TAKEN` reverse index OR document non-uniqueness in `AgentProfile` | ~15 LoC + 1 test, or docs only |
| F3 | LOW | Refund overage / reject mixed funds | ~10 LoC + 1 test |
| F4 | LOW | Add `ReactivateAgent` | ~15 LoC + 1 test |
| F5 | LOW | Pagination on `GetAgentsByOwner` | ~10 LoC + 1 test |
| F6 | LOW | Refactor `AGENT_BY_OWNER` to composite-key `Map<(&Addr, u64), ()>` | ~30 LoC + migration + 1 test |
| F7 | LOW | Document or change `max_agents` semantics | docs or 1-char change |
| F8 | NONE | Document admin-only slash; consider task-ledger auth in v1 | docs only |
| F9 | NONE | Document trust_score asymmetry in `AgentProfile` doc-comment | docs only |

**Recommendation.** Land **F1 + F3** in a v0.2 bump (the value-trap and the silent-absorption fix go together). **F2 + F5** are next; consider for v0.3. **F6** is a v1 refactor with migration. F4/F7/F8/F9 are quality-of-life items.

---

## 6. Comparative summary across the JunoClaw stack

| Contract | Audit status | Headline finding | Severity |
|---|---|---|---|
| `agent-company` | DONE (`a22e496`) | Vote weights not snapshotted at proposal creation | **HIGH** |
| `agent-registry` | DONE (this doc, `a22e496`) | Registration fees trapped (no withdraw path) | **MEDIUM** |
| `task-ledger` | next | TBD | TBD |
| `escrow` | pending | TBD | TBD |
| `zk-verifier` | pending (already in production on uni-7) | TBD | TBD |
| `moultbook-v0` | written deterministically from day 0 | None | None |

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark. This audit will be re-anchored as a Moultbook entry once Moultbook v0 is on devnet, citing `agent-registry` commit `a22e496` as the audit subject and `MOULTBOOK_DEV_COLLABORATION_NOTES.md` §3 as the methodology source.*
