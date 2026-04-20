# JunoClaw Tier 1.5 — Testnet Run on `uni-7`

**Date:** 2026-04-20
**Chain:** `uni-7` (Juno testnet)
**RPC:** `https://juno-testnet-rpc.polkachu.com`
**Deployer / wasmd admin:** `The Builder` — `juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`
**Smoke-test stranger wallet:** `The Contrarian` — `juno1n6h88sehc8c5ugvu3crhxlqach2smur9cmaw8n`
**Status:** Tier 1.5 `task-ledger` deployed and all three on-chain smoke tests passing.

> **TL;DR.** The v6 `task-ledger` at `juno17aq…` was instantiated on 2026-04-18
> without a wasmd migrate admin and is therefore permanently frozen at
> code_id 70. Rather than migrate it, a fresh Tier 1.5 `task-ledger` was
> instantiated at a new address with the migrate admin properly set, the
> `agent-registry` was rewired to point at it, and the three new Constraint
> variants — `TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed` —
> were exercised on chain with two Parliament wallets (admin + stranger).
> The existing `escrow` and `agent-company` contracts were reused untouched.
> One round of iteration was needed on the smoke harness itself to correct a
> local-vs-chain clock assumption; the contract behaved correctly throughout.

---

## What the upgrade adds

`docs/TIER15_ARCHITECTURE_UPGRADE.md` has the full engineering reference;
`docs/MEDIUM_ARTICLE_CONSTRAINTS.md` has the narrative. This file records the
live `uni-7` exercise.

| # | Variant | Intent | Query path |
|---|---------|--------|------------|
| C5 | `TimeAfter { unix_seconds }` | Wall-clock timelock — refuse completion before `env.block.time.seconds() >= unix_seconds` | `env.block.time` |
| C6 | `BlockHeightAtLeast { height }` | Block-count timelock — refuse completion before `env.block.height >= height` | `env.block.height` |
| C7 | `EscrowObligationConfirmed { escrow, task_id }` | Payment invariant — refuse completion unless the obligation for `task_id` on `escrow` is `Confirmed` | `escrow::GetObligationByTask` |

---

## Finding: v6 contracts were not instantiated with wasmd admins

Pre-flight of `deploy/migrate-tier15.mjs` reported `Contract admin on chain:
(none)` for the v6 `task-ledger`. Reviewing `deploy/deploy.mjs:240`:

```js
const res = await client.instantiate(address, codeId, msg, 'JunoClaw Task Ledger', 'auto')
```

The 6th argument to cosmjs's `client.instantiate` (`{ admin: <address> }`)
was omitted, defaulting the wasmd-level migrate authority to `undefined`.
The `admin: address` visible *inside* `msg` is the contract's *internal*
admin (used by `UpdateConfig` / `UpdateRegistry`), not the wasmd authority.

**Consequence.** The v6 `task-ledger`, `escrow`, and `agent-company` at
code_ids 70 / 71 / 72 are permanently frozen — they can receive and process
execute messages, but no wallet can migrate them. `agent-registry` at
code_id 69 is in the same state but is functionally fine for our purposes
because it is only rewired at the message layer via the admin-only
`UpdateRegistry` execute message, not via migration.

**Action taken.** Tier 1.5 was rolled out as a fresh instantiation of the
`task-ledger` only, at a new address, with the wasmd admin properly set
this time. The v6 `task-ledger` at `juno17aq…` was not touched on chain
(the `agent-registry.registry.task_ledger` pointer was reassigned, which
effectively retires the v6 contract as a live target). For audit history
it remains present in `deploy/deployed.json` under the
`task-ledger-v6-frozen` key.

**Follow-up.** Any future code change to `escrow` or `agent-company` will
require the same fresh-instantiation pattern. `deploy/deploy.mjs:240` and
the sibling `client.instantiate` sites for other contracts should receive
the `{ admin: address }` 6th-arg fix before the next deployment session.

---

## Deployed addresses — Tier 1.5

Verified against `uni-7` block ≥ 12929508.

| Contract | Code ID | Address | Notes |
|----------|--------:|---------|-------|
| `agent-registry` | **69** | `juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep` | reused; `registry.task_ledger` pointer updated |
| `task-ledger` (Tier 1.5) | **75** | `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` | **new; wasmd admin = deployer** |
| `escrow` | **71** | `juno17vrh77vjrpvu6v53q94x4vgcrmyw57pajq2vvstn608qvs5hw8kqeew3g9` | reused unchanged |
| `agent-company` | **72** | `juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw` | reused unchanged |
| `task-ledger-v6-frozen` | 70 | `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` | frozen, retired, preserved in `deployed.json` for audit history |

Full hashes are in `deploy/deployed.json` (gitignored).
Mintscan: <https://testnet.mintscan.io/juno-testnet>.

### Deployment transactions

```
Upload task_ledger_opt.wasm (386.8 KB)          → code_id 75
    tx: B7E0D1750FA6CCE0A7D1D6038B382F39F8A8DE7DDFEBC583A1FC6C4CB83290C5

Instantiate new task-ledger at code_id 75       → juno1cp88zj…nehfm
    tx: 9F247E8BB0D41F2F6F245F2D362FB5E932A9077502D1D1ADF1FC3D2F85E29DE7
    instantiate msg: { admin: The_Builder, agent_registry: juno1568…, operators: [], agent_company: null }
    wasmd admin arg (6th):   { admin: The_Builder }   ← the fix

agent-registry.UpdateRegistry { task_ledger: juno1cp88…, escrow: juno17vrh… }
    tx: 243B0BD1CFCB3053865ECCFFB376D04A13C89A524D55ADC23462EA019C4A6E78
    post-condition verified via GetConfig: registry.task_ledger == juno1cp88…
```

### Wasmd admin read-back

Queried `client.getContract(juno1cp88…)` immediately after instantiation:

```
address: juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm
codeId:  75
admin:   juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z   ← The Builder
label:   JunoClaw Task Ledger Tier 1.5
```

---

## Smoke-test results

Script: `deploy/smoke-tier15.mjs`
Results file: `deploy/smoke-tier15-results.json` (gitignored)

### T1 — `TimeAfter`

First run failed at the late-ok check; second run (post-patch) passed. See
"Finding: clock drift" below for the diagnosis. Final result on the
chain-time-aware retry:

```
stranger registers agent                    → agent_id 6   tx FBFDE1DB16…
stranger submits task with TimeAfter hook   → task_id 4    tx B7BBB02084…
admin CompleteTask before threshold         → REJECTED
    Constraint violated: pre_hook: hook[0]: TimeAfter: block time <chain_t_0> < required <chain_t_0 + 60>
waitUntilChainTime(threshold)               → crossed after ~60s of chain time
admin CompleteTask after threshold          → OK          tx C85744602D8CABC4D2D0E4D15FC92237C2C8AA8AA92DD7CDA90A4CAE98C89777
```

Diagnostic path is in `junoclaw-common::Constraint::TimeAfter::evaluate` via
`env.block.time.seconds()` vs the stored `unix_seconds` literal; revert
wrapper is in `task-ledger::execute_complete` at `pre_hook:` prefix.

### T2 — `BlockHeightAtLeast`

Passed on first run.

```
stranger registers agent                    → agent_id 4   tx 12F8E24B4A…
stranger submits task with BlockHeight hook → task_id 2    tx E7A6F1A604…
admin CompleteTask before threshold (12929504)
                                            → REJECTED
    Constraint violated: pre_hook: hook[0]: BlockHeightAtLeast: block height 12929504 < required 12929508
waitUntilHeight(12929508)                   → crossed after 4 blocks (~20s)
admin CompleteTask after threshold          → OK          tx BA62616356324712A0F466ACA04D35BEEF37739367FD4688DD55C9047922921A
```

Diagnostic path is in `junoclaw-common::Constraint::BlockHeightAtLeast::evaluate`
via `env.block.height`.

### T3 — `EscrowObligationConfirmed`

Passed on first run. Exercises cross-contract coherence between a v6 escrow
(non-migratable, unchanged) and a Tier 1.5 task-ledger — confirming the
composition is correct.

```
stranger registers agent                    → agent_id 5   tx 789BBC794B…
admin Authorize escrow obligation           → OK          tx 86F40901DE55EE3072275662A094A46B7849FA804E53EEB846A90F46C50FFE45
    escrow task_id slot: 716000782  payee: The Contrarian  amount: 1ujunox
stranger submits task with Escrow hook      → ledger_task_id 3  tx 8D052ECD2A…
admin CompleteTask while escrow is Pending  → REJECTED
    Constraint violated: pre_hook: hook[0]: EscrowObligationConfirmed: task 716000782 obligation is Pending, expected Confirmed
admin Confirm escrow obligation             → OK          tx 982154146BC9808B0E39C972C1FA7E20E701213509B79AC24FD0F6EC3D0EF976
admin CompleteTask after Confirmed          → OK          tx AE47ECB125C9B0561A937DCC33DAEA57E557C9F4DDBAC2B20A3EB0CFF2DE0DD2
```

Diagnostic path is in `junoclaw-common::Constraint::EscrowObligationConfirmed::evaluate`
via `WasmQuery::Smart { contract_addr: escrow, msg: GetObligationByTask { task_id } }`,
deserialising into `junoclaw_common::PaymentObligation` and matching on
`ObligationStatus::Confirmed`.

### Summary

```
T1  TimeAfter                   ✅  (after patch; see clock-drift finding)
T2  BlockHeightAtLeast          ✅
T3  EscrowObligationConfirmed   ✅
```

---

## Finding: local-vs-chain clock drift on `uni-7`

### Symptom

T1's first run rejected both the early and the late completion attempt. The
second failure — the unexpected one — reported:

```
Constraint violated: pre_hook: hook[0]: TimeAfter:
    block time 1776715017 < required 1776715958
```

The threshold (`1776715958`) had been computed as `Math.floor(Date.now() / 1000) + 60`.
The chain reported a block time of `1776715017`, **881 seconds** (≈14 min 41 s)
behind the local wall clock at the same instant.

### Root cause

The original T1 threshold and the post-attempt sleep both assumed local
wall-clock time tracks `env.block.time` in lockstep. They do not. Tendermint
block headers carry the time asserted by the block proposer, which on a
public testnet can drift minutes behind NTP when block production is slow
or block-proposer clocks are out of sync. `uni-7` specifically is known to
run behind.

### Fix

`deploy/smoke-tier15.mjs` was patched to add two helpers:

- `getChainTimeSec(client)` — reads `client.getBlock().header.time` and
  converts to seconds-since-epoch.
- `waitUntilChainTime(client, targetSec, maxWaitSec)` — polls `getChainTimeSec`
  until it crosses `targetSec` or hits the `maxWaitSec` budget.

T1 now computes its threshold as `chainNow + TIME_AFTER_OFFSET_SEC` rather
than `Date.now()/1000 + TIME_AFTER_OFFSET_SEC`, and waits via
`waitUntilChainTime` rather than `sleep`. The re-run completed successfully
in ~60 seconds of chain time.

### Implication for observability

Any dashboard that watches `TimeAfter` hook violations on `uni-7` must
normalise against chain-reported block time. A counter that uses local
wall-clock will over-report "failures" that are actually correct rejections
of requests made too early by local-clock standards. This is one concrete
instance of the fragility the observability strategy in
`docs/TIER15_ARCHITECTURE_UPGRADE.md` §10 anticipates; the mitigation is
the same for a dashboard as it was for the smoke harness — read chain time
from the block header, not from the wall.

### Implication for contract correctness

None. The contract evaluator read the correct value (`env.block.time.seconds()`),
compared it correctly (`>= *unix_seconds`), and produced a precise,
non-lossy diagnostic. The `pre_hook: hook[0]: TimeAfter: block time X < required Y`
prefix surfaced the exact values being compared, which is why the clock-drift
bug was diagnosable in minutes rather than hours. That is the design
intent of the layered error wrapping; it was validated here by the first
real unexpected behaviour it encountered.

---

## What stays untouched

- **Contracts at code_ids 69 / 71 / 72 / 73 / 74** — `agent-registry`,
  `escrow`, `agent-company`, `builder-grant`, `junoswap-pair` are all reused
  at their v6.1 code_ids and addresses. Tier 1.5 required no changes to any
  of them.
- **v6 testnet regressions F1 / F2 / F3 / F4** — `agent-registry` is the
  only v6 contract whose on-chain state was modified (`UpdateRegistry`),
  and that modification leaves every v6 invariant in force. The v6 smoke
  harness `deploy/smoke-v6.mjs` continues to exercise F1 / F2 / F3 on the
  new `task-ledger` with the same outcomes (tested off-band).
- **Workspace regression suite** — 149 / 149 passing (up from 148 after the
  same-day addition of `test_v7_constraint_violated_error_string_shape_is_stable`),
  28 of which exercise the Tier 1.5 surface on `task-ledger`.

---

## Reproducing

Prereqs: funded parliament wallet in `wavs/bridge/parliament-state.json`
(gitignored), Node 20+, Rust 1.84+, binaryen `wasm-opt`.

```powershell
# Build wasms (see docs/V6_TESTNET_RUN.md §Build notes for full pipeline)
$env:CARGO_TARGET_DIR = 'C:\Temp\junoclaw-wasm-target'
$env:RUSTFLAGS        = '-C link-arg=-s'
cargo build --target wasm32-unknown-unknown --release --lib -p task-ledger `
  --manifest-path contracts/Cargo.toml
wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz `
  -o  C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger_opt.wasm `
      C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release\task_ledger.wasm

# Fresh-deploy Tier 1.5 task-ledger and rewire agent-registry
$env:PARLIAMENT_ROLE = 'The Builder'
$env:DRY_RUN         = 'true'      # dry-run first — prints planned actions only
node deploy/deploy-tier15-fresh.mjs
$env:DRY_RUN         = 'false'
node deploy/deploy-tier15-fresh.mjs

# Smoke test the three Tier 1.5 variants on chain
$env:PARLIAMENT_ROLE = 'The Builder'
$env:STRANGER_ROLE   = 'The Contrarian'
node deploy/smoke-tier15.mjs
```

Opt-out flags:

- `SKIP_UPLOAD=true` on `deploy-tier15-fresh.mjs` — reuse the
  `tier15_code_id` from a prior run rather than re-uploading.
- `SMOKE_ONLY=T1,T3` on `smoke-tier15.mjs` — run only the named tests
  (comma-separated). Useful for re-running one test after a harness patch
  without burning tx fees on the others.

`deploy/deployed.json` records code_ids, addresses, and tx hashes.
`deploy/smoke-tier15-results.json` records per-test outcome, the failing-tx
error strings, and the tx hashes on the success path.

---

## Key files touched for Tier 1.5

- `contracts/junoclaw-common/src/lib.rs` — 3 new `Constraint` variants, `&Env` threaded through `evaluate` / `evaluate_all`, typed query shims for `EscrowQuery` / `PoolReservesView`
- `contracts/task-ledger/src/{msg,contract,error,tests}.rs` — `pre_hooks` / `post_hooks` plumbing on `ExecuteMsg::SubmitTask`, pre-then-post evaluation inside `execute_complete`, `ConstraintViolated` error variant, 3 new integration tests, `StubEscrow` helper contract
- `deploy/deploy-tier15-fresh.mjs` — new fresh-deploy script (used today)
- `deploy/migrate-tier15.mjs` — migration script (not used today because the v6 contract has no admin; retained for future Tier 1.5→Tier 2+ migrations)
- `deploy/smoke-tier15.mjs` — on-chain Tier 1.5 smoke harness; patched to use chain-reported time rather than local wall-clock
- `docs/TIER15_ARCHITECTURE_UPGRADE.md` — engineering reference
- `docs/MEDIUM_ARTICLE_CONSTRAINTS.md` — narrative Medium piece on the primitive
- `docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md` — consolidated narrative covering today end-to-end
- `docs/TIER1_SLIM_OBSERVATION.md` — field-operations tracker for the observation window
- `docs/TIER15_TESTNET_RUN.md` — this file
