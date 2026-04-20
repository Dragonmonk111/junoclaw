# JunoClaw Tier 1-slim + Tier 1.5 — Architecture Upgrade

**Target contract:** `task-ledger`
**Chain:** Juno `uni-7` testnet
**Live Tier 1.5 address:** `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` (code_id **75**, wasmd-admin-enabled)
**Retired v6 address:** `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` (code_id 70, frozen — no wasmd admin set at instantiate; preserved in `deployed.json` as `task-ledger-v6-frozen`)
**Status:** **Deployed to `uni-7` on 2026-04-20.** All three on-chain smoke tests passing (T1 after a harness patch; T2 and T3 first try). Workspace tests 149 / 149 green (up from 148 after same-day addition of the error-string shape regression test — see §11).
**Deployment mode:** Fresh-deploy, not migrate (the v6 contract could not be migrated). See `@junoclaw/docs/TIER15_TESTNET_RUN.md` for the full run log.
**Owner of shipping decision:** wasmd-level admin of the contract (`The Builder`) — historical. Tx hashes in `@junoclaw/docs/TIER15_TESTNET_RUN.md`.
**Pairs with:** `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`, `@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md`, `@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md`, `@junoclaw/docs/TIER15_TESTNET_RUN.md`, `@junoclaw/docs/RATTADAN_HARDENING.md`.

> This document is the canonical engineering reference for everything shipped
> today under the Tier 1-slim / Tier 1.5 banner. It is pair-read with the
> observation tracker (for the field-ops view) and the Medium article (for the
> narrative view). It does not replace an external audit. Every numeric claim
> below is grounded in a file path and a test; any drift from either means the
> document is stale.

---

## 1. Summary

### What it is

A declarative, bounded, read-only **pre/post completion hook vocabulary** for `task-ledger::CompleteTask`. Submitters attach `Vec<Constraint>` to a task at submission time; the ledger evaluates them at completion time; a failure returns `ConstraintViolated` which atomically reverts the completion transaction, leaving the task `Running`, the escrow un-settled, and the registry un-incremented.

### What it is not

Not an intent-solver, not an observer-registry, not a timelock-custody contract, not a WAVS replacement. See `@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md` "What this is not" for the detailed boundary arguments.

### Shape of the delta

| Aspect | v6.1 (code_id 70) | Tier 1.5 (pending code_id) |
|---|---|---|
| Variants on `Constraint` | 4 | **7** (+ `TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed`) |
| Fields on `TaskRecord` | baseline | **+ `pre_hooks`, `post_hooks`** (both `#[serde(default)]`) |
| Fields on `ExecuteMsg::SubmitTask` | baseline | **+ `pre_hooks`, `post_hooks`** (both `#[serde(default)]`) |
| Error variants on `ContractError` | baseline | **+ `ConstraintViolated { reason }`** |
| Storage layouts | baseline | **unchanged** (no new `Item`/`Map`) |
| cw2 migrate authority | wasmd admin | **wasmd admin** (unchanged) |
| New cross-contract dependencies | 0 | **0** (all variants query v6-exposed surfaces) |
| Optimized wasm size | 305.9 KB | **386.8 KB** (+81 KB, +26.5%) |
| Regression tests on `task-ledger` | 24 | **27** (+3 Tier 1.5 tests) |
| Workspace regression tests total | 145 | **148** |

---

## 2. Scope — files touched

All paths absolute-from-repo-root.

### Contract code

- `@junoclaw/contracts/junoclaw-common/src/lib.rs` — added 3 new `Constraint` variants (`TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed`), threaded `&Env` through `Constraint::evaluate` / `evaluate_all`, added `EscrowQuery::GetObligationByTask` typed-query and `PoolReservesView` deserialisation shim for `PairReservesPositive`.
- `@junoclaw/contracts/task-ledger/src/msg.rs` — extended `ExecuteMsg::SubmitTask` with `pre_hooks: Vec<Constraint>` / `post_hooks: Vec<Constraint>`, both `#[serde(default)]`.
- `@junoclaw/contracts/task-ledger/src/contract.rs` — in `execute_submit`, plumb hooks into the `TaskRecord`; in `execute_complete`, add pre-hook evaluation before the state transition (wrapped in `pre_hook:`) and post-hook evaluation after but before sub-messages append (wrapped in `post_hook:`), both returning `ConstraintViolated` on first failure.
- `@junoclaw/contracts/task-ledger/src/error.rs` — add `ConstraintViolated { reason: String }` variant with `#[error(...)]` attribute.
- `@junoclaw/contracts/task-ledger/src/tests.rs` — add 3 new integration tests and a `StubEscrow` helper contract that responds to `GetObligationByTask` for the Tier 1.5 tests.

### Tooling and docs (new)

- `@junoclaw/deploy/migrate-tier15.mjs` — uploads new wasm → migrates live contract → sanity-checks `GetConfig` → records everything to `deployed.json`. Pre-flight checks the wasmd-level admin match with `client.getContract(addr)` before broadcasting. Supports `DRY_RUN` and `SKIP_UPLOAD` env flags.
- `@junoclaw/deploy/smoke-tier15.mjs` — three on-chain integration tests (`TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed`), each exercising both violation and success paths on the same task via atomic revert.
- `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md` — field operations tracker for the observation window.
- `@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md` — narrative Medium piece on the primitive.
- `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md` — this file.

### Tooling and docs (unchanged)

`@junoclaw/deploy/deploy.mjs` (fresh-deploy script, untouched). `@junoclaw/deploy/smoke-v6.mjs` (v6 regression harness, untouched). The v6 migration path via `deploy.mjs` is preserved for fresh deployments on new chains.

---

## 3. Data model

### Constraint enum (7 variants)

```rust
pub enum Constraint {
    AgentTrustAtLeast { agent_id: u64, min_score: u64 },
    BalanceAtLeast    { who: Addr, denom: String, amount: Uint128 },
    PairReservesPositive { pair: Addr },
    TaskStatusIs      { task_ledger: Addr, task_id: u64, status: TaskStatus },
    TimeAfter         { unix_seconds: u64 },           // Tier 1.5
    BlockHeightAtLeast { height: u64 },                // Tier 1.5
    EscrowObligationConfirmed { escrow: Addr, task_id: u64 }, // Tier 1.5
}
```

Evaluator signature: `fn evaluate(&self, deps: Deps, env: &Env, agent_registry: Option<&Addr>) -> Result<(), String>`.

### TaskRecord additions

```rust
pub struct TaskRecord {
    // ... v6 fields unchanged ...
    #[serde(default)] pub pre_hooks:  Vec<Constraint>,
    #[serde(default)] pub post_hooks: Vec<Constraint>,
}
```

### Error additions

```rust
pub enum ContractError {
    // ... v6 variants unchanged ...
    #[error("Constraint violated: {reason}")]
    ConstraintViolated { reason: String },
}
```

### Storage diff

None. No new `Item<T>`, no new `Map<K,V>`. Pre-v7 storage reads verbatim; serde fills `pre_hooks` / `post_hooks` with `Vec::new()` on absence.

---

## 4. Control flow

### `execute_submit` (v7)

```
existing admission checks (agent ownership, proposal-id uniqueness, reserved IDs)
    ↓
TaskRecord::new including pre_hooks and post_hooks
    ↓
TASKS.save(storage, task_id, &record)
    ↓
Response with task_id event attribute
```

Hooks are **stored, not evaluated** at submit time. Submit admission is unchanged from v6.

### `execute_complete` (v7)

```
auth gate (admin / operator / agent_company)
    ↓
load TaskRecord; reject if not Running
    ↓
[v7] if !pre_hooks.is_empty():
    evaluate_all(deps, env, pre_hooks, registry)
        .map_err(|r| ConstraintViolated { reason: format!("pre_hook: {r}") })?
    ↓
mutate: record.status = Completed; TASKS.save
    ↓
[v7] if !post_hooks.is_empty():
    evaluate_all(deps, env, post_hooks, registry)
        .map_err(|r| ConstraintViolated { reason: format!("post_hook: {r}") })?
    ↓
append escrow::Confirm sub-message
append agent-registry::IncrementTasks sub-message
    ↓
Response with all sub-messages
```

Critical property: **every `?` above is an atomic revert point**. A post-hook failure discards the `Completed` status mutation and prevents both sub-messages from being dispatched. This is the v5 coherence inheritance.

### Post-hook design note

Post-hooks are evaluated **after** the status transition but **before** the sub-messages append to the `Response`. That means they see the task's new `Completed` status (visible via self-query) but do not see the downstream effects of the escrow and registry callbacks. For hooks that need to assert on downstream-callback effects (e.g. "the registry trust score actually did increment"), a `reply`-based extension would be required in a future v8.

---

## 5. Audit loop — explicit checklist

The audit delta is narrow enough that an external reviewer can walk it in a single session. The order below is the suggested reading sequence.

1. **Read `@junoclaw/contracts/junoclaw-common/src/lib.rs:248-436`** — the `Constraint::evaluate` impl and `evaluate_all` helper. Check that each variant's query path uses `.map_err(|e| format!(...))?` for every fallible step, that none of them write to storage, that none of them dispatch sub-messages, and that each returns a deterministic `Ok(())` / `Err(String)` based only on inputs.

2. **Read `@junoclaw/contracts/task-ledger/src/contract.rs:280-340`** — the `execute_complete` v7 pre/post hook integration. Check that pre-hook evaluation happens *before* the state mutation, post-hook evaluation *after* the state mutation but *before* sub-message append. Check that both wrappers produce `ContractError::ConstraintViolated`.

3. **Read `@junoclaw/contracts/task-ledger/src/contract.rs:670-682`** — the `migrate` entry-point. Check that it asserts `contract == CONTRACT_NAME`, bumps the cw2 version tag, and returns `Response::default()` — no storage mutation.

4. **Read `@junoclaw/contracts/task-ledger/src/tests.rs` Tier 1.5 tests** — `test_v7_time_after_constraint`, `test_v7_block_height_at_least_constraint`, `test_v7_escrow_obligation_confirmed_constraint`. Verify each exercises both violation and success paths on the same task, relying on atomic revert.

5. **Read the `StubEscrow` helper** in `tests.rs` — check that its `GetObligationByTask` response shape is structurally identical to the real escrow's (both deserialise into `junoclaw_common::PaymentObligation`).

6. **Read `@junoclaw/deploy/migrate-tier15.mjs`** — verify the admin pre-flight (`client.getContract(addr)`) hard-fails if sender ≠ contract admin, before any broadcast. Verify the prior `code_id` is preserved in `deployed.json` as `pre_tier15_code_id` for rollback.

7. **Run `cargo test --workspace`** — expect 149 / 149 green.

8. **Run `cargo clippy --workspace --all-targets`** — expect 0 errors; only the pre-existing MSRV-guarded `is_multiple_of` warning in `zk-verifier`.

Each step is a finite artefact with a finite number of lines. A reviewer who does not understand any one step can flag it without needing to load the entire contract set into context.

---

## 6. Security measures (inventory)

Inherited, unchanged:

- **Atomic revert** on any `Err` inside an entry-point — CosmWasm semantics; verified in v5 coherence pass.
- **wasmd-level admin gate** on migrate — standard CosmWasm behaviour.
- **cw2 contract-name check** in migrate entry-point — rejects cross-contract migration.
- **F1 self-complete gate** on `execute_complete` — from v6.
- **F2 `DistributePayment` admin/member gate** on agent-company — from v6.
- **F3 reverse-index uniqueness** in builder-grant — from v6.
- **F4 unexpected-denom rejection** in junoswap-pair — from v6.1.

Added, Tier 1-slim / 1.5:

- **Bounded enum vocabulary** — no user-supplied code path in the trust boundary.
- **Read-only evaluator** — no new state mutation surface.
- **Layered error wrapping** — every failure surfaces with `ConstraintViolated: {pre|post}_hook: hook[i]: VariantName: values` so reverts are diagnostically non-lossy.
- **Additive schema** — `#[serde(default)]` on every new field; pre-v7 records deserialise unchanged.
- **Migrate script admin pre-flight** — local detection of signer mismatch before a burned tx.
- **Rollback preservation** — prior code_id retained in deployed.json.
- **Typed query shims** — `EscrowQuery` / `PoolReservesView` defined in `junoclaw-common` so any schema drift on a queried contract surfaces as a compile-time error rather than a runtime lie.

---

## 7. Risk matrix

Eight risks, rough order of blast radius, with technical and operational mitigations.

| # | Risk | Blast radius | Mitigation (primary) | Mitigation (defence-in-depth) |
|---|------|-------------|----------------------|-------------------------------|
| 1 | Hook evaluator misreads `env.block` — timelock fires early or never | Funds trapped or released early | `test_v7_time_after_constraint`, `test_v7_block_height_at_least_constraint` advance block state explicitly and retry | `env.block` is read fresh at each `execute_complete`; no caching; evaluator signature requires `&Env` |
| 2 | Escrow query schema drift — `EscrowObligationConfirmed` returns wrong answer | Payment invariant broken | `StubEscrow` uses the exact `PaymentObligation` struct from `junoclaw-common`; so does real escrow; drift = compile error | Observation window watches for diagnostic strings that indicate deserialisation failure |
| 3 | Gas exhaustion through long hook list | Task un-completable | Each hook O(1); cost visible at submit time; short list convention | Submitter can cancel a self-wedged task |
| 4 | Submitter griefing via impossible hook | Self-inflicted; no 3rd-party harm | `CancelTask` available to submitter; agent stats unaffected | None needed — submitter pays their own tx fees |
| 5 | Cross-contract query target schema migration | Stale-answer or revert-loop | Coordinated migration policy among contracts sharing an admin | `.map_err(|e| format!(...))?` surfaces the failure loudly |
| 6 | Re-entrancy via Constraint → foreign contract → task-ledger read | Minimal (read-only path) | All variant queries are `query_wasm_smart` (read-only) | CosmWasm query entrypoints cannot mutate state |
| 7 | `proposal_id` vs hook fields disagreeing on uniqueness | Double-consumed proposal | `TASKS_BY_PROPOSAL` remains the sole uniqueness index; checked before hook parsing | v6 proposal-uniqueness regression tests still green |
| 8 | Schema round-trip failure on migrate | Contract bricked | `#[serde(default)]` on every new field; test covers submit-with-no-hooks round-trip | `migrate-tier15.mjs` step 3 queries `GetConfig` post-migrate and asserts v6 fields preserved |

Operational risks (not code-risks, mitigated outside the contract):

- **Wrong admin attempts migrate** — pre-flight check in `migrate-tier15.mjs` catches before broadcast.
- **Partial migration (upload succeeds, migrate tx fails)** — `deployed.json` records `tier15_code_id` even on partial; subsequent run with `SKIP_UPLOAD=true` resumes from migrate step.
- **Agent migration mismatch** — WAVS bridge must be updated to emit `pre_hooks` / `post_hooks` (even as empty arrays) for cosmjs backward-compat; no schema change required given `#[serde(default)]` but agent-level config should be explicit.
- **Observation telemetry coupling to RPC uptime** — see §10.

---

## 8. Migration procedure

### 8.1 Pre-flight

Verify all of:

- [ ] `cargo test --workspace` green (149 / 149).
- [ ] `cargo clippy --workspace --all-targets` — 0 errors.
- [ ] `task_ledger_opt.wasm` exists at `C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger_opt.wasm`, size ~387 KB.
- [ ] `deploy/deployed.json` exists and has `task-ledger.address = juno17aq…` and `task-ledger.code_id = 70`.
- [ ] `wavs/bridge/parliament-state.json` contains an MP entry for `The Builder` with a funded address (recommended ≥ 5 ujunox).
- [ ] `MNEMONIC` env var is unset (forces PARLIAMENT_ROLE path, which is auditable via parliament-state.json).

### 8.2 Build

```powershell
$env:CARGO_TARGET_DIR = 'C:\Temp\junoclaw-wasm-target'
$env:RUSTFLAGS = '-C link-arg=-s'
cargo build --target wasm32-unknown-unknown --release --lib -p task-ledger `
  --manifest-path contracts/Cargo.toml

wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz `
  -o C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger_opt.wasm `
     C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger.wasm
```

Expected output: `task_ledger_opt.wasm` ≈ 387 KB. Sanity-check under the `wasmd` 800 KB upload limit.

### 8.3 Dry run

```powershell
$env:PARLIAMENT_ROLE = 'The Builder'
$env:DRY_RUN = 'true'
node deploy/migrate-tier15.mjs
```

Expected console output:

```
║   JunoClaw Tier 1.5 — task-ledger        ║
║   migration (v6 → hooks)                 ║
** DRY RUN — no transactions will be broadcast **
Target:   task-ledger @ juno17aq…
Current code_id: 70
━━━  Step 1: upload task_ledger_opt.wasm  ━━━
📦  Uploading 386.8 KB…
(dry run: skipping upload)
━━━  Step 2: migrate task-ledger to new code_id  ━━━
(dry run) would send: client.migrate(...)
━━━  Step 3: sanity check  ━━━
(dry run: skipping sanity check)
```

If the admin pre-flight fails (sender ≠ contract admin), the script hard-stops with a clear diagnostic *before* the upload step.

### 8.4 Real migration

```powershell
$env:DRY_RUN = 'false'
node deploy/migrate-tier15.mjs
```

Expected: upload tx broadcast → new code_id recorded → migrate tx broadcast → `GetConfig` responds → `deployed.json` updated with `tier15_code_id`, `tier15_store_tx`, `tier15_migrate_tx`, `tier15_migrated_at`, `pre_tier15_code_id`.

### 8.5 On-chain smoke

```powershell
$env:PARLIAMENT_ROLE = 'The Builder'
$env:STRANGER_ROLE   = 'The Contrarian'
node deploy/smoke-tier15.mjs
```

Three tests: T1 `TimeAfter` (~70s), T2 `BlockHeightAtLeast` (~30s), T3 `EscrowObligationConfirmed` (~20s). Results written to `deploy/smoke-tier15-results.json`. Each test must show `ok: true`.

### 8.6 Post-smoke

- [ ] Append a `## Tier 1.5 (YYYY-MM-DD)` section to `@junoclaw/docs/V6_TESTNET_RUN.md` with: new code_id, migrate tx hash, T1/T2/T3 success tx hashes.
- [ ] Tag the commit as `tier15-uni7-YYYYMMDD`.
- [ ] Open the log section at the bottom of `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md` with the first entry.
- [ ] Update `@junoclaw/docs/MEDIUM_ARTICLE_CONSTRAINTS.md` "What happens next" to reference the concrete tx.

---

## 9. Rollback procedure

Triggered if: smoke-tier15 fails, production behaviour diverges from expected, or an off-chain observation surfaces a Tier 1.5 bug that blocks completion of hook-free tasks.

1. Read `deploy/deployed.json` `task-ledger.pre_tier15_code_id` (should be `70`).
2. Run the migrate call manually against the pre-tier15 code_id:

   ```powershell
   $env:PARLIAMENT_ROLE = 'The Builder'
   # invoke cosmjs manually; no dedicated rollback script exists yet,
   # keep the following as a powershell one-liner using @cosmjs/cli
   # or equivalent:
   #   client.migrate(builder, 'juno17aq…', 70, {}, 'auto', 'JunoClaw Rollback v6.1')
   ```

3. Verify `GetConfig` still responds and the v6 `agent_company` field is populated.
4. Update `deployed.json`: set `task-ledger.code_id` back to `70`; record `rollback_tx`, `rollback_reason`, `rolled_back_at`.
5. Stored `TaskRecord`s with `pre_hooks` / `post_hooks` will deserialise correctly against v6 because v6 simply ignores the extra fields — **but** v6 will not evaluate them, meaning any task with pending hooks becomes functionally equivalent to a task with no hooks. This is the correct behaviour for a rollback: the contract is no longer capable of enforcing Tier 1.5 invariants, so tasks that depended on them should be cancelled and re-submitted against the next Tier 1.5 attempt.

**If a rollback script is needed more often than once:** generalise the above into `@junoclaw/deploy/rollback.mjs`. Do not generalise speculatively.

---

## 10. Observability strategy

### 10.1 Three phases

**Phase 1 (day zero → 2 weeks): tx log grep.**
Dashboard parses `error.message` on failed cosmjs txs for the `ConstraintViolated:` prefix. Counts per day, distribution by variant name, failed-tx volume. No contract change, no event watcher required.

**Phase 2 (weeks 2–4, conditional): add success-path event attributes.**
Add two `.add_attribute` calls to the success-path `Response` in `execute_complete`: `("pre_hooks_count", …)` and `("post_hooks_count", …)`. Requires a re-migrate. Makes the denominator (hook-bearing completions) queryable via standard CosmWasm event indexers. Adoption trigger: if Phase 1 grep produces a tally without a ratio and the ratio is the question that matters.

**Phase 3 (parked, scale-triggered): on-chain storage counters.**
Add a `HOOK_USAGE: Map<String, u64>` in `task-ledger` state; increment per variant inside `evaluate_all` on success; expose via a new `GetHookStats {}` query. Makes observability independent of tx logs, event formats, archive retention, and indexer availability. Adoption trigger: when Phase 1+2 combined no longer meets the telemetry need, typically at ≥ 10,000 hook-bearing txs/day or when multiple agents need programmatic access to the counters (e.g. an agent's self-regulation logic wants to know its own recent hook failure rate).

### 10.2 Why not ship Phase 2 with Tier 1.5

Weighing the cost: adding the two attributes is a one-line change, passes clippy, requires a single test, and adds exactly 0 bytes of state. The argument for shipping it with Tier 1.5 is cleanliness — the migration delta captures the whole observability story. The argument against is audit-minimalism — one more thing in the migration delta is one more thing to read for an external reviewer, and we do not yet have the evidence that the attributes are needed. Current stance: **defer**, re-evaluate at the 2-week check-point.

### 10.3 Why not ship Phase 3 at all yet

Phase 3 is a real state-layer change. It writes to storage on every successful completion; it exposes a new query; it requires its own migration (to introduce the `Map` with a defined empty initial state). At current traffic it is over-engineered. The correct time to ship it is when the tx-log grep infrastructure is provably in the way — probably not before Q3 2026.

### 10.4 Escalation criteria (written in advance)

These are the conditions that escalate Phase 1 → 2 or 2 → 3. Written now so the decision is not made under end-of-quarter pressure.

| Condition | Escalation |
|---|---|
| Week 2 check-in: Phase 1 grep data is ambiguous because the denominator is missing | Ship Phase 2 attributes |
| Any wasmd point-release on Juno that changes `tx_result.log` shape | Ship Phase 2 attributes (immune to log-format drift) |
| Public RPC archive window shortens during the observation period | Ship Phase 2 (events are archived indefinitely by indexers), or stand up our own archive node |
| Agent daemon needs programmatic access to hook-failure rates | Ship Phase 3 storage counters |
| Hook-bearing tx volume exceeds 1,000 / day | Write a Tendermint-level indexer; Phase 2 attributes still sufficient for queries against it |
| Hook-bearing tx volume exceeds 10,000 / day | Ship Phase 3 storage counters; aggregate at the contract |

### 10.5 Lesson from the first smoke run: chain time ≠ wall time

The `uni-7` smoke run on 2026-04-20 exposed a concrete instance of the
log-format-and-timing fragility §10.1 anticipates. The Tendermint block
header's asserted time ran **881 seconds (≈14 min 41 s) behind local
wall-clock** at test time — which is not a bug but a normal property of
public testnets where block production is uneven. Our smoke harness's
original T1 implementation computed `threshold = Date.now()/1000 + 60` and
then `sleep(70s)`, which was wildly insufficient.

**Fix in harness:** `deploy/smoke-tier15.mjs` now calls
`getChainTimeSec(client)` (reads `client.getBlock().header.time`) and
`waitUntilChainTime(client, target, maxWait)` (polls the same) rather than
trusting local time. This is the pattern T2 already used for block height
and should have been used for block time too.

**Fix for dashboards:** any observability layer counting `TimeAfter`
violations on `uni-7` (or any public testnet) **must normalise against
chain-reported block time**. A counter keyed on local wall-clock time will
over-report "failures" that are correct rejections of requests made too
early by local-clock standards. This is a standing requirement for Phase 1
/ Phase 2 / Phase 3 alike, and is therefore added to §12 as a pre-Phase-2
checklist item.

---

## 11. Testing matrix

Which test exercises which risk.

| Test | File | Covers |
|------|------|--------|
| `test_v7_submit_task_stores_hooks_on_record` | `tests.rs` | Schema round-trip through storage |
| `test_v7_complete_evaluates_pre_hook_happy_path` | `tests.rs` | Pre-hook dispatched on complete; pass path |
| `test_v7_pre_hook_violation_reverts_tx` | `tests.rs` | Atomic revert on pre-hook failure — risk #1, #8 |
| `test_v7_post_hook_violation_reverts_tx` | `tests.rs` | Atomic revert on post-hook failure — risk #1 |
| `test_v7_multiple_hooks_first_failure_wins_with_index` | `tests.rs` | Non-lossy diagnostic with `hook[i]:` prefix — risk #5 log-fragility |
| `test_v7_balance_at_least_constraint` | `tests.rs` | `BalanceAtLeast` via bank query |
| `test_v7_pair_reserves_positive_constraint` | `tests.rs` | `PairReservesPositive` via junoswap-pair query |
| `test_v7_task_status_is_constraint_expresses_dependencies` | `tests.rs` | `TaskStatusIs` cross-task dependency |
| `test_v7_time_after_constraint` | `tests.rs` | `TimeAfter` via `env.block.time` — risk #1 (env freshness) |
| `test_v7_block_height_at_least_constraint` | `tests.rs` | `BlockHeightAtLeast` via `env.block.height` — risk #1 (env freshness) |
| `test_v7_escrow_obligation_confirmed_constraint` | `tests.rs` | `EscrowObligationConfirmed` three scenarios — risk #2 (schema drift) |
| `test_agent_trust_at_least_constraint` (pre-existing) | `tests.rs` | `AgentTrustAtLeast` via agent-registry |
| v6 F1 / F2 / F3 / F4 regressions (pre-existing) | `tests.rs`, `smoke-v6.mjs` | v6 invariants still in force |
| T1 / T2 / T3 (smoke) | `smoke-tier15.mjs` | All three Tier 1.5 variants live on uni-7 — **passing against `juno1cp88…` code_id 75** (see `docs/TIER15_TESTNET_RUN.md`) |
| Migration schema round-trip (smoke) | `migrate-tier15.mjs` step 3 | `GetConfig` post-migrate — risk #8. **Not exercised on today's run** because the v6 contract couldn't be migrated; equivalent coverage comes from the Step 3 `UpdateRegistry` read-back in `deploy-tier15-fresh.mjs`. |
| Chain-time vs wall-clock drift | `smoke-tier15.mjs::getChainTimeSec` | Harness-layer coverage of the lesson in §10.5 — any `TimeAfter` hook-bearing test uses `env.block.time` through the chain, not local `Date.now()` |

Pre-flight test originally flagged as *missing* is now in place as `test_v7_constraint_violated_error_string_shape_is_stable` (`contracts/task-ledger/src/tests.rs`). It locks down the full `Constraint violated: pre_hook: hook[0]: VariantName:` prefix shape using `TimeAfter` (pre_hook path) and `BlockHeightAtLeast` (post_hook path) as exemplars — the wrapper layering is variant-agnostic so those two exemplars cover the regression surface for all seven variants. This closes risk #5 and lets a downstream log-grep dashboard sleep.

---

## 12. Deferred / parked items

- **Success-path event attributes on `execute_complete` Response.** Two `.add_attribute` calls. Deferred pending 2-week observation check-in. Re-evaluate then.
- **Error-format regression tests.** ~~Should be added before Phase 2.~~ **Done 2026-04-20** — landed as `test_v7_constraint_violated_error_string_shape_is_stable` covering both `pre_hook:` and `post_hook:` wrappers and the inner `hook[i]: VariantName:` prefix; a Display-layer assertion also locks the outer `Constraint violated:` shape.
- **Chain-time normalisation in observability dashboards** (§10.5). Any `TimeAfter` counter must read chain time via block headers, not local time. **Pre-Phase-2 requirement; standing discipline.**
- **Fix `deploy/deploy.mjs` `client.instantiate` sites** to pass `{ admin: address }` as the 6th argument for every contract. ~~Tracked separately so it lands before the next testnet run.~~ **Done 2026-04-20** — all four instantiate sites in `deploy.mjs` (agent-registry, escrow, task-ledger, agent-company) now set the wasmd admin at instantiation. Existing frozen v6 instances (escrow at `juno17vrh…`, agent-company at `juno1lymt…`) remain frozen on chain; they will be replaced by the next fresh deploy.
- **Storage-layer hook usage counters (`HOOK_USAGE: Map<String, u64>`).** Parked; scale-triggered.
- **New `Constraint` variants** proposed during design but not shipped: `IbcChannelOpen`, `Cw20BalanceAtLeast`, `PriceInRange`, `AgentIsActive`. Re-evaluate after observation window; add only if real agents request them.
- **Reply-based post-callback hooks** (v8). Would let post-hooks observe escrow and registry callback effects. Not in scope for Tier 1.5.
- **Intent-solver architecture / observer-registry** (Tier 2). Explicitly deferred until evidence from the observation window warrants it.
- **Dedicated `rollback.mjs` script.** Not built; rollback is currently a manual migrate call (now actually possible on the new Tier 1.5 task-ledger because it has a wasmd admin). Build once we have rolled back at least once.

---

## 13. Open questions

Questions explicitly left unresolved; observation window should answer them.

1. What is the real hook distribution? Which of the seven variants actually get used?
2. Is the error-string grep approach sufficient? Does the missing success-side denominator actually bite?
3. What gas cost does a typical 2-hook completion add vs a hook-free one?
4. Are there invariants users ask for that the seven-variant vocabulary cannot express?
5. Does the atomic-revert behaviour of a failing post-hook ever surface in a user-visible way that needs UX treatment (e.g. a front-end that assumed `CompleteTask` always succeeds)?

---

## 14. Appendix — current live state snapshot (post-deploy)

Verified against `uni-7` block ≥ 12929508. Full run log in
`@junoclaw/docs/TIER15_TESTNET_RUN.md`.

| Contract | Code ID | Address | Notes |
|----------|--------:|---------|-------|
| `agent-registry` | 69 | `juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep` | reused; `registry.task_ledger` rewired to point at the Tier 1.5 contract |
| `task-ledger` (Tier 1.5) | **75** | `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` | **new, wasmd-admin-enabled** |
| `task-ledger-v6-frozen` | 70 | `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` | retired, no wasmd admin, preserved for audit |
| `escrow` | 71 | `juno17vrh77vjrpvu6v53q94x4vgcrmyw57pajq2vvstn608qvs5hw8kqeew3g9` | reused unchanged (also frozen, non-migratable) |
| `agent-company` | 72 | `juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw` | reused unchanged (also frozen, non-migratable) |
| `builder-grant` | 73 | *(stored only)* | unchanged |
| `junoswap-pair` | 74 | *(stored only)* | unchanged |

### Tier 1.5 deployment transactions (2026-04-20)

| Step | Tx |
|------|----|
| Upload task-ledger Tier 1.5 wasm → code_id 75 | `B7E0D1750FA6CCE0A7D1D6038B382F39F8A8DE7DDFEBC583A1FC6C4CB83290C5` |
| Instantiate new `task-ledger` (with wasmd admin) | `9F247E8BB0D41F2F6F245F2D362FB5E932A9077502D1D1ADF1FC3D2F85E29DE7` |
| `agent-registry.UpdateRegistry` → re-point at new task-ledger | `243B0BD1CFCB3053865ECCFFB376D04A13C89A524D55ADC23462EA019C4A6E78` |

### Smoke-test success txs (2026-04-20)

| Test | Final OK tx |
|------|-------------|
| T1 `TimeAfter` (post chain-time fix) | `C85744602D8CABC4D2D0E4D15FC92237C2C8AA8AA92DD7CDA90A4CAE98C89777` |
| T2 `BlockHeightAtLeast` | `BA62616356324712A0F466ACA04D35BEEF37739367FD4688DD55C9047922921A` |
| T3 `EscrowObligationConfirmed` | `AE47ECB125C9B0561A937DCC33DAEA57E557C9F4DDBAC2B20A3EB0CFF2DE0DD2` |

---

*Last updated: 2026-04-20 (post-deploy, post-smoke).
Next update due: first hook-bearing `CompleteTask` from a live agent, or the 2-week observation check-point, whichever comes first.*
