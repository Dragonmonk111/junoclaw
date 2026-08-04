# Track B — Deterministic Implementation Plan

**Goal:** Forward-port BN254 precompile host functions to wasmvm v3.0.4 / cosmwasm v3.0.6, build a patched `junod` binary, and deploy via chain upgrade so that `env.bn254_add`, `env.bn254_scalar_mul`, and `env.bn254_pairing_equality` are available as CosmWasm host imports on Juno mainnet.

**Status as of 2026-08-04:** ~90% of patch work complete. Remaining work is finalization, build integration, and chain upgrade.

**Publication shape: P2 (patch series only, no public fork).** Decision made 2026-08-04. Upstream CosmWasm#2685 is deferred to Backlog (no PR accepted until ~Q3/Q4 2026). Maintaining a public fork for 3-6 months with no upstream merge path is unnecessary burden. Instead, the 10 patches are applied at build time by Juno's build script. Zero fork maintenance. Patches rebase automatically on `cargo update`.

**Prerequisite evidence:** Mainnet v30 `junod` binary confirmed missing BN254 imports — `wasm store` of precompile wasm rejected with `Wasm contract requires unsupported import: "env.bn254_add"`. Available imports list includes only `bls12_381_*`, `secp256k1_*`, `secp256r1_*`, `ed25519_*`.

---

## Phase 1: Finalize Patches Against v3.0.6 (est. 1-2 hours)

### 1.1 Regenerate patches 01, 04, 08 against cosmwasm v3.0.6

- [ ] Run `check-baseline-v3.ps1 -CosmwasmTag v3.0.6` to confirm current state (expect 7 clean / 3 3-way-OK)
- [ ] Re-anchor `01-cosmwasm-std.imports.rs.patch` against v3.0.6 (path: `packages/std/src/exports/imports.rs`; line shifts from v3.0.1 → v3.0.6 expected to be minimal — `imports.rs` and `memory.rs` are byte-identical between v3.0.1 and v3.0.5)
- [ ] Regenerate `04-cosmwasm-std.Cargo.toml.patch` via `--3way` merge against v3.0.6
- [ ] Regenerate `08-cosmwasm-vm.Cargo.toml.patch` via `--3way` merge against v3.0.6
- [ ] Verify: `check-baseline-v3.ps1 -CosmwasmTag v3.0.6` returns **10/10 CLEAN**

### 1.2 Full test suite against v3.0.6

- [ ] Run `apply-and-test-v3.ps1 -CosmwasmTag v3.0.6`
- [ ] Confirm `cargo test -p cosmwasm-crypto-bn254 --no-default-features` → **22/22 pass**
- [ ] Confirm `cargo test -p cosmwasm-vm` → **315/316 pass** (1 pre-existing float test failure on Windows/Rust 1.82, unrelated to BN254)
- [ ] Capture logs to `wasmvm-fork/patches/logs/v3.0.6-cargo-test-{crypto-bn254,vm}.log`

### 1.3 Update patch manifest

- [ ] Update `wasmvm-fork/patches/v3.0.x/README.md` with v3.0.6 re-baseline results
- [ ] Update `FORWARD_PORT_V3.md` — mark Day 3 tasks complete

**Exit criteria:** 10/10 patches clean against v3.0.6, all tests pass, manifest updated.

---

## Phase 2: Build Integration Script (est. 1 hour)

**Approach: P2 — patches applied at build time, no public fork.**

### 2.1 Create build script

- [x] Write `wasmvm-fork/patches/build-wasmvm-bn254-v3.sh` that:
  1. Clones `CosmWasm/wasmvm` at tag `v3.0.4`
  2. Clones `CosmWasm/cosmwasm` at tag `v3.0.6` (what wasmvm v3.0.4 resolves to)
  3. Applies the 10 patches from `wasmvm-fork/patches/v3.0.x/` to the cosmwasm checkout
  4. Adds a `[patch.crates-io]` section to `libwasmvm/Cargo.toml` pointing to the local patched cosmwasm
  5. Builds `cargo +1.82 build --release` in `libwasmvm/`
  6. Verifies BN254 symbols in the resulting `libwasmvm.x86_64.so`
  7. Copies the `.so` to a configurable output path

### 2.2 Verify BN254 symbols

- [ ] Run the build script
- [ ] Verify symbols:
  ```bash
  nm -D libwasmvm.x86_64.so | grep bn254
  ```
  Expected: `cosmwasm_vm::imports::do_bn254_add`, `do_bn254_scalar_mul`, `do_bn254_pairing_equality`

**Exit criteria:** Build script produces a `libwasmvm.x86_64.so` with BN254 symbols. No public fork needed. Script is idempotent and reproducible.

---

## Phase 3: Build Patched junod (est. 2-3 hours)

### 3.1 Clone Juno chain repo and swap libwasmvm

- [ ] Clone `CosmosContracts/juno` at the v30 tag (or latest v30 branch)
- [ ] Build wasmvm with BN254 patches using the Phase 2 script → produces `libwasmvm.x86_64.so`
- [ ] Replace the stock `libwasmvm.x86_64.so` in the Juno build with our patched version:
  ```bash
  # Option A: Pre-build .so swap (simplest)
  cp /path/to/patched/libwasmvm.x86_64.so /usr/local/lib/
  ldconfig
  # Then build junod normally — it links against the .so in /usr/local/lib/
  make build
  ```
- [ ] Alternative Option B: Local Go replace directive (for devnet testing):
  ```
  replace github.com/CosmWasm/wasmvm/v3 => /path/to/local/wasmvm-v3.0.4-bn254
  ```
  Then `go mod tidy && make build`

### 3.2 Verify BN254 in the binary

- [ ] Check libwasmvm version: `./build/junod query wasm libwasmvm-version`
  - Expected: still reports `3.0.4` (our fork is based on v3.0.4)
- [ ] Check capabilities: `strings build/junod | grep bn254`
  - Expected: `bn254` capability string present
- [ ] Check host imports: store the precompile wasm on a **local devnet**:
  ```bash
  junod tx wasm store zk_verifier_precompile.wasm --from validator --keyring-backend test --chain-id <devnet-chain-id> --node http://localhost:26657 --gas auto --gas-adjustment 1.5 --gas-prices 0.075ustake --yes
  ```
  - Expected: **success** (no "unsupported import" error)

### 3.3 Gas benchmark on devnet

- [ ] Instantiate both pure and precompile variants
- [ ] Execute `VerifyProof` on both
- [ ] Measure gas: expect ~370k (pure) → ~203k (precompile), 1.823× reduction
- [ ] Compare with Phase 2.3 benchmark results from `BN254_BENCHMARK_RESULTS.md`

**Exit criteria:** Patched `junod` stores and runs the precompile wasm. Gas reduction matches prior benchmarks (±5%).

---

## Phase 4: Chain Upgrade Proposal (est. 1-2 days for governance, 1 hour for prep)

### 4.1 Coordinate with Jake / Dimi

- [ ] DM Jake: share the build script, patch directory, and devnet benchmark results
- [ ] Confirm upgrade version (v30.1 patch or v31)
- [ ] Agree on upgrade height

### 4.2 Submit governance proposal

- [ ] Prepare upgrade proposal JSON:
  ```json
  {
    "messages": [
      {
        "@type": "/cosmos.upgrade.v1beta1.MsgSoftwareUpgrade",
        "plan": {
          "name": "v30.1-bn254",
          "height": <agreed-height>,
          "info": "BN254 precompile integration. Build script applies 10 patches to cosmwasm v3.0.6, rebuilds libwasmvm.x86_64.so, swaps into v30 binary. See proposal text for details."
        }
      }
    ],
    "deposit": "100000000ujuno",
    "title": "v30.1 BN254 Precompile Patch",
    "summary": "Adds BN254 host functions (ecadd, scalar_mul, pairing_equality) to wasmvm v3.0.4 via build-time patch application. Reduces ZK proof verification gas from ~370k to ~203k (1.82×). Patches apply cleanly to cosmwasm v3.0.6 (10/10, 22/22 tests pass). No consensus changes — additive host-function surface only."
  }
  ```
- [ ] Submit: `junod tx gov submit-proposal upgrade.json --from "my validator wallet" --keyring-backend os --chain-id juno-1 --node <rpc> --gas auto --gas-prices 0.075ujuno --yes`
- [ ] Vote YES with validator: `junod tx gov vote <proposal-id> yes --from "my validator wallet" --keyring-backend os --chain-id juno-1 --node <rpc> --yes`

### 4.3 Validator upgrade execution

- [ ] Before upgrade height: stop `junod` service
- [ ] Replace binary: `cp build/junod /usr/local/bin/junod` (or via cosmovisor: place in `~/.juno/cosmovisor/upgrades/v30.1-bn254/bin/`)
- [ ] Start `junod` service
- [ ] Verify: `junod status` — node syncing, no panic
- [ ] Verify: `junod query wasm libwasmvm-version` — reports version (BN254 presence verified by storing precompile wasm)
- [ ] Verify: store precompile wasm on mainnet — **success**

**Exit criteria:** BN254 precompile wasm stores and executes on mainnet. Gas ~203k for VerifyProof.

---

## Phase 5: Post-Upgrade Verification & Documentation (est. 1 hour)

- [ ] Store `zk_verifier_precompile.wasm` on mainnet — record code ID
- [ ] Instantiate contract — record contract address
- [ ] Execute `VerifyProof` — record gas used
- [ ] Compare gas: mainnet vs devnet (should match within ±5%)
- [ ] Update `BN254_BENCHMARK_RESULTS.md` with mainnet numbers
- [ ] Update `POST_VOTE_EXECUTION_PLAN.md` — mark Track B complete
- [ ] Update `ARTICLE_JLENS_SCALING_STUDY_THREE_MODELS_2026_08_02.md` — add verified mainnet precompile status
- [ ] Post results to DAO Discord / Telegram

---

## Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| v3.0.6 re-anchor reveals new drift | Low | Low | `imports.rs` and `memory.rs` are byte-identical v3.0.1↔v3.0.5; v3.0.6 likely same |
| `go mod tidy` fails to resolve local replace | Low | Medium | Use .so swap approach instead of Go replace directive for production builds |
| Cosmovisor doesn't pick up upgrade binary | Low | High | Test cosmovisor path locally on devnet first |
| Governance vote fails | Medium | High | Coordinate with Jake/Dimi before submission; have ≥40% voting power lined up |
| Gas on mainnet differs from devnet | Low | Low | SDK gas is deterministic; expect exact match |
| Upstream CosmWasm maintainers reject PR | Medium | Low | P2 (patches) works independently; upstream merge is nice-to-have, not blocking |

---

## Timeline Summary

| Phase | Duration | Dependency |
|-------|----------|------------|
| 1. Finalize patches | 1-2 hours | None |
| 2. Build integration script | 1 hour | Phase 1 |
| 3. Build patched junod | 2-3 hours | Phase 2 |
| 4. Chain upgrade | 1-2 days (governance) | Phase 3 + Jake coordination |
| 5. Verification | 1 hour | Phase 4 complete |
| **Total active work** | **~5-7 hours** | |
| **Total wall-clock** | **~3-4 days** (governance voting period) | |

---

## Reproducibility

All steps are scripted and idempotent:

```powershell
# Phase 1: verify patches against v3.0.6
powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1 -CosmwasmTag v3.0.6
powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\apply-and-test-v3.ps1 -CosmwasmTag v3.0.6
```

Patch series: `wasmvm-fork/patches/v3.0.x/` (10 patches, numbered 00-09)
Drift report: `wasmvm-fork/patches/DRIFT_REPORT_V3.md`
Worklog: `wasmvm-fork/patches/FORWARD_PORT_V3.md`
Publication shape: `wasmvm-fork/patches/PUBLICATION_SHAPE.md` (P2 decision)
Upstream tracking: `docs/UPSTREAM_TRACKING.md` (#2685 deferred to Backlog, re-check late Q3 2026)

---

*Created 2026-08-04. Updated 2026-08-04: P2 (no-fork) approach adopted. Supersedes the deferred workstream in DEFERRED_WORK_PLAN.md.*
