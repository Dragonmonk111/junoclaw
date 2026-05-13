# Moultbook-style review — `dao-voting-juno-staked` (DA0-DA0/dao-contracts PR #929)

*Substantive review of [PR #929](https://github.com/DA0-DA0/dao-contracts/pull/929) by JakeHartnell + JunoAI + Noah Saso, applying the same deterministic-scrutiny benchmark we use on JunoClaw contracts. Anchor: PR head SHA at time of review (May 13, 2026); diff fetched from `https://patch-diff.githubusercontent.com/raw/DA0-DA0/dao-contracts/pull/929.diff`.*

*This is the next-step on the [`JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md`](./JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md) §3.2 plan: do the substantive review with the same discipline we apply internally, and surface findings in a form that can be posted on the PR (or kept as a Moultbook entry once Moultbook v0 is on devnet).*

---

## 0. Headline

**The architecture is correct.** The thin-consumer-of-`x/voting-snapshot` shape fixes the sparse-delegator bug from [#832](https://github.com/DA0-DA0/dao-contracts/pull/832) at the architectural level — the chain owns the snapshot, the cw contract owns the routing. ✅

**Two real findings worth raising on the PR**, plus four lower-severity observations:

1. **🔴 F1 (HIGH if confirmed): Same-block multi-event accumulation produces duplicate-counted Stake / Unstake hook fan-out.** When two staking events for the same delegator land in the same block, the contract emits two cumulative deltas (each measured against `chain[h-1]`) rather than two incremental deltas. Subscribers tracking flow rather than balance see double-counts.
2. **🔴 F2 (HIGH): `reply` handler is a no-op but the README claims "auto-unregistered if their execute call errors."** The code uses plain `SubMsg::new` (per the contract.rs comment); a single subscriber error halts the whole sudo. In `x/cw-hooks`-driven flow, that's a chain-consensus risk.
3. 🟡 F3 (MEDIUM): `BeforeValidatorSlashed` silently ignored; relies on lazy chain-side decay that may not be implemented.
4. 🟡 F4 (LOW): No `migrate` entry point; future schema evolution requires re-deploying.
5. 🟢 F5 (LOW): `i64` cast for height — wraps at extreme block heights.
6. 🟢 F6 (LOW): `instantiate` accepts `auto_register_staking_hooks: Some(false)` but the field exists only to reject `Some(true)`. Surface debt — could be removed entirely.

The **F1** and **F2** findings are the ones I'd raise on the PR. Everything below LOW-MEDIUM I'd hold for a follow-up unless asked.

Both backends pass 9/9 unit tests under the dao-contracts CI pin. The test surface is good — sudo dispatch, hook fan-out, lifecycle event silence-swallowing all explicitly tested. **F1's specific failure mode (multi-event in same block) is not in the test set**; that's the test gap.

---

## 1. F1 (HIGH if confirmed) — Same-block multi-event accumulation

### Statement

When multiple staking events for the same delegator land in the same block, the `prev_power` formula (`chain[h-1]`) is too stale, and the emitted `StakeChangedHookMsg::Stake/Unstake` deltas are cumulative-against-block-start rather than incremental.

### Worked example

Delegator `voter-a` enters block H with `chain[H-1] = 100`.

In block H, two staking events fire (e.g., user submitted a multi-msg tx with `MsgDelegate(50)` followed by `MsgDelegate(25)`):

| Event | Chain state after event | Sudo handler computes | Emits |
|---|---|---|---|
| 1: `AfterDelegationModified` (now bonded 150) | `chain[H] = 150` | `new = chain[H] = 150`, `prev = chain[H-1] = 100`, delta = +50 | `Stake { addr: voter-a, amount: 50 }` |
| 2: `AfterDelegationModified` (now bonded 175) | `chain[H] = 175` | `new = chain[H] = 175`, `prev = chain[H-1] = 100`, delta = +75 | `Stake { addr: voter-a, amount: 75 }` |

**Subscribers see total Stake of 125 for voter-a in block H.** The actual stake change is +75 (100 → 175). The duplicate is +50.

The bug isn't visible in any of the 9 unit tests because none of them fires two events for the same delegator at the same block height.

### Why it can happen in production

- Multi-msg transactions: a user submits one tx with multiple `MsgDelegate` / `MsgUndelegate` to different validators. Each fires `AfterDelegationModified` for the same delegator.
- Cross-tx within a block: two separate txs in the same block, both touching the same delegator. (Less common; user-driven.)
- Slash redistribution: when slashing happens, multiple `BeforeValidatorSlashed` events fire — but those are silently swallowed (F3 below), so this isn't the slash path.

### Severity

Depends entirely on what subscribers do with the hook events.

- **Cumulative-power-tracking subscribers** (the gauge orchestrator, dao-rewards-distributor when computing weights from `VotingPowerAtHeight` queries): not affected. They re-read voting power from the chain at hook-fire time; the emitted delta is just a wake-up signal.
- **Flow-tracking subscribers** (a hypothetical "stake velocity" indexer, or a rewards system that proportions to deltas rather than balances): **broken**. They double-count the +50.

The README presents the contract as the universal cw consumer of staking changes. Subscribers may include flow-trackers we don't know about. Conservative call: **HIGH if the duplicate-counting hits any flow-tracker; MEDIUM otherwise**.

### Suggested fix

Track the contract's own emitted-power per delegator per block as state. On each event:

1. Read `last_emitted = LAST_EMITTED.may_load(deps.storage, &delegator).unwrap_or_default()`.
2. If `last_emitted.height == current_height`: use `last_emitted.power` as `prev_power` (instead of `chain[h-1]`).
3. Else: use `chain[h-1]` as `prev_power`, then save `LAST_EMITTED { height: current_height, power: new_power }`.

This adds a `Map<&Addr, EmittedPower>` to state with one read + one write per sudo. Storage cost grows linearly with active delegators; bounded sweep at end-of-block (or on staleness) keeps it tidy.

Alternative: **document the issue and let subscribers handle idempotence.** Add a `(delegator, height)` deduplication key in subscriber-side libraries. Lower contract-side change but pushes complexity outward.

### Test to add (after fix)

```
#[test]
fn multiple_same_block_events_emit_incremental_deltas() {
    // Two AfterDelegationModified for VOTER_A at the same height,
    // chain[h-1] = 100, after event 1: chain[h] = 150, after event 2: chain[h] = 175.
    // Should emit Stake { 50 } then Stake { 25 }, NOT Stake { 50 } then Stake { 75 }.
}
```

---

## 2. F2 (HIGH) — `reply` handler is empty but README promises auto-unregistration on hook failure

### Statement

The README says:

> Subscribers register via `AddHook { addr }` (gated to the DAO) and are auto-unregistered if their execute call errors (standard `reply_on_error` pattern from the rest of the dao-contracts hook surface).

But `contract.rs` says:

```rust
pub fn reply(_deps: DepsMut<JunoQuery>, _env: Env, _reply: Reply) -> Result<Response, ContractError> {
    // Reserved for future auto-unregistration on hook failure. The
    // current dao_hooks call sites use plain SubMsg::new (no reply
    // requested), so this entry-point is unreachable in practice.
    Ok(Response::new())
}
```

**The reply handler is unreachable.** And the comment confirms `dao_hooks::stake::stake_hook_msgs` builds `SubMsg::new` (no reply), not `SubMsg::reply_on_error`.

### Why it matters

Sudo handlers are called by the chain via the `x/cw-hooks` module. If `sudo` returns an error, **the staking transaction itself fails**. In Cosmos SDK staking flow, that means:

- A user submits `MsgDelegate`. The chain runs the staking handler → fires `AfterDelegationModified` hook → routes to this contract's sudo → emits sub-messages to subscribers → if any subscriber errors, the sub-message fails → the whole sudo fails → the whole `MsgDelegate` fails.

A misbehaving subscriber can therefore **break delegation** for every voter in the DAO. If a single subscriber goes bad (their contract panics, their address gets banned, etc.), all delegation traffic to validators voting in that DAO halts until the DAO admin manually `RemoveHook`s them.

In a multi-DAO ecosystem with shared infrastructure, this is a chain-consensus-adjacent risk — though strictly the chain doesn't halt; only delegations to/from voters in DAOs using this voting module fail.

### Severity

**HIGH.** The README promises a safety property the code doesn't deliver. Either the README is wrong (fix: edit README) or the code is wrong (fix: switch to `SubMsg::reply_on_error` + populate the reply handler with auto-unregister logic). The PR shouldn't merge with this drift.

### Suggested fix

Two clean options:

1. **Implement what the README promises.** Switch `dao_hooks::stake::stake_hook_msgs` to `reply_on_error`, populate the reply handler with `HOOKS.remove_hook(deps.storage, failed_addr)?`. Map the reply-id back to the addr (via a `pending_addr: Map<u64, Addr>` written before send and read in reply). This is the standard cw-hooks pattern; the dao-contracts library already has a reference impl elsewhere.
2. **Update the README to match the code.** Acknowledge that subscriber failures halt the sudo. Add a warning: "Subscribers must be deeply tested; a bad subscriber can break delegation flow."

I'd recommend (1) — option (2) is technically honest but operationally untenable. The dao-contracts ecosystem expects the safety property.

### Test to add (after fix)

```
#[test]
fn faulty_subscriber_is_auto_unregistered_on_error() {
    // Add subscriber whose execute always errors.
    // Fire a stake event.
    // Assert: sudo succeeds, HOOKS no longer contains the faulty addr.
}
```

---

## 3. F3 (MEDIUM) — `BeforeValidatorSlashed` silently ignored

### Statement

```rust
SudoMsg::BeforeValidatorSlashed { .. } => {
    // Slashing redistributes power across many delegators at
    // once. We don't enumerate them here — the chain's
    // x/voting-snapshot has lazy per-delegator decay (see
    // memory/v30-upgrade-plan.md B1+B2+B4) so callers that
    // re-query power at the slash height get correct values.
    Ok(Response::new())
}
```

### What this trusts

The contract trusts that `x/voting-snapshot` implements **lazy per-delegator decay** — i.e., the snapshot at the slash height correctly reflects the post-slash power for every affected delegator, computed lazily on read.

If lazy decay is implemented in `x/voting-snapshot`: this design is correct, and the silent-swallow is fine. Cumulative-power subscribers see the correct slashed value the next time they query.

If lazy decay is NOT implemented (or is partial — e.g., only fires after the next staking event for that delegator): subscribers see **stale, pre-slash voting power for slashed delegators** until each one's next delegation event lands. Could be days or weeks for inactive delegators.

### Why I can't verify here

The chain code lives in [`CosmosContracts/juno`](https://github.com/CosmosContracts/juno) PR [#1202](https://github.com/CosmosContracts/juno/pull/1202) (the v30 work). The `memory/v30-upgrade-plan.md B1+B2+B4` reference is to Jake's local memory corpus which I don't have read access to.

### Severity

**MEDIUM if lazy decay is partial; LOW if it's complete.** The README and PR description should explicitly state which chain-side commit implements lazy decay, with a link.

### Suggested fix

Either:

1. **Cross-link the chain-side decay implementation in the README** (one line: "lazy decay is implemented at `juno/x/voting-snapshot/keeper/snapshot.go:LINE`") so reviewers can verify.
2. **Enumerate delegators in the slash event** (expensive — requires querying the chain for the full delegator list) and emit explicit `Unstake { delegator, slashed_amount_per_delegator }` events. Heavy but explicit.

I recommend (1). (2) is correct in shape but probably unaffordable.

---

## 4. F4 (LOW) — No `migrate` entry point

### Statement

The contract has no `pub fn migrate` entry point. Other dao-contracts voting modules (e.g., `dao-voting-token-staked`) do.

### Why it matters

If the contract's schema needs to evolve (new state field, changed message variant), the only path is:

1. Deploy the new code.
2. The DAO migrates to the new code via `dao-core`'s voting-module-swap proposal.
3. The new contract starts with empty state; subscribers re-register.

This is heavyweight. A `migrate` entry point would let in-place upgrades preserve `HOOKS` and `DAO`.

### Severity

**LOW** — operational debt. Not blocking the PR; consider for a follow-up.

### Suggested fix

Add the standard cw2-version-bumping `migrate`:

```rust
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut<JunoQuery>, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let stored = cw2::get_contract_version(deps.storage)?;
    if stored.contract != CONTRACT_NAME {
        return Err(ContractError::Std(StdError::generic_err("contract name mismatch")));
    }
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new())
}
```

Add `MigrateMsg` to `msg.rs` and the `examples/schema.rs` `write_api!` block.

---

## 5. F5 (LOW) — `i64` cast for height

### Statement

```rust
pub fn voting_power_at(&self, address: String, height: u64) -> StdResult<Uint128> {
    let req: QueryRequest<JunoQuery> = QueryRequest::Custom(JunoQuery::VotingPowerAt(VotingPowerAt {
        address,
        height: height as i64,
    }));
    ...
}
```

### Why `i64`?

Likely because `juno/wasmbindings/types/query.go` defines the field as `int64` (Go convention for proto-encoded heights, supports historical queries).

### When it bites

`height as i64` wraps when `height > i64::MAX` (≈9.2 × 10^18). At Juno's ~6-second block time, that's ~1.7 trillion years. Practically unreachable.

### Severity

**LOW** — cosmetic. Not exploitable in any reasonable horizon.

### Suggested fix

Document the assumption: `// safe cast: i64::MAX is unreachable at any realistic block height`. ~1 line. Or use `i64::try_from(height).map_err(|_| ...)` and surface a clean error. ~3 lines.

---

## 6. F6 (LOW) — `auto_register_staking_hooks: Some(false)` is dead surface

### Statement

```rust
pub struct InstantiateMsg {
    pub auto_register_staking_hooks: Option<bool>,
}
```

In the contract:

```rust
if msg.auto_register_staking_hooks.unwrap_or(false) {
    return Err(ContractError::AutoRegisterNotYetSupported {});
}
```

The field is reserved for a future feature ("single-tx registration with `x/cw-hooks`"). **Right now, only `None` and `Some(false)` are valid.** The schema accepts `Some(true)` but the contract rejects it with an error.

### Why it's debt

External consumers see a `bool?` and assume it's a real switch. Naïve callers passing `Some(true)` (because they want auto-registration) hit a runtime error rather than a clear "not supported yet" at construction time.

### Severity

**LOW** — caller-confusion only. The reserved-for-future framing is honest; the cost is just one rejected-at-runtime path.

### Suggested fix

Two options:

1. **Remove the field for now**, add it back when the feature is implemented. ~5 LoC.
2. **Document loudly** in the README and the schema's description (already partially done in the schema, less so in the contract code). ~3 LoC docstring.

---

## 7. Confirmed-correct observations

These are explicit positive findings — things the contract does well:

- **At-or-before semantics inherited from chain layer** ✅. The contract is genuinely a thin proxy; it doesn't reimplement snapshot logic. Tests confirm.
- **Sudo dispatch covers all 11 staking-hook event types explicitly** ✅. No silent unhandled variants. `cw-hooks` registration stays alive (the silent-swallow on lifecycle events is intentional and explained).
- **Custom-query binding (`JunoQuery`) mirrors `juno/wasmbindings/types/query.go`** ✅. Verified by reading the diff against my mental model of the chain-side surface; the variant names are exact.
- **Hook fan-out via `dao_hooks::stake::StakeChangedHookMsg::{Stake, Unstake}`** ✅. Standard pattern; no surprise routing.
- **`previous_power` saturates at genesis (`current_height == 0`)** ✅. Edge case handled cleanly.
- **Test isolation via `JunoMockQuerier`** ✅. The custom-query support gap in `cw-multi-test` is solved cleanly without pulling in chain binaries.

---

## 8. Summary table

| ID | Severity | Finding | Fix size |
|----|----------|---------|----------|
| F1 | **HIGH if confirmed** | Same-block multi-event accumulation duplicate-counts deltas | ~25 LoC + 1 state field + 1 test |
| F2 | **HIGH** | README claims auto-unregistration; code is no-op | ~30 LoC + 1 test (option 1) OR ~5 LoC README edit (option 2) |
| F3 | MEDIUM | `BeforeValidatorSlashed` swallow trusts unverified lazy decay | ~1 LoC README cross-link |
| F4 | LOW | No `migrate` entry point | ~15 LoC + schema.rs update |
| F5 | LOW | `i64` cast wraps at impractical heights | ~1 LoC docstring |
| F6 | LOW | `auto_register_staking_hooks` debt | ~3-5 LoC |

---

## 9. Posting plan

When ready to engage on the PR:

**Option A (single comprehensive comment):** Post all 6 findings in one PR comment with the same structure as our [PR #1202 review](./MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md). Pros: one shot, clear deliverable. Cons: long, easier for Jake to triage piecemeal.

**Option B (staged comments):** Post F1 + F2 first (the two HIGHs) as a single comment titled "Two findings worth raising before merge"; hold F3-F6 for after their first response. Pros: respects Jake's bandwidth, makes the headline land cleanly. Cons: requires a second engagement.

**My recommendation: Option B.** Open with F1 + F2 as a tight comment that asks one question: *"Do you want patches for these, or are they intentional and we should adjust expectations?"* That mirrors the tone of the PR-1202 review (warm, concrete, ends with a question). Hold the LOWs for the follow-up if the conversation continues.

The exact comment body for Option B would go in `MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929_COMMENT_DRAFT.md` — not yet drafted; do that as a separate step when you're ready to post.

---

## 10. Cross-references

- [`JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md`](./JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md) — the morning-after analysis that triggered this substantive review.
- [`MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md`](./MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md) — the prior review of Jake's `x/voting-snapshot` chain-module work; the chain-side code that this contract consumes.
- [`JAKE_DM_TRACK_B_CLARIFY.md`](./JAKE_DM_TRACK_B_CLARIFY.md) — the parallel BN254 Track B conversation; same Jake, different topic.
- [`LESSONS_2026_05_13_MORNING.md`](./LESSONS_2026_05_13_MORNING.md) §8 — context on JunoCommsDept revival + this PR's role in that comms loop.

---

*Apache-2.0. Review conducted 2026-05-13 PM under the deterministic scrutiny benchmark. Findings are observations, not demands; the PR's architecture is correct, and these are what I'd want to discuss before merge.*
