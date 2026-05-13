# Escrow (Payment-Ledger) — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark. Anchor commit: `26a43f7` on origin/main. Files read in full: `src/contract.rs` (432 lines), `src/state.rs` (33 lines), `src/error.rs` (30 lines), `src/msg.rs` (82 lines).*

---

## 0. Architectural surprise (read this first)

**This contract is non-custodial.** It does **not hold funds**. Every prior reasoning in the audit-sweep that assumed escrow as a fund-holder is wrong. Escrow is a **payment-ledger journal**: it records obligations between payer and payee, tracks status transitions (Pending → Confirmed/Disputed/Cancelled/Verified), and exposes status reads to other contracts (notably as the `EscrowObligationConfirmed` constraint in `junoclaw-common::Constraint`).

Funds flow off-contract. The payer Authorizes, the payer (or task-ledger on the payer's behalf) Confirms, and **the contract never sees the money**. This is a fundamentally different trust model from custodial escrow, with different failure modes.

The `crates.io` name confirms it: `crates.io:junoclaw-payment-ledger`. The package is named `escrow` for historical reasons; the contract's purpose is a payment-obligation journal.

This architectural choice is **explicit and correct** for the JunoClaw threat model: keeps the contract small, removes the funds-management attack surface, lets WAVS attestations carry the off-chain truth claim. But it raises a different class of concerns about journal integrity (covered below).

---

## 1. Surface summary

| Area | Lines | Functions |
|------|-------|-----------|
| Entry points | 18-122, 395-431 | `instantiate`, `execute`, `query`, `migrate` |
| Lifecycle | 124-172, 174-215, 217-253, 255-290 | `execute_authorize`, `execute_confirm`, `execute_dispute`, `execute_cancel` |
| Attestation | 292-321 | `execute_attach_attestation` |
| Admin / config | 323-393 | `execute_update_config`, `execute_update_registry` |
| Read-side | 395-421 | 5 query branches incl. `GetObligationByTask` reverse lookup |

**State surface:**

| Storage item | Key | Value | Notes |
|---|---|---|---|
| `CONFIG` | singleton | `Config` (~200 bytes) | admin + task_ledger + denom + registry |
| `OBLIGATIONS` | u64 (obligation_id) | `PaymentObligation` (~250 bytes) |  |
| `OBLIGATIONS_BY_TASK` | u64 (task_id) | u64 (obligation_id) | **1:1 reverse index — NOT `Vec<u64>`** ✅ |
| `NEXT_OBLIGATION_ID` | singleton | u64 | starts at 1 |
| `LEDGER_STATS` | singleton | `LedgerStats` (~80 bytes) |  |

**No prefix collisions.** ✅ (`config`, `obligations`, `obligations_by_task`, `next_obligation_id`, `ledger_stats` all distinct.)

**The Vec-index bug-class prediction does not apply here.** `OBLIGATIONS_BY_TASK` is a one-task-to-one-obligation mapping (not a Vec) because each task can only have one obligation. The 1:1 constraint is enforced at line 133 (`if OBLIGATIONS_BY_TASK.has(deps.storage, task_id) { Err(AlreadyAuthorized) }`).

---

## 2. Failure Mode Enumeration

### 🟡 F1 — `Confirm` is trust-only; no proof the payer actually paid; `tx_hash` silently dropped

**Location:** `execute_confirm` lines 174-215. Note `_tx_hash: Option<String>` (line 180) — the underscore prefix is the smoking gun: the developer knew the parameter was unused.

**The flaw.** The Confirm handler:
1. Loads the obligation by task_id.
2. Checks `info.sender ∈ {payer, admin, task_ledger}`.
3. Checks status is Pending.
4. Sets status to Confirmed.
5. **Never validates that funds actually moved.** The `tx_hash` parameter is dropped on the floor.

**Result.** A payer can self-Confirm without ever sending funds. The payee gets nothing on-chain (and possibly nothing off-chain either). The ledger reports the obligation as Confirmed.

**Concrete attack on the public-task path.** The default task-ledger flow:

1. Bob authorizes obligation: 100 JUNO to Alice for task X.
2. An operator marks task X Completed.
3. Task-ledger fires `escrow.Confirm` as a sub-message (atomic with completion).
4. **Bob never sent 100 JUNO. Alice has no on-chain recourse — Confirm is final, no dispute path post-Confirm.**

In the *governance* flow (where agent-company is the payer), the DAO is the trust-anchor and the assumption is that DAO-funded confirmations correspond to real treasury outflows. That's defensible.

In the *operator-submitted* flow, the trust model is murkier. The submitter (Bob) could grief Alice by authorizing then never paying.

**Severity: LOW-MEDIUM.** The non-custodial design means this is **inherent to the contract's choice**, not a bug. But the claim of "Confirmed" without on-chain proof of payment is a strong claim that the API doesn't currently substantiate.

**Suggested fix.** Two options, layered:

1. **Store the `tx_hash` in the obligation.** Add `tx_hash: Option<String>` to `PaymentObligation` in `junoclaw-common/src/lib.rs:85-97`. On Confirm, save it. This makes the ledger queryable for proof-attempts even if the chain doesn't validate them.

2. **Couple Confirm to `AttachAttestation`.** Either:
   - **Strict:** `Confirm` requires either `obligation.attestation_hash.is_some()` OR `info.sender == config.task_ledger` (the trusted automation path). Public payers must attach an attestation before confirming.
   - **Lenient:** `Confirm` accepts both `Pending` and `Verified` (post-attestation) states. (See F5 — this is also needed to fix the broken state machine.)

Combined effort: ~25 LoC + 2 tests.

---

### 🟡 F5 — `AttachAttestation` breaks the state machine: Pending→Verified blocks subsequent Confirm

**Location:** `execute_attach_attestation` lines 292-321 (especially line 311-313); `execute_confirm` line 196.

**The flaw.** `AttachAttestation` does:
```rust
if obligation.status == ObligationStatus::Pending {
    obligation.status = ObligationStatus::Verified;
}
```

But `Confirm` requires status == Pending:
```rust
if obligation.status != ObligationStatus::Pending {
    return Err(ContractError::NotPending { obligation_id });
}
```

**Result.** The flow `Authorize → AttachAttestation → Confirm` **fails at step 3** with `NotPending`. The intended-purpose flow (WAVS attests payment, then payer confirms) is broken.

**Why it doesn't trigger today.** Looking at the JunoClaw stack:
- `task-ledger.execute_complete` calls `escrow.Confirm` without ever calling `AttachAttestation`. So the normal completion path stays in Pending → Confirmed, never touching Verified.
- `AttachAttestation` is only callable by `admin` or `task_ledger` (line 300). No automation invokes it today.
- WAVS attestation pipeline isn't fully wired into the cw layer yet.

So this is a **latent bug** that activates the moment WAVS attestations get wired up.

**Severity: LOW-MEDIUM.** Doesn't bite today; will bite the next time someone enables the WAVS attestation pipeline in the cw flow.

**Suggested fix.** Two clean options:

1. **Don't change status on AttachAttestation.** Just save the attestation hash; let `Confirm` flip status to Confirmed regardless. Drop the `Verified` variant entirely (it's currently the only writer, no readers).

2. **Make `Confirm` accept both `Pending` and `Verified`.** One-line change at line 196:
   ```rust
   if !matches!(obligation.status, ObligationStatus::Pending | ObligationStatus::Verified) {
       return Err(ContractError::NotPending { obligation_id });
   }
   ```

Option 1 is cleaner; option 2 preserves the `Verified` status enum if downstream consumers care about distinguishing attested vs non-attested confirmations.

**Regression test (after fix).**
```
fn confirm_succeeds_after_attestation() {
    // authorize, attach_attestation, confirm — all should succeed
}
```

---

### 🟡 F8 — `timeout_blocks` field is dead; type-unit mismatch with `created_at`

**Location:** `state.rs:11` (`timeout_blocks: u64`); `contract.rs:152` (`created_at: env.block.time.seconds()`); no read path for `timeout_blocks` exists in the contract.

**The flaw.** The contract advertises a timeout mechanism via `Config.timeout_blocks` and `InstantiateMsg.timeout_blocks`. **Nothing reads this field.** No entrypoint expires obligations. No query reports remaining time. It's a dead field in the API.

**Type mismatch.** Even if we add the timeout enforcement, the storage layout has a unit-mismatch bug:
- `obligation.created_at` is in **Unix seconds** (line 152: `env.block.time.seconds()`).
- `config.timeout_blocks` is the count of **blocks**.

These can't be subtracted/compared without a unit conversion (and the conversion is non-deterministic at chain level: block intervals vary).

**Why it matters concretely.** Combined with task-ledger F1 (CancelTask leaves orphaned escrow obligations) and the absence of any expire-on-timeout mechanism, **obligations can sit in Pending forever**. The DAO has no automated cleanup; the API field that promised cleanup is silently dead.

**Severity: MEDIUM** — feature gap that's also silently incoherent (the field is named in blocks but stored in seconds).

**Suggested fix.** Two-part:

1. **Resolve the unit mismatch.** Rename `timeout_blocks` → `timeout_seconds` in `Config` and `InstantiateMsg`. Migration: store the renamed field, treat existing values as seconds. (If users supplied "blocks" expecting block-based, they get a different timeout — flag in MIGRATION_NOTES.) ~10 LoC.

2. **Add `ExecuteMsg::ExpirePending { task_id }`.** Permission-less; any caller can trigger if `env.block.time.seconds() > obligation.created_at + config.timeout_seconds`. Transitions Pending → Cancelled, decrements `total_pending`, increments `total_cancelled`. ~25 LoC + 2 tests.

**Regression tests.**
```
fn expire_pending_succeeds_past_timeout() { ... }
fn expire_pending_fails_before_timeout() { ... }
fn expire_pending_fails_for_non_pending() { ... }
```

---

### 🟢 F10 — `Authorize` is open to anyone; task_id squatting possible

**Location:** `execute_authorize` lines 124-172. No sender authorization check.

**The flaw.** Anyone can call `Authorize` with any `task_id` and any `payee`, binding themselves as the payer. The handler enforces:
- `OBLIGATIONS_BY_TASK.has(task_id)` ⇒ `AlreadyAuthorized` (1:1 mapping)
- `amount.is_zero()` ⇒ `ZeroAmount`

But there's no check that the `task_id` is real, that the caller is allowed to authorize, or that the payee/payer relationship makes sense.

**The squatting attack.** An attacker pre-claims task_ids 1, 2, 3, ... 100 with bogus 1-ujuno obligations targeting themselves. Now real tasks #1-#100 cannot have escrow obligations created — `task-ledger.execute_complete` calls `escrow.Confirm` looking up by task_id and finds the squatter's obligation, marking it as Confirmed (admin or task_ledger can confirm). Confused state.

**Mitigations already in place.**
- The squatter pays gas + 1 ujuno per squat. Cost is non-zero.
- Admin can call `UpdateConfig` to swap to a fresh escrow contract address, but that's a heavyweight migration.
- Task-ledger's `proposal_id`-based path (governance tasks) uses `proposal_id` as the escrow key, sidestepping local task_id collisions.

**Severity: LOW** — annoyance with cost-to-attacker, not theft. But it muddies the ledger's correctness story.

**Suggested fix.** Restrict `Authorize` to:
- `admin`, OR
- `config.task_ledger`, OR
- `config.registry.agent_registry` (allows the agent-company / governance path), OR
- explicit allowlist (a new `authorizers: Vec<Addr>` in Config, defaulted to empty).

~10 LoC + 1 test.

---

### 🟢 F2 — `tx_hash` is in the message schema but never persisted

**Location:** `execute_confirm` lines 174-215; `PaymentObligation` in `junoclaw-common/src/lib.rs:85-97`.

**The flaw.** `ExecuteMsg::Confirm { task_id, tx_hash }` accepts a `tx_hash: Option<String>` parameter, but the handler signature uses `_tx_hash` (underscore prefix → unused). The `PaymentObligation` struct has no `tx_hash` field. Callers passing tx_hash receive no error and no acknowledgement; the field is silently dropped.

**Severity: LOW** — caller-confusion. No exploit. But it's dishonest API surface.

**Suggested fix.** Add `tx_hash: Option<String>` to `PaymentObligation` and persist it in Confirm. ~5 LoC + 1 schema-regen.

This pairs with F1 (the proof-of-payment fix should write the tx_hash for queryability).

---

### 🟢 F11 — `LedgerStats` mixes current-state counters with lifetime cumulatives

**Location:** `state.rs:17-23`; `execute_authorize/confirm/dispute/cancel` updates.

**The inconsistency.**
- `total_pending` — current state. Increments on Authorize, decrements on Confirm/Dispute/Cancel.
- `total_confirmed`, `total_disputed`, `total_cancelled` — lifetime cumulative. Only ever increment.
- `total_obligations` — lifetime cumulative count.

The naming `total_*` is consistent in form but the semantics differ. A reader querying `GetStats` might assume `total_confirmed` is "value of all currently-Confirmed obligations" (parallel to `total_pending`). It's actually "lifetime sum of confirmed amounts."

**Severity: LOW** — cosmetic / docs. No exploit.

**Suggested fix.** Rename:
- `total_pending` → `current_pending`
- `total_confirmed` → `lifetime_confirmed`
- etc.

OR (simpler) add docstrings to the struct fields explaining each. ~5 LoC.

---

### 🟢 F12 — No path to undisputed a Disputed obligation

**Location:** `execute_dispute` lines 217-253; absence of any handler that transitions out of Disputed.

**The flaw.** Once an obligation is Disputed, it's permanently Disputed. No admin override, no resolution mechanism, no transition to Cancelled or Confirmed.

**Severity: LOW** — design-incomplete. In a real dispute, the off-chain process would either:
- Resolve in payer's favor (payee accepts) → obligation should transition to Cancelled.
- Resolve in payee's favor (payer accepts) → obligation should transition to Confirmed.
- Time out → ?

The contract has no on-chain handler for either resolution. Disputes become permanent ledger entries with no closure.

**Suggested fix.** Add `ExecuteMsg::ResolveDispute { task_id, resolution: DisputeResolution }` where `DisputeResolution` is `enum { ConfirmObligation, CancelObligation }`. Admin-only or mutual (payer + payee both must call). ~25 LoC + 2 tests.

---

### 🟢 F13 — `total_pending` uses `saturating_sub` (masks accounting drift)

**Location:** `execute_confirm` line 206; `execute_dispute` line 244; `execute_cancel` line 282.

**Behaviour.** `s.total_pending = s.total_pending.saturating_sub(obligation.amount)`. If for any reason `total_pending < obligation.amount`, the subtraction silently saturates to zero rather than panicking.

**Why it's defensive but masks bugs.** If accounting drift ever occurs (it shouldn't given the increment/decrement is symmetric on Authorize/Confirm/Dispute/Cancel), the saturation hides it. A `checked_sub` that errors would surface the drift loudly.

**Severity: LOW** — robustness vs visibility tradeoff. The current choice is more robust under stress (a bug doesn't brick the contract) but less visible to operators.

**Suggested fix (optional).** Switch to `checked_sub` and return a contract error on underflow. Surface accounting bugs at write-time rather than letting them silently pile up. ~3 LoC.

---

### 🟢 F14 — `Confirm` and `Dispute` cannot be reordered; only Pending → Confirmed/Disputed/Cancelled forward arrows

**Location:** All status transitions check `obligation.status != ObligationStatus::Pending` before transitioning.

**Behaviour.** State machine is strictly forward:

```
Pending → Confirmed
Pending → Disputed
Pending → Cancelled
Pending → Verified (via AttachAttestation; broken — F5)
```

No state has a successor (except F5's broken Verified). All terminal states are absorbing.

**Severity: NONE** — by design. But worth flagging because:
- F12 wants a way out of Disputed.
- F1 wants AttachAttestation to extend Confirmed (proof-after-the-fact).

The strictly-forward model is correct for an immutable journal but limits the kinds of corrections a real-world dispute resolution would need. Document this as the explicit choice.

---

## 3. Cross-cutting predicition update

The task-ledger audit (`@/c:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/contracts/task-ledger/DETERMINISTIC_AUDIT.md` §6) predicted a unified Vec-index migration PR covering agent-registry F6, task-ledger F3, and "the same pattern is likely in escrow." **The escrow prediction was wrong** — escrow uses 1:1 maps and doesn't have the Vec-index issue.

The unified migration PR therefore covers only:
- agent-registry F6 (`AGENT_BY_OWNER: Map<&Addr, Vec<u64>>` → `Map<(&Addr, u64), ()>`)
- task-ledger F3 (`TASKS_BY_AGENT: Map<u64, Vec<u64>>` → `Map<(u64, u64), ()>`, same for `TASKS_BY_SUBMITTER`)

Escrow's `OBLIGATIONS_BY_TASK: Map<u64, u64>` is fine as-is. ✅

---

## 4. Gas Trace — hot paths

### 4.1 `execute_authorize`

| Step | Gas est. |
|---|---|
| `OBLIGATIONS_BY_TASK.has` | ~3,000 |
| Zero-amount check | ~50 |
| `CONFIG.load` | ~3,500 |
| `addr_validate(&payee)` | ~1,000 |
| `NEXT_OBLIGATION_ID.load` | ~2,000 |
| `OBLIGATIONS.save` | ~5,000 |
| `OBLIGATIONS_BY_TASK.save` | ~3,000 |
| `NEXT_OBLIGATION_ID.save` | ~2,000 |
| `LEDGER_STATS.update` | ~5,000 |

**Total:** ~24K SDK → ~72K with multiplier. Cheap. ✅

### 4.2 `execute_confirm`

| Step | Gas est. |
|---|---|
| `OBLIGATIONS_BY_TASK.may_load` | ~3,000 |
| `OBLIGATIONS.load` | ~3,500 |
| `CONFIG.load` | ~3,500 |
| Auth check (3-way comparison) | ~150 |
| Status check | ~50 |
| `OBLIGATIONS.save` | ~5,000 |
| `LEDGER_STATS.update` | ~5,000 |

**Total:** ~20K SDK → ~60K with multiplier. Cheap. ✅

(All other handlers are similar in shape.)

---

## 5. Determinism Proof

| Concern | Status |
|---|---|
| No floats | ✅ |
| No HashMap iteration | ✅ — only `Map<>` from cw-storage-plus |
| No `std::time` | ✅ — uses `env.block.time.seconds()` |
| Deterministic queries | ✅ — `Order::Ascending` |
| Serde stability | ✅ — `cw_serde`; `EscrowDeposit` / `EscrowStatus` aliases preserved for migration |

**All clear.** ✅

---

## 6. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | LOW-MEDIUM | Persist `tx_hash` in obligation; document the trust-only Confirm OR couple to attestation | ~25 LoC + 2 tests |
| F5 | LOW-MEDIUM | Don't change status on AttachAttestation OR allow Confirm from Verified state | 1-line + 1 test |
| F8 | **MEDIUM** | Rename `timeout_blocks` → `timeout_seconds` + add `ExpirePending` handler | ~35 LoC + 3 tests |
| F10 | LOW | Restrict `Authorize` to admin / task-ledger / registry | ~10 LoC + 1 test |
| F2 | LOW | Add `tx_hash` field to `PaymentObligation`, persist on Confirm | ~5 LoC |
| F11 | LOW | Rename `total_*` to `current_*` / `lifetime_*` OR add docstrings | ~5 LoC |
| F12 | LOW | Add `ResolveDispute` handler | ~25 LoC + 2 tests |
| F13 | LOW | Switch `saturating_sub` → `checked_sub` to surface accounting drift | ~3 LoC |
| F14 | NONE | Document the strictly-forward state machine | docs only |

**Recommendation.**

- **Sprint 1 (escrow-v0.2):** F8 (the most consequential — dead feature with type bug) + F5 (latent state-machine bug) + F2 (tx_hash persistence). All three are small and unblock the WAVS attestation flow.
- **Sprint 2 (escrow-v0.3):** F1 (proof-of-payment couplings — design conversation needed) + F10 (Authorize restriction) + F12 (dispute resolution path).
- **Sprint 3 (cosmetics):** F11, F13, F14.

---

## 7. Comparative summary across the JunoClaw stack

| Contract | Audit status | Headline finding | Severity |
|---|---|---|---|
| `agent-company` | DONE (`a22e496`) | Vote weights not snapshotted at proposal creation | **HIGH** |
| `agent-registry` | DONE (`a22e496`) | Registration fees trapped (no withdraw path) | **MEDIUM** |
| `task-ledger` | DONE (`a22e496`) | CancelTask leaves orphaned escrow obligations | **LOW-MEDIUM** |
| `escrow` | DONE (`26a43f7`, this doc) | `timeout_blocks` field is dead + unit mismatch with `created_at` | **MEDIUM** |
| `zk-verifier` | pending (production on uni-7) | TBD | TBD |
| `moultbook-v0` | written deterministically from day 0 | None | None |
| `junoswap-pair` / `junoswap-factory` | pending | TBD | TBD |
| `builder-grant` | pending | TBD | TBD |

**Headline cross-cutting observations:**

1. **Vec-index bug class** affects 2 of 4 audited contracts (agent-registry, task-ledger). Escrow is clean.
2. **State-machine completeness** is the recurring theme: every contract has at least one "no path out of state X" finding (agent-registry F4 deactivation, task-ledger F1 cancel-with-orphan, escrow F12 dispute-resolution).
3. **Dead config fields** appear in escrow (F8 `timeout_blocks`). Worth a sweep across the other contracts to check for similar dead fields.
4. **The non-custodial choice in escrow is the most surprising and consequential design decision in the stack**, and arguably correct under the WAVS-attests-truth model — but the contract's API doesn't yet substantiate the trust claim it implies.

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark. This audit will be re-anchored as a Moultbook entry once Moultbook v0 is on devnet, citing `escrow` commit `26a43f7` as the audit subject.*
