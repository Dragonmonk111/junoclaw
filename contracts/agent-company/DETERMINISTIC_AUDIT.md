# Agent-Company — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark (2026-05-12). Anchor commit: `06fee11` on origin/main. Files read in full: `src/contract.rs`, `src/state.rs`, `src/error.rs`. Cargo-level: `agent-company v0.1.0`, depends on `junoclaw-common`, `cw-storage-plus`, `sha2`.*

---

## 0. Surface summary

| Area | Lines (in `contract.rs`) | Functions |
|------|--------------------------|-----------|
| Entry points | 46-200 | `instantiate`, `execute`, `query`, `migrate` |
| Value flow | 202-274 | `execute_distribute` |
| Governance creation | 280-406 | `execute_create_proposal` |
| Governance voting | 408-525 | `execute_cast_vote` |
| Governance execution | 527-857 | `execute_execute_proposal` (8 ProposalKind branches) |
| Randomness | 891-1017 | `select_members`, `resolve_sortition`, `execute_nois_receive`, `execute_submit_randomness` |
| Attestation | 1020-1203 | `execute_submit_attestation` (zk-sidecar branch + hash recompute + task-ledger coherence) |
| Member / admin mutation | 1211-1370 | `execute_update_members`, `execute_transfer_admin`, `execute_rotate_*` |
| Migration | 1372-1417 | `migrate` |

State complexity (per `state.rs`):
- 8 ProposalKind variants (WeightChange, WavsPush, ConfigChange, FreeText, OutcomeCreate, OutcomeResolve, SortitionRequest, CodeUpgrade)
- 5 CodeUpgradeAction variants (StoreCode, InstantiateContract, MigrateContract, ExecuteContract, SetDexFactory)
- 7 storage items: CONFIG (singleton), PROPOSAL_SEQ, SORTITION_SEQ, PROPOSALS, MEMBER_EARNINGS, PAYMENT_HISTORY, SORTITION_ROUNDS, PENDING_SORTITION, ATTESTATIONS

---

## 1. Gas Trace — hot paths

### 1.1 `execute_distribute` — N-way bank send to members

| Step | Operation | Host calls | SDK gas est. |
|------|-----------|------------|--------------|
| CONFIG.load | 1× `db_read` (~600 bytes — Config is large) | 1 | ~3,500 |
| Member-or-admin check | Pure wasm scan of `cfg.members` (N members) | 0 | ~50N |
| PAYMENT_HISTORY.has | 1× `db_read` | 1 | ~2,000 |
| Funds filter / sum | Pure wasm | 0 | ~100 |
| Per-member loop (N members) | Each iteration: 1× `db_read` (MEMBER_EARNINGS), 1× `db_write` (MEMBER_EARNINGS), 1× BankMsg push | 2N | N × ~5,000 |
| PAYMENT_HISTORY.save | 1× `db_write` (~40 bytes) | 1 | ~3,000 |
| BankMsg dispatch (N) | Per-coin native send | — | N × ~3,000 native |

**Total at N=5 members:** ~33K SDK + ~15K native = **~48K SDK gas + 5× ~3K native bank sends**. With 3× wasm gas multiplier: ~144K. Within ADR-002's projected 100-200K envelope. ✅

**At N=20 members (extreme upper bound):** ~133K SDK + 60K native = ~399K + 60K. Still ships in a 25M block-gas-budget. ✅

### 1.2 `execute_cast_vote` — vote landing + auto-resolution

| Step | Op | Host calls | SDK gas est. |
|------|-----|------------|---|
| CONFIG.load | 1× `db_read` | 1 | ~3,500 |
| Member lookup (linear scan) | Pure wasm | 0 | ~50N |
| PROPOSALS.load | 1× `db_read` (~variable, ~300 bytes for vote-sparse, ~2KB for fully-voted) | 1 | ~3,000-8,000 |
| Status / deadline / dedup checks | Pure wasm | 0 | ~200 |
| `proposal.votes.push` + tally arithmetic | Pure wasm | 0 | ~200 |
| Adaptive deadline check | Pure wasm | 0 | ~100 |
| Auto-resolution arithmetic | Pure wasm (`cfg.total_weight * percent / 100`) | 0 | ~100 |
| PROPOSALS.save | 1× `db_write` | 1 | ~5,000 |

**Total:** ~12-17K SDK gas → **~36-51K** with multiplier. Cheap. ✅

### 1.3 `execute_submit_attestation` — auth + hash + zk-sidecar branch

| Step | Op | Host calls | SDK gas est. |
|------|-----|------------|---|
| CONFIG.load | 1 | 1 | ~3,500 |
| Auth checks | Pure wasm | 0 | ~50 |
| PROPOSALS.load | 1 | 1 | ~3,000-8,000 |
| Status / kind validation | Pure wasm | 0 | ~50 |
| Task-ledger query (cross-contract) | 1× `query_chain` (Wasm Smart query) | 1 | ~15,000-30,000 |
| SHA256 attestation_hash recompute | Pure wasm | 0 | ~500 |
| ATTESTATIONS.has | 1× `db_read` | 1 | ~2,000 |
| ATTESTATIONS.save | 1× `db_write` | 1 | ~5,000 |
| zk-verifier sub-msg (if proof present) | 1× WasmMsg::Execute → triggers BN254 verify in zk-verifier (~700K gas with the BN254 precompile, ~2M without) | — | downstream |

**Total parent frame:** ~30-50K SDK without proof, ~30-50K + downstream BN254 with proof. The proof verification is the dominant cost when present, and that's the BN254 precompile's whole point (we've already measured this: ~250K with the precompile vs ~21M without, per the 311/311 baseline). ✅

---

## 2. Failure Mode Enumeration

### 🔴 F1 — Vote weights are read at vote time, not snapshot at proposal creation

**Location:** `execute_cast_vote` line 418-419, line 442-455.

**The flaw.** `cfg.members.iter().find(|m| m.addr == info.sender)` reads the **current** member list at vote time. If a `WeightChange` proposal passes between proposal A's creation and a vote on A, the new weights apply retroactively to all *future* votes on A. Past votes already in `proposal.votes` keep their historical weight, but anyone who hasn't voted yet sees a different weight.

**Concrete adversarial sequence:**

1. DAO has 4 members: A (3000), B (3000), C (2500), D (1500).
2. D creates **Proposal P1** (FreeText, ordinary 51% quorum).
3. A creates **Proposal P2** (WeightChange: A→9999, B→0, C→0, D→1). Voting deadline shared with P1.
4. A, B, C collude and vote Yes on P2 (8500 / 10000 ≥ 6700 supermajority). **P2 passes and is executed before P1 closes.**
5. D, who hadn't voted on P1 yet, now votes Yes on P1 — but D's weight is now **1**, not 1500.
6. A votes No on P1 with weight 9999. P1 fails despite the original intent.

**Why this matters at the design level.** Most production governance contracts (Compound, DAODAO, OpenZeppelin Governor) **snapshot** voter power at proposal creation precisely to prevent this attack class. The agent-company contract does not. Compound's snapshot pattern uses a per-proposal `quorumVotes` field set at creation and per-voter `getPriorVotes(account, proposalBlock)` lookups; the same pattern lands cleanly in CosmWasm via `cw-storage-plus::Snapshot<T>`.

**Worked impact at the JunoClaw threshold.** The 9 DAO templates in `DaoPanel.tsx` (community_fund, crop_protection, credential_verifier, …) all use the same agent-company contract under the hood. Any of those DAOs with ≥4 members and ≥1 concurrent open proposal is exposed. Probability of a malicious coalition forming in any one DAO is low; probability across all 9 template instances over a year is non-trivial.

**Suggested fix.** Add `member_weights_snapshot: Vec<(Addr, u64)>` and `total_weight_snapshot: u64` to the `Proposal` struct, populated at `execute_create_proposal`. In `execute_cast_vote`, look up the voter's weight from this snapshot, not `cfg.members`. Storage cost is bounded (~N×50 bytes per proposal, where N=member count, capped by `parse_members`). For a 20-member DAO with 1000 historical proposals, ~1 MB additional storage — trivial.

**Severity: HIGH** — exploitable, governance-violating, requires multi-party collusion but the collusion path is natural (any WeightChange supermajority).

**Regression test plan.** A new test in `tests.rs`:
```
fn weight_change_does_not_retroactively_affect_open_proposals() {
    // 4-member DAO
    // member D creates FreeText proposal P1
    // members A,B,C execute WeightChange P2 to zero out C,D and amp A
    // D votes Yes on P1 — assert D's weight is still original (1500), not new (1)
}
```

---

### 🟡 F2 — `WavsPush` escrow `Authorize` always routes payee to admin

**Location:** `execute_execute_proposal` line 640-647 (the `WavsPush` branch).

**The constraint.** Every WAVS-task escrow obligation is authorized with `payee: cfg.admin.to_string()`. There's no way for a member-proposed task to declare a different payee. This means:

- A multi-member DAO with 10 task-doers can't natively pay them individually — every task's escrow lands at admin first, then admin has to manually redistribute via `DistributePayment` or a bank send.
- The admin becomes a value-flow funnel and a permanent trust anchor. If admin is compromised or AWOL, escrows freeze.
- The `DistributePayment` path does fan out to members by weight, but that requires admin to receive first and then call distribute. Two-step. Centralised.

**Severity: MEDIUM** — design limitation, not a bug; documented behaviour but limits expressiveness.

**Suggested fix.** Extend `ProposalKind::WavsPush` with an optional `payee: Option<String>` field. Default `None` → current behaviour (route to admin). `Some(addr)` → route to the named payee, validated at proposal creation. Backwards-compatible at the serde layer.

---

### 🟡 F3 — `DistributePayment` silently absorbs non-`cfg.denom` funds

**Location:** `execute_distribute` line 230-233.

**The flaw.** The funds filter only counts coins with `denom == cfg.denom`. Coins of any other denom that arrive in the same MsgExecuteContract.funds are credited to the contract's bank balance but never enter the distribution arithmetic, and there's no path to withdraw them. They sit in the contract permanently.

**Severity: LOW** — gated by F2 (only members and admin can call), so the blast radius is small. But it's still a real value-trap: a member who accidentally sends `1ujuno + 100uosmo` (e.g. IBC-routed denom mix-up) loses the uosmo.

**Suggested fix (any of):**

1. **Reject mixed funds.** If `info.funds.len() != 1 || info.funds[0].denom != cfg.denom`, error with `ContractError::Std("DistributePayment accepts only cfg.denom funds; got {denoms:?}")`.
2. **Refund mismatch.** Add a BankMsg::Send back to `info.sender` for any non-cfg.denom coins. Cleaner UX but adds a side-effect.
3. **Add a sweep entrypoint.** A new admin-only `SweepDenom { denom, recipient }` execute that drains accidentally-stuck balances.

Option 1 is the minimal-diff fix.

---

### 🟢 F4 — `cfg.total_weight` and quorum percentages mutate mid-proposal

**Location:** `execute_cast_vote` line 487-512 (auto-resolution arithmetic uses `cfg.total_weight`, `cfg.quorum_percent`, `cfg.supermajority_quorum_percent`).

**Variant of F1.** If a `ConfigChange` or a future `quorum_percent` change passes during P1's voting window, the auto-resolution thresholds for P1 change retroactively. Same mitigation pattern: snapshot the relevant config fields at proposal creation.

`TOTAL_WEIGHT` itself is a hard-coded `const = 10_000` so its `cfg.total_weight` storage field is always 10,000 by construction — no drift on that axis. But `quorum_percent` and `supermajority_quorum_percent` are mutable.

**Severity: LOW-MEDIUM** depending on whether mid-stream quorum changes are part of the threat model. Fix in lockstep with F1's snapshot.

---

### 🟢 F5 — `select_members` Fisher-Yates is deterministic but produces order-dependent output

**Location:** `select_members` lines 900-917.

**Observation.** The sortition shuffles indices in-place, then truncates to `count`. The resulting `selected` vector preserves the *post-shuffle index order*. If two DAOs receive the same 32-byte randomness with the same eligible list, they get the same selected set — that's the desired determinism. Different eligible-list orderings of the same set, however, would produce different selections, which is fine because the eligible list is built from `cfg.members.iter()` which preserves insertion order.

**Edge case.** If the `cfg.members` list is reordered via a `WeightChange` that keeps the same members but changes their order (legal under `parse_members`), a *pending* sortition's randomness would, when applied, produce a different selection than it would have before the reorder. The pending sortition snapshots `eligible: Vec<Addr>` in `PendingSortition`, so this is already mitigated. ✅

**Severity: NONE** — already mitigated by the eligible-list snapshot in `PendingSortition`.

---

### 🟢 F6 — `parse_members` uses `std::collections::HashSet` for dedup

**Location:** `parse_members` line 30.

**Concern under the deterministic benchmark.** The audit rule says "no HashMap iteration". `HashSet::insert()` returns whether the value was new — this result is deterministic regardless of iteration order. The HashSet is never iterated. ✅

**Suggested polish (optional).** Replace with `BTreeSet<Addr>` for consistency with the broader rule — same complexity, deterministic iteration if ever needed, removes the rule-violation aesthetic flag. One-line change, no behavioural impact.

**Severity: NONE** (false alarm under the strict rule; cosmetic).

---

### 🟢 F7 — `CodeUpgradeAction::StoreCode` is event-only

**Location:** `execute_execute_proposal` line 764-777 (StoreCode branch).

**Documented behaviour.** The comment at line 770-772 is explicit: *"StoreCode is not a native CosmWasm message — the DAO records the intent on-chain; off-chain relayer (bridge) handles the actual wasmd StoreCode via authz or gov proposal."*

**Implication.** A DAO that votes through a CodeUpgrade proposal expecting wasm to be stored on-chain by the contract itself will see an event and no actual upload. Trust assumption on off-chain relayer.

**Severity: LOW** (design choice, documented inline).

**Suggested follow-up.** When CosmWasm v3 lands with `MsgStoreCode` access for contracts (some chains expose this via authz or x/wasm/MsgStoreCode), revisit. Probably v1-of-this-contract work, not v0.

---

### 🟢 F8 — `execute_submit_attestation` pins WAVS version string to v0.1.0

**Location:** line 1099-1101: `hasher.update(b"junoclaw-wavs-v0.1.0")`.

**Concern.** If WAVS components later emit attestations against `"junoclaw-wavs-v0.2.0"`, the on-chain hash recomputation rejects them.

**Suggested fix.** Move the version string into `Config` as `wavs_attestation_version: String` (default `"junoclaw-wavs-v0.1.0"`), updateable via `RotateWavsOperator` or a new `SetWavsAttestationVersion` admin-only entrypoint. Cheap, future-proofs the contract against WAVS component upgrades.

**Severity: LOW** — intentional pinning now, but the lever should be governance-tunable.

---

### 🟢 F9 — Unbounded proposal creation rate

**Location:** `execute_create_proposal` — no rate-limit or per-member cooldown.

**Concern.** A single member can spam proposals, growing the `PROPOSALS` map. Gas is the natural throttle (each proposal creation costs ~10-20K SDK gas), and the only attack scenario is a malicious zero-weight member… except zero-weight members are blocked at line 291-293. So only weighted members can spam, and they pay gas for it.

**Severity: NONE** — already mitigated by gas cost + zero-weight gate.

---

## 3. Storage Layout Analysis

### Prefix map

| Storage item | Prefix | Key | Value size |
|---|---|---|---|
| CONFIG | `"config"` | singleton | ~800 bytes (members + verification + 8 addr/option fields) |
| PROPOSAL (legacy) | `"proposal"` | singleton | ~variable |
| PROPOSAL_SEQ | `"proposal_seq"` | singleton | 8 bytes |
| SORTITION_SEQ | `"sortition_seq"` | singleton | 8 bytes |
| PROPOSALS | `"proposals"` | u64 | ~300-2KB (sparse vs fully-voted) |
| MEMBER_EARNINGS | `"member_earnings"` | &Addr | 16 bytes (Uint128) |
| PAYMENT_HISTORY | `"payment_history"` | u64 | ~40 bytes |
| SORTITION_ROUNDS | `"sortition_rounds"` | u64 | ~variable, ~500 bytes per round |
| PENDING_SORTITION | `"pending_sortition"` | &str (job_id) | ~variable, eligible list |
| ATTESTATIONS | `"attestations"` | u64 | ~200 bytes |

**No prefix collisions.** All prefix strings are distinct. ✅

### Scale projections

| Scenario | Storage impact |
|---|---|
| 100 proposals lifetime | PROPOSALS: ~30-200 KB |
| 10,000 proposals lifetime | PROPOSALS: ~3-20 MB |
| 100K payment distributions | PAYMENT_HISTORY: ~4 MB |
| 1M sortition rounds | SORTITION_ROUNDS: ~500 MB — would warrant pagination/pruning |

**Iterator usage.** No full-scan iterators in the contract. All reads are point-keyed by proposal_id or addr. Pagination handled by query handlers via cw-storage-plus `Bound::exclusive`. ✅

### What's NOT pruned

- `PROPOSALS` grows forever. After 5 years of weekly proposals: ~260 entries, trivial.
- `PAYMENT_HISTORY` grows forever. With one task per day: ~1,800 entries after 5y, still trivial.
- `SORTITION_ROUNDS` grows forever, by design (sortition history is part of the audit trail).
- `ATTESTATIONS` grows forever.

**Verdict.** No pruning needed at any realistic scale. ✅ (Contrast with `x/voting-snapshot/keeper/prune.go` in Juno v30 PR #1202 where pruning was attempted and broke correctness — agent-company correctly chooses unbounded growth here.)

---

## 4. Determinism Proof

| Concern | Status |
|---|---|
| No floating point | ✅ — all arithmetic is `u64`, `u128`, or `Uint128` checked-math |
| No HashMap iteration | ✅ — `HashSet` used only for `insert` return value; never iterated |
| No `std::time` / SystemTime | ✅ — block height + block time from `env` only |
| No randomness in state transitions | ✅ — randomness comes via NOIS callback or WAVS submission, both signed/attested |
| Fisher-Yates uses deterministic sub-randomness | ✅ — SHA256(seed‖counter) per swap |
| Serde round-trip stability | ✅ — `cw_serde` everywhere; no custom Serialize impls |
| External Wasm queries are deterministic within-block | ✅ — task-ledger query, zk-verifier query, NOIS query are all single-block-deterministic by CosmWasm's execution model |

**All clear.** ✅

---

## 5. Serde Boundary Hardening

**Schema evolution risk.** Several state structs (`Config`, `Proposal`, `Member`) have grown across v4 → v7 with new fields. The current pattern uses `#[serde(default)]` on the newest fields (e.g. `wavs_operator`, `zk_verifier`, `dex_factory`, `supermajority_quorum_percent`), which is correct.

**Verified additions during migration:** the `migrate` entry-point at line 1372-1417 explicitly re-serialises the stored Config to bridge old → new shape. Good defensive practice.

**Future-field discipline (same as Moultbook SE1).** All future field additions to `Config`, `Proposal`, `Member`, `Attestation`, `SortitionRound` MUST be `Option<T>` with `#[serde(default)]`. Document this as a contract invariant.

**Malformed `msg_json` in CodeUpgradeAction.** `execute_create_proposal` line 364-371 already eager-validates via `from_json::<serde::de::IgnoredAny>`. ✅

---

## 6. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | **HIGH** | Snapshot member weights + quorum percentages at proposal creation; read from snapshot in `cast_vote` and `auto-resolution` | ~50 LoC + 2-3 regression tests |
| F2 | **MEDIUM** | Add optional `payee` field to `WavsPush` proposal kind | ~10 LoC + 1 test |
| F3 | **LOW** | Reject mixed funds in `DistributePayment` (option 1 from §2.3) | ~5 LoC + 1 test |
| F4 | covered by F1 | snapshot quorum percentages alongside member weights | (folded into F1) |
| F5 | NONE | — | — |
| F6 | NONE | Optional cosmetic: `HashSet` → `BTreeSet` | 1 LoC |
| F7 | LOW | Document StoreCode is event-only in ADR/README; revisit with CosmWasm v3 | docs only |
| F8 | LOW | Move WAVS version string into Config | ~5 LoC + 1 test |
| F9 | NONE | — | — |

**Recommendation:** Land F1 + F2 + F3 in a single follow-up PR (`agent-company-v7.1`). F8 in a later sweep. F6/F7 are cosmetic/docs-only.

---

## 7. Comparative gas cost (vs Moultbook v0)

| Operation | agent-company | moultbook-v0 |
|---|---|---|
| Hot-path execute (post / vote) | ~36-51K SDK gas | ~36.5K SDK gas |
| With cross-contract query | ~80-90K (task-ledger) | ~80K (whoami) |
| With cryptographic verification | ~250K with BN254 precompile | N/A (v0) |

Both contracts are within the ADR-002 envelope and well below block gas budget. ✅

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark codified after Ffern Institute's Lex Fridman pointer (see `MOULTBOOK_DEV_COLLABORATION_NOTES.md` §3). Once Moultbook v0 is on devnet, this audit re-lands as a Moultbook entry citing `agent-company` commit `06fee11` as its anchor.*
