# PR #1202 Review Comment — paste into GitHub

> Paste the content below the `---` line as a single comment on https://github.com/CosmosContracts/juno/pull/1202

---

## Code Review: `x/voting-snapshot` — three findings from a full read of the keeper

*Reviewer: VairagyaNodes / Cascade. Anchor commit: `0a7098ef07`. Files read in full: `upgrades.go`, `keeper.go`, `backfill.go`, `prune.go`.*

Great work on this — the upgrade handler structure is clean, the keeper doc-comments are precise enough to review against, and the `RetentionWindowHeights = 0` escape hatch is well-shaped. Three findings below, one critical.

---

### 🔴 CRITICAL — `pruneVotingPower` deletes the last snapshot for sparse delegators

**File:** `x/voting-snapshot/keeper/prune.go` (the `pruneVotingPower` function)

The pruner deletes every entry with `height < cutoff` unconditionally. A delegator whose last staking event is older than `RetentionWindowHeights` ends up with **zero entries**, and the "latest-at-or-before" read returns nothing — even though their bonded stake hasn't changed.

**Worked example:** Alice delegates 10,000 JUNO at height 100 and never touches the delegation again. At height 5,250,101 (with `RetentionWindowHeights = 5,250,000`), `cutoff = 101`, her entry at height 100 is deleted. From this point, any DAO querying Alice's voting power returns zero. Set-and-forget delegators are the *median* case on Juno — Mintscan distribution data shows most delegators have zero staking events per quarter.

**Suggested fix:** Two-pass prune that preserves the most-recent-snapshot-per-delegator across the retention boundary. Delete entries with `height < h_max(delegator)` where `h_max ≤ cutoff`, but keep the `h_max` entry itself. This is why `pruneTotalPower` doesn't have the same bug — TotalPower is dense.

**Safe default until fixed:** Ship with `RetentionWindowHeights = 0` (disabled). Unbounded storage growth is the better failure mode vs. silently zeroing sparse delegators. The `if params.RetentionWindowHeights == 0 { return nil }` guard at line 33 already supports this.

**We can send a patch + regression test for this if useful** — estimated ~30 lines of Go plus a `keeper_test.go` case that constructs a sparse delegator, advances past retention, prunes, and asserts the read still works.

---

### 🟡 IMPORTANT — LST exclusion creates silent quorum asymmetry

**File:** `x/voting-snapshot/keeper/backfill.go`

Per-delegator backfill writes `power = ZeroInt()` for LST-allowlisted addresses (line 51-55), but `TotalPower` uses `stakingKeeper.TotalBondedTokens()` (line 68) which **includes** LST bonded stake. Result:

```
Σ VotingPower[d] = total_bonded − Σ lst_bonded
TotalPower       = total_bonded
```

A DAO computing quorum as `Σ votes / TotalPower` has a denominator inflated by the LST share. If LSTs hold 20% of bonded stake, a "33.4% quorum" DAO effectively requires 41.75% of vote-eligible stake. Not necessarily wrong — but needs to be documented explicitly so DAO designers don't miscalculate.

**Suggested fix (any of):** (a) Subtract LST stake from `TotalPower` at backfill + in hooks, (b) document the asymmetry in module docs + field comments, or (c) expose a second `TotalVotablePower` field for DAO quorum arithmetic.

---

### 🟡 IMPORTANT — EndBlocker scan cost is O(total map size)

**File:** `x/voting-snapshot/keeper/prune.go`

The `Pair[[]byte, int64]` key sorts by delegator first, so height-range queries aren't natively indexable. The pruner iterates *all* rows and filters by `K2() < cutoff` in-loop. With `pruneInterval = 1` (hard-coded const), this runs every block.

At 1 year with ~6,500 delegators × 10 events/yr = ~65K rows — fine. At 5 years = ~325K rows scanned every block. Not catastrophic but worth heading off.

**Suggested fix:** Move `pruneInterval` into `types.Params` so it can be tuned via governance (same discipline as `RetentionWindowHeights` already being in `Params`).

---

### Minor notes

- **`upgrades.go` feemarket reset:** `MinBaseGasPrice = 0.075` + `SetState(newState)` resets EIP-1559 state. Operators with `minimum-gas-prices = "0.025ujuno"` in `app.toml` will reject inbound txs post-upgrade. Worth flagging in operator upgrade notes.
- **`ContractFailureRemovalThreshold = 3`:** Does the failure counter reset on successful execution? If monotonic (no reset), it's a soft-DoS surface — adversary spams malformed inputs → force-evicts any hook contract after 3 failures.

---

*Happy to send patches for any of the above. The BN254 forward-port (Track B — wasmvm v3.0.x) is also on our radar per prop #374; let us know if/when that's useful to coordinate.*
