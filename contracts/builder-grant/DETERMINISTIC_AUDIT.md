# Deterministic Audit — `builder-grant`

*Apache-2.0. Methodology: [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md). Sister audits: [`junoswap-factory/DETERMINISTIC_AUDIT.md`](../junoswap-factory/DETERMINISTIC_AUDIT.md), [`agent-registry/DETERMINISTIC_AUDIT.md`](../agent-registry/DETERMINISTIC_AUDIT.md), [`zk-verifier/DETERMINISTIC_AUDIT.md`](../zk-verifier/DETERMINISTIC_AUDIT.md). Anchor: contract source at `contracts/builder-grant/src/` as of 2026-05-14.*

## Summary (3 lines)

`builder-grant` is the TEE-attestation-gated treasury distributor — builders submit proof-of-work, operators verify in-enclave, builders claim native tokens. Authority and state-integrity surface is well-designed (work-hash uniqueness enforced, status state-machine clean, operator/admin separation explicit). Two MEDIUM findings worth pre-mainnet remediation: F1 (`Fund {}` silently absorbs non-configured-denom tokens into the contract balance with no recovery path); F2 (`Withdraw` and `Verify` interact in a way that can strand verified-but-unclaimed grants if the admin withdraws between verification and claim).

## Methodology

Same 4-axis deterministic-scrutiny benchmark (authority surface · state integrity · failure determinism · resource bounds).

## Findings

| ID | Severity | Title | Affected |
|----|----------|-------|----------|
| F1 | **MEDIUM** | `Fund {}` accepts and absorbs tokens of any denom; non-configured denoms become unrecoverable | `contract.rs:273-286` |
| F2 | **MEDIUM** | `Withdraw` and `VerifyWork` are not coupled; admin can strand verified-pending grants | `contract.rs:155-209, 365-393` |
| F3 | MEDIUM | `query_builder_stats` and `query_stats` scan the entire `SUBMISSIONS` map with no pagination | `contract.rs:456-505` |
| F4 | LOW | `Custom` tier accepts unbounded `amount`; gated only by operator trust; `total_granted += reward` is unchecked | `state.rs:14-26, contract.rs:254` |
| F5 | LOW | `RemoveOperator` doesn't invalidate in-flight verifications by that operator | `contract.rs:309-326` |
| F6 | LOW | `SetActive { active: false }` only gates `SubmitWork`; verify and claim continue on existing Pending entries | `contract.rs:101-103, 328-344` |
| F7 | LOW | No `migrate` entry_point | `contract.rs` |
| F8 | LOW | `query_list_submissions` correctly uses `Bound::exclusive` but post-filters by status — for skewed distributions iteration cost exceeds `limit` | `contract.rs:432-454` |

---

## F1 — `Fund {}` silently absorbs tokens of any denom (MEDIUM)

### Observation

`execute_fund` at `contract.rs:273-286`:

```rust
fn execute_fund(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let total_funded: u128 = info
        .funds
        .iter()
        .filter(|c| c.denom == config.denom)
        .map(|c| c.amount.u128())
        .sum();

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("funder", info.sender)
        .add_attribute("amount", total_funded.to_string()))
}
```

The `.filter(|c| c.denom == config.denom)` correctly accounts only the configured denom in the attribute. But `info.funds` is the actual coin transfer already moved to the contract address by wasmd at message-dispatch time, **before** `execute_fund` runs. Any non-`config.denom` tokens in `info.funds` are now in the contract's balance and the function ignores them.

### Failure modes

1. **Silent accounting drift.** User sends `Fund {}` with `1000 ujuno` when `config.denom = "ujunox"`. The transaction succeeds. `total_funded` attribute reports `0`. Off-chain indexers see "successful fund" and may treat it as 1000 ujuno funded (since they don't necessarily filter the attribute). Contract balance now has `1000 ujuno` that doesn't show in any query.
2. **Funds are unrecoverable.** `execute_withdraw` at `contract.rs:365-393` builds `BankMsg::Send` with `config.denom` only. The misdirected `ujuno` cannot be withdrawn through any contract path. Admin would need a separate `Withdraw_Any { denom }` to recover, which doesn't exist.
3. **Phishing-adjacent surface.** A user setting up an automated funder for `ujunox` who mistypes the denom (or whose downstream system supplies `ujuno` accidentally) loses the entire send permanently with a green checkmark on the tx.

### Severity rationale — MEDIUM

Not exploitable for value extraction (the contract gains funds; sender loses funds). Not normal-operation correctness either — most users will send the right denom. But the silent-absorb pattern is exactly the kind of footgun that compounds in production:

- A misconfigured frontend can drain user-side balances into the contract over many transactions before anyone notices.
- The lack of a recovery path turns a user input error into a permanent loss.
- The accounting attribute reads `0` when funds *were* received, which actively misleads anyone reading the chain log.

This is a one-line fix that should land before mainnet.

### Suggested fix

```rust
fn execute_fund(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    // Reject any non-configured-denom funds at the message level.
    // Wasmd has already moved them to the contract balance; refusing here
    // unwinds the bank transfer atomically with the contract execution.
    for coin in &info.funds {
        if coin.denom != config.denom {
            return Err(ContractError::UnexpectedDenom {
                expected: config.denom.clone(),
                received: coin.denom.clone(),
            });
        }
    }
    
    let total_funded: u128 = info
        .funds
        .iter()
        .map(|c| c.amount.u128())
        .sum();
    
    if total_funded == 0 {
        return Err(ContractError::EmptyFunds {});
    }

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("funder", info.sender)
        .add_attribute("amount", total_funded.to_string()))
}
```

Add `UnexpectedDenom { expected, received }` and `EmptyFunds {}` variants to `error.rs`. Now an early `return Err` from inside the contract triggers a full tx rollback, including the bank transfer — funds stay with sender.

### Test coverage gap

`tests.rs` does not currently test `Fund {}` with a wrong-denom send. Add:

```rust
#[test]
fn test_fund_wrong_denom_rejected() {
    let mut deps = mock_dependencies();
    setup_grant(&mut deps);
    let info = message_info(&Addr::unchecked("funder"), &coins(1000, "ujuno"));
    let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Fund {}).unwrap_err();
    assert!(matches!(err, ContractError::UnexpectedDenom { .. }));
}

#[test]
fn test_fund_empty_rejected() {
    let mut deps = mock_dependencies();
    setup_grant(&mut deps);
    let info = message_info(&Addr::unchecked("funder"), &[]);
    let err = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Fund {}).unwrap_err();
    assert!(matches!(err, ContractError::EmptyFunds {}));
}
```

---

## F2 — `Withdraw` and `VerifyWork` are not coupled; admin can strand verified-pending grants (MEDIUM)

### Observation

The grant lifecycle is `Pending → Verified → Claimed` (or `Rejected` terminal). State transitions:

- **Verify (operator/admin/agent_company):** `Pending → Verified` if `approved`, else `Rejected`. Does not check balance — verification can happen even with zero contract balance.
- **Claim (builder):** `Verified → Claimed`, fails with `InsufficientFunds` if `balance < reward` at claim time.
- **Withdraw (admin only):** Sends any amount up to the full balance. Does **not** reserve outstanding-verified-but-unclaimed obligations.

### Failure scenarios

**Scenario A (innocent timing).** Admin batches verifications throughout the day, then nightly sweeps the treasury for monthly accounting via `Withdraw { amount: None }` (all-balance). Any builder with `Verified` status who hasn't claimed yet will fail at `claim_grant` with `InsufficientFunds`. Their grant is stranded in `Verified` state — the next admin refund (`Fund {}`) re-enables the claim, but the builder has no on-chain visibility into when that happens.

**Scenario B (intentional grief).** Compromised admin verifies a builder's submission, then immediately withdraws the balance before the builder can claim. The builder is stuck with a `Verified` record they cannot redeem. The verified status is not retractable.

**Scenario C (operator/admin separation breakdown).** Operator verifies (legitimately), admin (separate key, compromised) withdraws. Same outcome as B but harder to attribute.

### Severity rationale — MEDIUM

This is a **trust-model finding**, not a code-correctness bug. The contract's design explicitly grants admin treasury authority. The intended trust model is "admin = treasury manager = governed by DAO eventually." For the operational phase (admin = single key), this is a documented centralization risk.

But: the failure mode is non-atomic and silent from the builder's perspective. A builder who verifies-then-claims in two separate transactions has a window of arbitrary length where the admin can rug-pull their grant. The contract emits no on-chain signal that a claim is pending; admin is on their honor.

Pre-mainnet, this should be mitigated either by:
- Tracking outstanding verified-but-unclaimed obligations and refusing `Withdraw` that would drop balance below the sum
- Documenting the trust-model gap explicitly in `LEGAL_CAVEATS.md` and in the contract's README

### Suggested fix

Track outstanding obligations in `Config` (or as a derived `Item`):

```rust
// In state.rs:
pub const VERIFIED_PENDING_TOTAL: Item<u128> = Item::new("verified_pending_total");

// In execute_verify_work (when approved):
if approved {
    submission.status = SubmissionStatus::Verified;
    let pending = VERIFIED_PENDING_TOTAL.may_load(deps.storage)?.unwrap_or(0);
    VERIFIED_PENDING_TOTAL.save(deps.storage, &(pending + submission.tier.reward_amount()))?;
}

// In execute_claim_grant (after status transition to Claimed):
let pending = VERIFIED_PENDING_TOTAL.may_load(deps.storage)?.unwrap_or(0);
VERIFIED_PENDING_TOTAL.save(deps.storage, &pending.saturating_sub(reward))?;

// In execute_withdraw:
let pending = VERIFIED_PENDING_TOTAL.may_load(deps.storage)?.unwrap_or(0);
let withdrawable = balance.amount.u128().saturating_sub(pending);
let withdraw_amount = amount.unwrap_or(withdrawable);
if withdraw_amount > withdrawable {
    return Err(ContractError::WouldStrandVerifiedGrants {
        outstanding: pending,
        requested: withdraw_amount,
        withdrawable,
    });
}
```

Adds ~25 LoC + 1 storage Item + 1 error variant. Builders' verified grants are now atomically protected from admin withdrawal until claimed.

### Alternative — document the gap

If the operational model is "admin = DAO and we trust DAO not to rug," document it explicitly in `LEGAL_CAVEATS.md` under §"Builder Grant trust assumptions" and in `builder-grant/README.md`. Not a substitute for the code-level fix but valid for the v0.1 phase.

---

## F3 — `query_builder_stats` and `query_stats` scan the entire `SUBMISSIONS` map (MEDIUM)

### Observation

**`query_builder_stats`** (`contract.rs:456-479`):

```rust
let submissions: Vec<WorkSubmission> = SUBMISSIONS
    .range(deps.storage, None, None, Order::Ascending)
    .filter_map(|item| {
        let (_, sub) = item.ok()?;
        if sub.builder == addr { Some(sub) } else { None }
    })
    .collect();
```

No limit, no pagination, no per-builder index. For a contract with N total submissions and a builder who submitted M of them, the storage cost is `O(N)` reads + `O(M)` allocations. The returned `Vec<WorkSubmission>` payload is unbounded.

**`query_stats`** (`contract.rs:481-504`):

```rust
let pending_count: u64 = SUBMISSIONS
    .range(deps.storage, None, None, Order::Ascending)
    .filter_map(|item| {
        let (_, sub) = item.ok()?;
        if sub.status == SubmissionStatus::Pending { Some(1u64) } else { None }
    })
    .sum();
```

Same `O(N)` scan, no maintained counter.

### Failure modes

1. **Gas-limit ceiling.** At ~1000 stored submissions, `query_builder_stats` for an active builder starts approaching CosmWasm query gas limits. By 10000 submissions it's unusable. CosmWasm sets query gas limits per-chain (typically 3-10M); a storage read is ~500-1000 gas.
2. **Response size.** `query_builder_stats` returns `Vec<WorkSubmission>` of arbitrary length. CosmWasm caps query response sizes (typically 64KB). Even at modest submission sizes (~500 bytes each), 128 submissions hits the cap and the query fails with a generic "response too large" — no helpful error for the caller.
3. **Indexer-side cost.** Off-chain indexers polling `query_stats` for pending counts re-do the full scan on every poll. Maintaining a `PENDING_COUNT` counter in storage would make this O(1).

### Severity rationale — MEDIUM

This is `agent-registry` F3's same pattern, re-flagged. It's a query-side issue, not a state-integrity bug. The contract grows fine; only the read side degrades. But "growing fine, reads degrade silently" is exactly the failure mode that surfaces in production after months of operation.

### Suggested fix

**For `query_builder_stats`:**

Maintain a per-builder submission index. In `execute_submit_work`:

```rust
// state.rs:
pub const BUILDER_SUBMISSION_IDS: Map<(&Addr, u64), ()> = Map::new("builder_submission_ids");

// In execute_submit_work, after SUBMISSIONS.save:
BUILDER_SUBMISSION_IDS.save(deps.storage, (&info.sender, seq), &())?;

// In query_builder_stats:
let submission_ids: Vec<u64> = BUILDER_SUBMISSION_IDS
    .prefix(&addr)
    .keys(deps.storage, start_bound, None, Order::Ascending)
    .take(limit)
    .filter_map(|k| k.ok())
    .collect();
let submissions: Vec<WorkSubmission> = submission_ids
    .iter()
    .filter_map(|id| SUBMISSIONS.load(deps.storage, *id).ok())
    .collect();
```

Plus paginate the query: add `start_after: Option<u64>, limit: Option<u32>` parameters to `GetBuilderStats`.

**For `query_stats.pending_count`:**

Maintain a `PENDING_COUNT` counter as an `Item<u64>`:

```rust
// In state.rs:
pub const PENDING_COUNT: Item<u64> = Item::new("pending_count");

// In execute_submit_work, before returning:
let pc = PENDING_COUNT.may_load(deps.storage)?.unwrap_or(0);
PENDING_COUNT.save(deps.storage, &(pc + 1))?;

// In execute_verify_work, after status transition (Pending → Verified or Rejected):
let pc = PENDING_COUNT.may_load(deps.storage)?.unwrap_or(0);
PENDING_COUNT.save(deps.storage, &pc.saturating_sub(1))?;
```

`query_stats` then reads `PENDING_COUNT` directly — O(1).

---

## F4 — `Custom` tier accepts unbounded amount; `total_granted += reward` unchecked (LOW)

### Observation

`GrantTier::Custom { amount: u128, description: String }` (`state.rs:15`) has no cap on `amount`. A builder can submit:

```rust
ExecuteMsg::SubmitWork {
    tier: GrantTier::Custom { amount: u128::MAX, description: "lol".into() },
    evidence: "tx-hash".into(),
    work_hash: "<64-hex>".into(),
}
```

If a careless operator approves it, `execute_claim_grant` runs `submission.tier.reward_amount()` which returns `u128::MAX`. Two failure paths:

1. **Balance check fires:** `if balance.amount.u128() < reward` returns `InsufficientFunds`. Safe.
2. **`config.total_granted += reward` overflows.** `total_granted: u128` plus `u128::MAX` overflows. In Rust debug builds this panics; release builds wrap. Wasmd compiles contracts in release mode, so wrap. After overflow, `config.total_granted` shows a garbage value.

### Severity rationale — LOW

- Gated by operator verification — requires operator collusion or negligence.
- The balance check protects the bank transfer itself.
- The overflow is in an accounting field (`total_granted`), not a security-critical one (no fund movement depends on it).
- In practice the `Custom` tier description field is a human-readable governance escape hatch; abuse would be caught in PR review.

But it's a stylistic inconsistency — `agent-registry` and `zk-verifier` use `checked_add` for similar accumulations.

### Suggested fix

```rust
// In state.rs, GrantTier::Custom:
Custom { amount: u128, description: String },  // unchanged

impl GrantTier {
    pub fn reward_amount(&self) -> u128 { /* unchanged */ }
    
    pub fn is_within_bounds(&self) -> bool {
        const MAX_CUSTOM_AMOUNT: u128 = 100_000_000_000;  // 100k JUNOX cap
        match self {
            GrantTier::Custom { amount, .. } => *amount <= MAX_CUSTOM_AMOUNT,
            _ => true,
        }
    }
}

// In execute_submit_work, after evidence/work_hash validation:
if !tier.is_within_bounds() {
    return Err(ContractError::CustomTierExceedsCap {});
}

// In execute_claim_grant, replace `config.total_granted += reward`:
config.total_granted = config.total_granted
    .checked_add(reward)
    .ok_or(ContractError::Overflow {})?;
```

---

## F5 — `RemoveOperator` doesn't invalidate in-flight verifications (LOW)

### Observation

If operator A has verified submission #5 (`submission.verified_by = Some(A)`, status `Verified`), and admin then runs `RemoveOperator { address: A }`, the submission's verification stands. The builder can still claim.

### Severity rationale — LOW

This is **correct** behavior: verifications were valid at the time they were issued. Retroactively invalidating them would create an unstable security model (operators can verify, then be revoked, then their work is undone). But it's worth documenting that operator removal is not a "kill switch" for the verifications they issued.

### Suggested fix

Documentation only. Add to `state.rs` doc comment on `WORK_HASH_USED` or add an inline comment in `execute_remove_operator`:

```rust
fn execute_remove_operator(...) -> Result<Response, ContractError> {
    // Note: removing an operator does NOT retroactively invalidate
    // submissions they verified. Verifications are point-in-time
    // attestations; their validity does not depend on the operator's
    // current authorization status. If you need to revoke a specific
    // verification, do it via the per-submission flow (currently not
    // exposed — would need a new ExecuteMsg::RevokeVerification).
    ...
}
```

---

## F6 — `SetActive { false }` only gates `SubmitWork`; `Verify` and `Claim` continue (LOW)

### Observation

`execute_submit_work` checks `if !config.active { return Err(SubmissionsPaused) }`. Neither `execute_verify_work` nor `execute_claim_grant` check this flag.

### Severity rationale — LOW

This is **by design**: pause = stop accepting new submissions but flush the pipeline. The admin can `SetActive(false)`, let existing Pending submissions be verified and claimed, then resume. But the design isn't documented anywhere — a casual reader sees `active: bool` and assumes it's a master kill switch.

### Suggested fix

Doc comment on `Config::active`:

```rust
pub struct Config {
    ...
    /// If false, NEW SubmitWork calls are rejected. Existing Pending
    /// submissions can still be verified, and Verified submissions can
    /// still be claimed. This is a soft pause — the pipeline drains.
    /// For a hard kill switch, the admin should additionally withdraw
    /// the balance via Withdraw.
    pub active: bool,
}
```

---

## F7 — No `migrate` entry_point (LOW)

Same as `junoswap-factory` F5. Recommend adding an empty `migrate` stub now so future state-schema changes (e.g., F3 fix introducing `BUILDER_SUBMISSION_IDS` and `PENDING_COUNT`) have an in-place migration path.

---

## F8 — `query_list_submissions` scan-then-filter on status (LOW)

### Observation

`query_list_submissions` (`contract.rs:432-454`) correctly uses `Bound::exclusive(start)` for pagination. But the `.filter_map` on status forces iteration past the start cursor until `limit` items match the filter. For a contract where 99% of submissions are `Claimed` and the query asks for `Pending` with `limit: 10`, the iterator scans most of the map before finding 10 matches.

### Severity rationale — LOW

Today (low submission count) this is fine. Becomes an O(N) gas-leak on skewed distributions at scale.

### Suggested fix

Maintain status-indexed maps:

```rust
// state.rs:
pub const PENDING_SUBMISSIONS: Map<u64, ()> = Map::new("pending_submissions");
pub const VERIFIED_SUBMISSIONS: Map<u64, ()> = Map::new("verified_submissions");
// (also _rejected, _claimed if needed)

// On status transition: remove from old, insert into new.
```

`query_list_submissions` then ranges over the status-specific map first, then loads the full `WorkSubmission` for each — `O(limit)` reads instead of `O(N)` filter.

Costs 4× extra writes per status transition (one delete + one insert per submission). Worth it once submissions count exceeds ~100.

---

## Severity-weighted summary

- **MEDIUM (3):** F1 (silent fund-eating), F2 (admin-rug verified grants), F3 (full-scan query gas leak). All three should land before mainnet.
- **LOW (5):** F4-F8. Polish before mainnet but no block.

## Recommended sprint sequencing

1. **F1 fix** — 15 LoC + 2 error variants + 2 tests. **One-day PR.**
2. **F2 fix** (or doc-only acceptance) — 25 LoC + 1 storage Item + 1 error variant + 3 tests. **Half-day PR.**
3. **F3 fix** — 40 LoC + 2 storage maps + paginated `query_builder_stats` + counter for `query_stats`. **One-day PR.**
4. **F4-F8** — bundle into one polish PR. 50 LoC + 1 error variant + doc comments + `migrate` stub + status-indexed maps.

Total: ~3 days of focused work. None of these block the contract from operating in a controlled testnet environment; all should land before any mainnet deployment that takes real-value contributions.

## Cross-references

- [`junoswap-factory/DETERMINISTIC_AUDIT.md`](../junoswap-factory/DETERMINISTIC_AUDIT.md) F3 — same `range`-without-`Bound` pattern (different file, same cross-cutting concern).
- [`agent-registry/DETERMINISTIC_AUDIT.md`](../agent-registry/DETERMINISTIC_AUDIT.md) F3 — same full-scan query pattern.
- [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md) — methodology + cross-contract finding index. Worth adding a "Full-scan query queries" cross-cutting note now that the pattern has appeared in 3+ contracts.

---

*Audited 2026-05-14 by Cascade/VairagyaNodes deterministic-scrutiny pass. Code anchor: `contracts/builder-grant/src/{contract,state,msg,error,tests}.rs`. Apache-2.0.*
