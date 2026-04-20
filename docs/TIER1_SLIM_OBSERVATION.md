# Tier 1-slim / Tier 1.5 — Field Observation Tracker

**Status:** Deployed to `uni-7` on 2026-04-20 as a fresh contract at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` (code_id 75). All three Tier 1.5 on-chain smoke tests passing.
**Window:** 4–8 weeks starting 2026-04-20.
**Owner:** Whoever is on-call for the agent stack.
**Companion docs:** `@junoclaw/docs/TIER15_TESTNET_RUN.md` (audit record), `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md` (engineering reference), `@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md` (narrative).

---

## What shipped

### Code

- `contracts/junoclaw-common/src/lib.rs` — `Constraint` enum (7 variants), `TaskRecord.{pre_hooks,post_hooks}`, `evaluate` / `evaluate_all`.
- `contracts/task-ledger/src/{msg,contract,error}.rs` — `ExecuteMsg::SubmitTask.{pre_hooks,post_hooks}`, `execute_complete` evaluates hooks, `ContractError::ConstraintViolated`.
- `contracts/task-ledger/src/tests.rs` — 11 hook-related regressions (8 Tier 1-slim + 3 Tier 1.5).

### Variants in scope

| Variant | Ships in | Query target |
|---|---|---|
| `AgentTrustAtLeast` | 1-slim | `agent-registry::GetAgent` |
| `BalanceAtLeast` | 1-slim | bank module |
| `PairReservesPositive` | 1-slim | junoswap-pair `Pool {}` |
| `TaskStatusIs` | 1-slim | task-ledger (self or cross) `GetTask` |
| `TimeAfter` | 1.5 | `env.block.time` |
| `BlockHeightAtLeast` | 1.5 | `env.block.height` |
| `EscrowObligationConfirmed` | 1.5 | `escrow::GetObligationByTask` |

---

## Deployment checklist (uni-7)

### Build (local)

```powershell
$env:CARGO_TARGET_DIR = 'C:\Temp\junoclaw-wasm-target'
$env:RUSTFLAGS = '-C link-arg=-s'
cargo build --target wasm32-unknown-unknown --release --lib -p task-ledger `
  --manifest-path contracts/Cargo.toml
wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz `
  -o C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger_opt.wasm `
     C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger.wasm
```

Expected size: ~387 KB (v6 was 306 KB; +81 KB for 3 new Constraint variants + `#[serde(default)]` plumbing). Well under `wasmd` ~800 KB limit.

### Migrate the live contract (**not used today — see fresh-deploy path below**)

The original plan was `migrate-tier15.mjs` against the v6 `task-ledger` at
`juno17aq…`. That proved impossible because the v6 contract was
instantiated without a wasmd-level admin. The migrate script remains in
the repo and will work against the new Tier 1.5 contract at
`juno1cp88…` — which *does* have a wasmd admin — for future Tier 2+
upgrades.

```powershell
$env:PARLIAMENT_ROLE = 'The Builder'
node deploy/migrate-tier15.mjs
```

Supports `DRY_RUN=true` and `SKIP_UPLOAD=true`.

### Fresh-deploy (what was actually used, 2026-04-20)

```powershell
$env:PARLIAMENT_ROLE = 'The Builder'
$env:DRY_RUN         = 'true'   # dry-run first
node deploy/deploy-tier15-fresh.mjs
$env:DRY_RUN         = 'false'
node deploy/deploy-tier15-fresh.mjs
```

What it does:

1. Uploads `task_ledger_opt.wasm` → fresh code_id.
2. Instantiates a new `task-ledger` **with** `{ admin: sender }` as the 6th
   arg to `client.instantiate` (the line the v6 `deploy.mjs` omitted).
3. Calls `agent-registry.UpdateRegistry { task_ledger: <new>, escrow: <existing> }`
   so CompleteTask sub-messages route correctly.
4. Preserves the v6 record as `task-ledger-v6-frozen` in `deploy/deployed.json`.
5. Verifies the wasmd admin read-back on chain before returning success.

Supports `DRY_RUN=true` and `SKIP_UPLOAD=true`.

### On-chain smoke

```powershell
$env:PARLIAMENT_ROLE = 'The Builder'
$env:STRANGER_ROLE   = 'The Contrarian'
node deploy/smoke-tier15.mjs
```

Three test cases (results recorded to `deploy/smoke-tier15-results.json`):

| # | Hook | Flow |
|---|---|---|
| **T1** | `TimeAfter { unix_seconds }` | submit with threshold = **chain_time** + 60s; admin CompleteTask → reject with `TimeAfter`; poll `waitUntilChainTime(threshold)`; retry → ok |
| **T2** | `BlockHeightAtLeast { height }` | submit with threshold = current_height + 6; CompleteTask → reject with `BlockHeightAtLeast`; `waitUntilHeight(threshold)`; retry → ok |
| **T3** | `EscrowObligationConfirmed { escrow, task_id }` | admin Authorize escrow (Pending); submit ledger task; CompleteTask → reject with `expected Confirmed`; admin Confirm escrow; retry → ok |

Each test exercises both the violation path and the success path on the **same** task, relying on atomic revert to leave the task Running after a failed hook. Supports `SMOKE_ONLY='T1'` / `SMOKE_ONLY='T1,T3'` env flag to skip already-passing tests during harness iteration.

**Warning:** T1 thresholds must be computed against **chain-reported time**, not local wall-clock. The first run of T1 on 2026-04-20 failed with an 881-second chain-vs-local drift; see the deployment log below and `@junoclaw/docs/TIER15_TESTNET_RUN.md` "Finding: local-vs-chain clock drift on uni-7" for the full analysis.

### Post-smoke (done 2026-04-20)

- [x] Tier 1.5 deployment recorded in `@junoclaw/docs/TIER15_TESTNET_RUN.md` (new canonical audit doc; V6_TESTNET_RUN.md is the v6 baseline).
- [ ] Tag the repo commit as `tier15-uni7-20260420` so later log archaeology can correlate tx hashes to code state.

---

## Instrumentation

Before migrating live agents, ensure these are in place:

- [ ] `wavs/bridge` logs attach `pre_hooks.length` and `post_hooks.length` to every `SubmitTask` it emits.
- [ ] Agent daemon surfaces hook evaluation outcomes in its chat / updates panel so humans see them.
- [ ] Dashboards parse **tx failure logs** for `ConstraintViolated` — CosmWasm discards `Response` events when an entry point returns `Err`, so there is no `wasm-constraint_violated` event to listen for. The diagnostic lives only in the tx's error string (e.g. `hook[0]: pre_hook: TimeAfter: block time 1745190000 < required 1745200000`). Grep for `ConstraintViolated` in RPC `tx_result.log` or in cosmjs's `error.message`.
- [ ] *Optional follow-up:* add `.add_attribute("pre_hooks_count", …)` / `("post_hooks_count", …)` to the **success-path** `Response` in `task-ledger::execute_complete` so successful hook-bearing completions are countable via `wasm-*` event attributes. This is a one-line additive change, zero trust-boundary delta, and closes the "how many hooks shipped to production" telemetry question without needing to pull `GetTask` for every completion. Defer to post-observation if field data shows the grep path suffices.

---

## What to watch (priority order)

### Signal-bearing metrics

1. **Which variants are used at all.** Count of submissions-with-hooks by variant. A variant with zero usage over 8 weeks is a candidate for removal.
2. **Which variants fire the violation path.** Count of `ConstraintViolated` reverts per variant. A variant that never violates is either unnecessary (the real world already satisfies it) or redundant with a predecessor check. A variant that violates frequently is catching something — its audit weight is earned.
3. **Latency added to `execute_complete`.** Hook evaluation is cross-contract query cost. Measure gas delta for typical hook-bearing completions vs hook-free. Flag if > 10× baseline.
4. **Error diagnostic usefulness.** When violations fire, how often does the operator understand the revert reason without off-chain diagnosis? The `hook[i]:` index prefix and human-readable error string are the primary UX surface — collect examples of confusing ones.

### Anti-signals (things that should NOT appear)

- Any completion that silently skips a hook. The invariant is: if a task has `!pre_hooks.is_empty()` or `!post_hooks.is_empty()`, `ConstraintViolated` or successful evaluation must be observable. A tx that completes a hook-bearing task without either event is a bug.
- Any case where a post-hook failure leaves partial state (Completed status + un-fired escrow callback). The atomic-revert regression test covers the cw-multi-test model; the real chain should behave identically but watch for it.
- Schema drift: a pre-v7 stored `TaskRecord` that fails to deserialise after migrate. If it happens, freeze migration and investigate the serde default chain.

---

## Decision criteria for Tier 2

At the end of the window, answer these questions. A **yes** to any of 1–3 AND a **no** to all of 4–6 is the argument for proceeding to Tier 2 (some form of intent-ledger or observer-registry). Otherwise, stay at Tier 1.x and iterate on variants.

1. Has a real agent asked for a task-shape that *cannot* be expressed as a `Vec<Constraint>`?
2. Are multiple solvers / operators competing to fulfil the same logical piece of work, and has that competition produced actual behaviour differences on-chain?
3. Are any users managing hook-bearing tasks as a DAG large enough (> 5 tasks) that `TaskStatusIs` chains are becoming painful to author?
4. Is the set of 7 variants covering > 90% of submissions?
5. Are the gas costs of constraint evaluation acceptable for the current traffic profile?
6. Is the error diagnostic giving operators the signal they need?

---

## Pending additions (parked, not blocking)

Variants that have been discussed but not added. Re-evaluate at end of window based on evidence:

- `IbcChannelOpen { channel_id }` — requires Stargate query; evidence that cross-chain tasks are a real use-case.
- `Cw20BalanceAtLeast { token, who, amount }` — requires cw20 crate; adds a dev-dependency. Defer until a cw20-denominated task appears.
- `PriceInRange { pair, min, max }` — a richer pair query; likely needs a new `SpotPrice {}` query on junoswap-pair first.
- `AgentIsActive { agent_id }` — boolean specialisation of `AgentTrustAtLeast`. Trivial to add if demand appears.

---

## Log

(Append observations here, newest first.)

### 2026-04-20 — Tier 1.5 deployed, all three smoke tests passing

**Deployment.** Fresh-deploy via `deploy-tier15-fresh.mjs` (migrate blocked by missing wasmd admin on v6 contract):

- Upload task_ledger_opt.wasm (386.8 KB) → code_id **75**. Tx `B7E0D1750FA6CCE0A7D1D6038B382F39F8A8DE7DDFEBC583A1FC6C4CB83290C5`.
- Instantiate new `task-ledger` at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` with wasmd admin = `The Builder`. Tx `9F247E8BB0D41F2F6F245F2D362FB5E932A9077502D1D1ADF1FC3D2F85E29DE7`.
- `agent-registry.UpdateRegistry { task_ledger: juno1cp88…, escrow: juno17vrh… }`. Tx `243B0BD1CFCB3053865ECCFFB376D04A13C89A524D55ADC23462EA019C4A6E78`.
- Read-back verified: `registry.task_ledger == juno1cp88…`.
- Old v6 `task-ledger` at `juno17aq…` (code_id 70) preserved as `task-ledger-v6-frozen` in `deployed.json`; it is frozen on chain (no wasmd admin) and retired from active use.
- `escrow` at `juno17vrh…` (code_id 71) and `agent-company` at `juno1lymt…` (code_id 72) reused unchanged.

**Smoke.** All three passed (T1 on second attempt, after harness fix).

- **T2 BlockHeightAtLeast** — passed on first attempt. Rejected at height 12929504 with `Constraint violated: pre_hook: hook[0]: BlockHeightAtLeast: block height 12929504 < required 12929508`. Success tx `BA62616356324712A0F466ACA04D35BEEF37739367FD4688DD55C9047922921A`.
- **T3 EscrowObligationConfirmed** — passed on first attempt. Rejected with `task 716000782 obligation is Pending, expected Confirmed`. Confirm tx `982154146BC9808B0E39C972C1FA7E20E701213509B79AC24FD0F6EC3D0EF976`. Success tx `AE47ECB125C9B0561A937DCC33DAEA57E557C9F4DDBAC2B20A3EB0CFF2DE0DD2`. Demonstrated a v6 escrow + Tier 1.5 task-ledger compose cleanly.
- **T1 TimeAfter** — failed on first attempt; passed on second. Chain block time was 881 seconds (≈15 minutes) behind local wall-clock at test time; 70-second local sleep was not enough to cross the threshold. Patched `smoke-tier15.mjs` to compute threshold against chain-reported time (`getChainTimeSec` reads `client.getBlock().header.time`) and to poll chain time for threshold crossing (`waitUntilChainTime`). Re-ran T1 only via `SMOKE_ONLY=T1`; success tx `C85744602D8CABC4D2D0E4D15FC92237C2C8AA8AA92DD7CDA90A4CAE98C89777`.

**Contract behaviour note.** The evaluator was correct throughout. The T1 failure was a test-harness assumption, not a contract bug. The `Constraint violated: pre_hook: hook[0]: TimeAfter: block time X < required Y` diagnostic surfaced the exact values being compared, which is why diagnosis took minutes rather than hours.

**Observability implication.** Any dashboard that counts `TimeAfter` violations on `uni-7` (or any public testnet) must normalise against chain-reported block time, not local wall-clock. This is now written up in `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md` §10.5 as a standing requirement and added to §12 as a pre-Phase-2 observability checklist item.

**Follow-up backlog surfaced today.**

- [x] Fix `deploy/deploy.mjs` `client.instantiate` sites to pass `{ admin: address }` as the 6th arg for every contract. Done same-day across agent-registry / escrow / task-ledger / agent-company instantiate sites (commit follows). The frozen v6 `escrow` at `juno17vrh…` and `agent-company` at `juno1lymt…` will be addressed by the next fresh-deploy pass, which will now correctly set admins.
- [x] Add error-string regression test asserting the full `Constraint violated: pre_hook: hook[0]: VariantName: ...` shape. Done same-day as `test_v7_constraint_violated_error_string_shape_is_stable` in `@junoclaw/contracts/task-ledger/src/tests.rs` — covers the prefix layering using `TimeAfter` (pre_hook path) and `BlockHeightAtLeast` (post_hook path) as exemplars; the wrapper layering is variant-agnostic so those two exemplars lock down the shape for all seven variants. Pre-Phase-2 requirement satisfied.
- [ ] Tag the repo commit `tier15-uni7-20260420`.
