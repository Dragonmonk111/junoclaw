# JunoClaw v6 — Testnet Run on `uni-7`

**Date:** 2026-04-18
**Chain:** `uni-7` (Juno testnet)
**RPC:** `https://juno-testnet-rpc.polkachu.com`
**Deployer:** `The Builder` — `juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`
**Status:** v6.0 + v6.1 deployed and all on-chain regressions passing.

> TL;DR: Fresh v6 wasms stored and instantiated, F1 / F2 / F3 regressions exercised
> against live `uni-7` state with two parliament wallets (admin + stranger). F4
> (junoswap-pair unexpected-denom rejection) is covered by unit tests; it can't be
> reproduced on live testnet without minting a rogue native denom.

---

## What v6 fixes

`docs/RATTADAN_HARDENING.md` has the full audit; this file records the first
end-to-end testnet exercise that demonstrates the regressions are closed on a
real chain, not just in `cw-multi-test`.

| # | Regression | Where it lived | v6 fix | Testnet proof |
|---|------------|----------------|--------|---------------|
| F1 | Task submitter could self-complete | `task-ledger::execute_complete` permit-check let submitter mark own task `Completed`, which triggers atomic `escrow::Confirm` callback → obligation flips to Paid without funds moving | Gate `CompleteTask` / `FailTask` on admin / operator / `agent-company` only | stranger's `CompleteTask` rejected `Unauthorized`, admin's `CompleteTask` succeeded |
| F2 | Public `DistributePayment` griefing | `PAYMENT_HISTORY` is keyed by `task_id`; any public caller could front-run a pending distribution with 1 ujunox against the same task_id and lock the legitimate call out with `AlreadyDistributed` | Restrict `DistributePayment` to `admin` or DAO member | stranger's `DistributePayment` rejected `Unauthorized`, admin's succeeded |
| F3 | `builder-grant` duplicate `work_hash` accepted | No reverse index on `work_hash`; two submissions with the same output hash + different `evidence` strings both passed verification and could both claim | `WORK_HASH_USED: Map<&str, u64>` written inside `execute_submit` before `SUBMISSIONS.save` so duplicate check and state write can't drift | second submission with same hash rejected with `duplicate work_hash: already submitted as id 1` |
| F4 | `junoswap-pair` ignored unexpected denoms in `info.funds` | `extract_native_amounts` only matched the two pair denoms and discarded the rest, letting a caller over-fund with a rogue denom and walk away with unaccounted excess | Reject any denom in `info.funds` not in the pair | covered by `contracts/junoswap-pair/src/tests.rs` — live reproduction requires minting a rogue native denom on the chain, not feasible on public testnets |

---

## Deployed addresses

Verified against `uni-7` block ≥ `12855882`.

| Contract | Code ID | Address | Store tx | Instantiate tx |
|----------|--------:|---------|----------|----------------|
| `agent-registry` | **69** | `juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep` | `D04EC97…` | `5B8B4D6…` |
| `task-ledger`    | **70** | `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` | `82311E3…` | `7A8D285…` |
| `escrow`         | **71** | `juno17vrh77vjrpvu6v53q94x4vgcrmyw57pajq2vvstn608qvs5hw8kqeew3g9` | `D0F11BA…` | `F545F0A…` |
| `agent-company`  | **72** | `juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw` | `9923A17…` | `654C0BD…` |
| `builder-grant`  | **73** | *(stored only — smoke instantiates fresh each run)* | `D5F8CCE…` | — |
| `junoswap-pair`  | **74** | *(stored only — factory-managed instantiation)* | `94CE4E3…` | — |

Full hashes are in `deploy/deployed.json`. Mintscan: <https://testnet.mintscan.io/juno-testnet>.

### Post-deploy wiring

`agent-registry.UpdateRegistry { task_ledger: <tl>, escrow: <esc> }`
— tx `037446966882EF5E82D3E41E572BEE0DA811EB26EC5FD32BB9B7CC498AF1E2E2`

This is **required** for v6 (and v5). Without it, the `CompleteTask` sub-message
callback into `agent-registry::IncrementTasks` is rejected with `Unauthorized`
and the whole parent tx reverts. Deploy step 6 (`deploy.mjs`) runs it
idempotently.

---

## Smoke-test results

Script: `deploy/smoke-v6.mjs`
Stranger: `The Contrarian` — `juno1n6h88sehc8c5ugvu3crhxlqach2smur9cmaw8n`
Results file: `deploy/smoke-v6-results.json`

### F1 — task-ledger submitter self-complete

```
stranger registers agent          → agent_id 2      tx 66DAE02…
stranger submits task             → task_id 1       tx 713AC18…
stranger CompleteTask(task_id)    → REJECTED Unauthorized  (expected)
admin    CompleteTask(task_id)    → OK              tx C1E8A4C…
```

`Unauthorized` surfaces from `task-ledger::execute_complete` line 257-261.
Admin pass-through exercises the intended settlement flow including the
`IncrementTasks` sub-message now that `agent-registry.registry.task_ledger` is
wired.

### F2 — DistributePayment gate

```
stranger DistributePayment(task_id, 1000ujunox)  → REJECTED Unauthorized
admin    DistributePayment(task_id, 1000ujunox)  → OK  tx 649D38A…
```

`Unauthorized` surfaces from `agent-company::execute_distribute` line 199-201.
Admin is the v6 `cfg.admin`, so the gate lets the call through and the 1,000
ujunox is distributed equal-weighted across the single member (`admin`
himself, weight 10000).

### F3 — builder-grant duplicate work_hash

Fresh `builder-grant` instance: `juno1u9vdgkxkcnwq4cq24utlkd2dv3flrrz2t7taezfg7mz0tghrpgdsmrq4rt`
(code_id 73 → new instantiate `E64907…`)

```
submit_work(ContractDeploy, evidence=tx/ABC123, work_hash=<hex64>)  → OK  tx C682603…
submit_work(ContractDeploy, evidence=tx/DEF456, work_hash=<hex64>)  → REJECTED
    error: "duplicate work_hash: already submitted as id 1"
```

Error string matches `builder-grant::ContractError::DuplicateWorkHash` exactly;
the reverse index `WORK_HASH_USED` is consulted on the second call before any
state write, so the duplicate can neither claim nor grow storage.

### F4 — junoswap-pair unexpected-denom rejection

Not exercised on testnet. The v6.1 fix to `extract_native_amounts` rejects any
`info.funds` entry whose denom isn't one of the pair's two denoms. Reproducing
that on `uni-7` requires sending the pair a native token the wallet doesn't
actually hold, which the bank module blocks before the contract is even
invoked. Unit coverage:

- `contracts/junoswap-pair/src/tests.rs::test_provide_liquidity_rejects_unexpected_denom`
- `contracts/junoswap-pair/src/tests.rs::test_swap_rejects_unexpected_denom`

A `uni-7` reproduction would need either a custom-denom faucet or a
`tokenfactory` mint — out of scope for this run.

---

## Build notes

`rustc 1.94.0` emits post-MVP wasm features (`reference-types`, `sign-ext`)
that `wasmd v0.54` rejects with `Wasm bytecode could not be deserialized.
Deserialization error: "reference-types not enabled: zero byte expected"`.
`RUSTFLAGS="-C target-cpu=mvp -C target-feature=-reference-types,-sign-ext,..."`
did not fully suppress emission.

The working pipeline:

```powershell
# 1. Raw release build with link-time optimisation
$env:CARGO_TARGET_DIR = 'C:\Temp\junoclaw-wasm-target'
$env:RUSTFLAGS = '-C link-arg=-s'
cargo build --target wasm32-unknown-unknown --release --lib `
  -p agent-registry -p task-ledger -p escrow `
  -p agent-company -p builder-grant -p junoswap-pair

# 2. Binaryen wasm-opt post-process (lowers sign-ext to MVP, strips target-features)
foreach ($n in 'agent_registry','task_ledger','escrow','agent_company','builder_grant','junoswap_pair') {
  wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz `
    -o "$env:CARGO_TARGET_DIR\wasm32-unknown-unknown\release\${n}_opt.wasm" `
    "$env:CARGO_TARGET_DIR\wasm32-unknown-unknown\release\$n.wasm"
}
```

`_opt.wasm` sizes (post-lowering):

```
agent_registry   264.3 KB
task_ledger      305.9 KB
escrow           263.5 KB
agent_company    484.4 KB
builder_grant    270.7 KB
junoswap_pair    247.4 KB
```

All are well under `wasmd`'s ~800 KB upload limit.

---

## Reproducing

Prereqs: funded parliament wallet in `wavs/bridge/parliament-state.json`
(gitignored), Node 20+, Rust 1.94, binaryen `wasm-opt`.

```powershell
# Build wasms (see "Build notes" above)

# Deploy + wire
$env:PARLIAMENT_ROLE = 'The Builder'
$env:AUTO_CONFIRM    = 'true'
node deploy/deploy.mjs

# Smoke tests
$env:PARLIAMENT_ROLE = 'The Builder'
$env:STRANGER_ROLE   = 'The Contrarian'
node deploy/smoke-v6.mjs
```

`deploy/deployed.json` records code_ids, addresses, and tx hashes.
`deploy/smoke-v6-results.json` records per-test outcome, the failing-tx error
strings, and the ephemeral builder-grant instance address.

---

## Key files touched for v6.0 / v6.1

- `contracts/task-ledger/src/contract.rs` (F1 gate + regression test)
- `contracts/agent-company/src/contract.rs` (F2 gate + regression test)
- `contracts/builder-grant/src/{contract,state,error}.rs` (F3 reverse index)
- `contracts/junoswap-pair/src/{contract,error,tests}.rs` (F4 unexpected-denom reject)
- `deploy/deploy.mjs` (raw-wasm fallback, parliament wallet loader, step 6 registry wiring)
- `deploy/smoke-v6.mjs` (F1/F2/F3 on-chain harness; new)
- `docs/V6_TESTNET_RUN.md` (this file; new)
