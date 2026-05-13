# `memory/dao-contracts-prs-928-929.md`

## Summary (3 lines)

Jake Hartnell shipped two related PRs to DA0-DA0/dao-contracts in May 2026: **PR #928** (gauge orchestrator finalization) and **PR #929** (`dao-voting-juno-staked` voting module that consumes Juno v30's `x/voting-snapshot`). #929 is the cw-side companion to Juno PR #1202. Our substantive review of #929 is at `docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md` and identifies two HIGH-severity findings worth raising before merge.

## Key facts

| Item | Value |
|---|---|
| PR #928 | Gauge orchestrator finalization (epoch tally, edge cases, integration tests) |
| PR #929 | New contract `dao-voting-juno-staked` (~700 LoC + ~600 LoC schema/tests) |
| Authors | Jake Hartnell, Juno AI, Noah Saso |
| Architecture (#929) | Thin consumer of Juno v30 `x/voting-snapshot` via custom-query binding `JunoQuery` |
| Fixes vs PR #832 | Sparse-delegator bug (chain owns the snapshot, not the contract) |
| Our review of #929 | [`docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md`](../docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md) |
| Findings | 2 HIGH (F1 same-block accumulation; F2 README/code drift on auto-unregister) + 4 LOW/MEDIUM |
| Posting plan | Option B — F1+F2 first, then LOWs in follow-up |

## Full context

### #928 — gauge orchestrator finalization

Closes long-standing edge cases in the gauge orchestrator (epoch tally rollover, tie-breaking, edge weights). Pure incremental work over an existing module; not directly relevant to JunoClaw but signals that the gauges-on-snapshot architecture is now mature enough for production use.

### #929 — `dao-voting-juno-staked`

A DAO DAO voting module derived from staked JUNO using Juno v30's `x/voting-snapshot` module. Replaces the failed PR #832 attempt (which lived in contract storage and couldn't do historical queries).

**Key architecture.**
- `JunoQuery` custom-query binding mirrors `juno/wasmbindings/types/query.go` byte-for-byte.
- `VotingPowerAtHeight` and `TotalPowerAtHeight` proxy directly to the chain.
- LST exclusion enforced at chain layer (allowlist), not in the contract.
- `sudo` handles all 11 staking-hook event types from `x/cw-hooks`.
- Hooks fan out to registered subscribers via `dao_hooks::stake::StakeChangedHookMsg::{Stake, Unstake}`.

**Our findings (full doc: [`docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md`](../docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md)):**

| ID | Severity | Issue |
|----|----------|-------|
| F1 | **HIGH** | Same-block multi-event accumulation duplicate-counts deltas. Two staking events for the same delegator in one block produce two cumulative-from-`chain[h-1]` deltas instead of two incremental deltas. Subscribers tracking flow rather than balance see double-counts. Not in test set. |
| F2 | **HIGH** | README claims "auto-unregistered if their execute call errors" via `reply_on_error` pattern. Code uses plain `SubMsg::new` with an empty reply handler. A bad subscriber halts the whole sudo and breaks delegation. |
| F3 | MEDIUM | `BeforeValidatorSlashed` swallowed silently, trusts unverified lazy-decay claim. |
| F4 | LOW | No `migrate` entry point. |
| F5 | LOW | `i64` cast for height (impractical to hit). |
| F6 | LOW | `auto_register_staking_hooks: Some(false)` is dead surface. |

### Posting plan

**Option B (selected):** Open with F1 + F2 in a single comment ending with one question — *"Do you want patches for these, or are they intentional and we should adjust expectations?"* Hold F3-F6 for follow-up if conversation continues.

The comment-draft markdown is **not yet written** (deferred to user go-ahead). When ready, draft into `docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929_COMMENT_DRAFT.md` and post.

### Relationship to JunoClaw stack

- The `x/voting-snapshot`-on-chain pattern is what `agent-company` should adopt to fix our own F1 finding (vote-weight-at-proposal-creation snapshot). Cf. [`contracts/agent-company/DETERMINISTIC_AUDIT.md`](../contracts/agent-company/DETERMINISTIC_AUDIT.md).
- Jake's structured-memory convention (`memory/v30-upgrade-plan.md` referenced inline in #929 contract code) is what triggered the JunoClaw `memory/` migration this session.
- The Track B forward-port (cosmwasm v3.0.x) is in the same v30 timeline as #929 — both must land for v30 to ship cleanly with both BN254 and snapshot voting.

## Cross-references

- [`docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md`](../docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md) — full review.
- [`docs/JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md`](../docs/JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md) — earlier morning-after analysis.
- [`memory/v30-upgrade-pr-1202.md`](./v30-upgrade-pr-1202.md) — chain-side companion (Juno v30 PR #1202).
- [`memory/track-b-forward-port.md`](./track-b-forward-port.md) — BN254 fork forward-port driven by the same v30 timeline.
- [`memory/lessons-2026-05-13.md`](./lessons-2026-05-13.md) §6 — JunoCommsDept revival amplifying our Medium article + Jake's PRs in parallel.
- [`contracts/agent-company/DETERMINISTIC_AUDIT.md`](../contracts/agent-company/DETERMINISTIC_AUDIT.md) F1 — same architectural fix needed in our own stack.

---

*Apache-2.0.*
