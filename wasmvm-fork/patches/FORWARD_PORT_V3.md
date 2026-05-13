# Forward-port worklog — `v2.2.7/` → `v3.0.x/`

*Started 2026-05-13 in response to Jake's PR [`CosmosContracts/juno#1202`](https://github.com/CosmosContracts/juno/pull/1202) pinning `wasmvm/v3 v3.0.4` directly with no `replace` directive for our fork. For prop #374 to land in Juno v30, the BN254 patches need to forward-port from `cosmwasm` v2.2.x onto `cosmwasm` v3.0.x.*

---

## Goal

Produce a `wasmvm-fork/patches/v3.0.x/` series equivalent to the existing `v2.2.7/` set:

- 10 numbered patches against `CosmWasm/cosmwasm` v3.0.x (currently `v3.0.1` is the tag pinned by `wasmvm` v3.0.4).
- All patches `git apply --check` clean against the v3 tag.
- `cargo test` passes against the patched checkout (target: 22/22 crypto-bn254, ≥311/311 cosmwasm-vm — same baselines as v2.2.7, possibly more if v3 added tests).
- Forward-port matches the architectural shape v3 uses (capability strings, feature flags, gas-table layout) — these may have moved between 2.2.x and 3.0.x.

The wasmvm-side wrapper patches (`wasmvm.api.rs.patch.dropped`, `wasmvm.lib.go.patch.dropped`) become **live again** in v3 — finding 2 in `UPSTREAM_ISSUE_DRAFTS.md` §Issue 2 noted that wasmvm v2.2.x lacked the BLS12-381 Go-wrapper analogue we were mirroring; v3.x has it. So the v3 patch series will likely include the previously-dropped Go wrappers.

## Status

- [x] **2026-05-13 (morning):** Worklog opened. Skeleton `v3.0.x/` directory created.
- [x] **2026-05-13 (afternoon, day 1 baseline check):** Set up `check-baseline-v3.{sh,ps1}`. Cloned cosmwasm v3.0.1 (`74a568d38`) into `~/junoclaw-build/cosmwasm-bn254`. Ran `git apply --check` against all 10 v2.2.7 patches.
  - **Result: 7 CLEAN / 2 3-way-OK / 1 needs reanchor.**
  - Drift report: [`DRIFT_REPORT_V3.md`](./DRIFT_REPORT_V3.md).
  - Headline: drift is **much lower** than the original plan estimated — 1.5-2 working days, not 3-5.
- [x] **2026-05-13 (PM, day 2 forward-port complete):** Reanchored `01-cosmwasm-std.imports.rs.patch` against v3's `packages/std/src/exports/imports.rs` via `regen-patch-01-v3.ps1` (path move + line-number shift; BN254 `Api` impl placed before `fn debug` in `impl Api for ExternalApi`). Regenerated `04` and `08` Cargo.toml patches via `regen-patches-cargo-v3.ps1` (manual application against v3's non-`workspace=true` style, since `git apply --3way` couldn't merge with v2.2.7 SHAs). Bumped `00` rust-toolchain pin from 1.78 to 1.82 after first `cargo test` run failed (v3 transitive deps `wasmer 5.0.6` need 1.81, `icu_provider 2.0.0` need 1.82).
  - **Result: 10/10 patches CLEAN against v3.0.1.** Patch series complete in `v3.0.x/`. README in that dir documents the per-patch provenance.
  - Helpers: `regen-patch-01-v3.ps1`, `regen-patches-cargo-v3.ps1`, `finalize-v3-series.ps1`, `apply-and-test-v3.ps1`. All idempotent.
  - The originally-feared HIGH-risk patches (`05`, `07`) applied **structurally CLEAN** — but see the finding below for `05`.
  - **🟡 Finding (day-2.5 work item): `cargo test -p cosmwasm-vm` against patched v3.0.1 fails to compile.** Root cause: v3's `cosmwasm-vm::memory::write_region` changed its signature from `write_region(ptr, data)` (v2.2.x) to `write_region(env, &mut StoreMut, ptr, data)` (v3.x). Our `05-cosmwasm-vm.imports.rs.patch` body invokes the old 2-arg signature inside the `do_bn254_*` host-fn impls. The patch applies cleanly because the surrounding lines (the `extern "C"` registrations, the gas-cost constants) didn't move — but the call sites are now stale. **Fix shape:** rewrite the BN254 host-fn impls in `05` to thread `env` and `store` through, matching the v3 `do_bls12_381_*` impls already in v3 source. ~30 min of focused work. This finding is the v3-specific equivalent of why we feared `05` would need rewriting — it does, just not for the reason the day-0 plan predicted.
- [ ] **Day 2-3:** Add wasmvm-side patches (`10-wasmvm.api.rs.patch`, `11-wasmvm.lib.go.patch` mirroring the BLS12-381 path). These were the dropped patches in v2.2.x; v3.x has the analogue. Run a separate `check-baseline-v3-wasmvm.ps1` against `wasmvm` v3.0.4 first.
- [ ] **Day 3:** Tag `bn254-precompile-v3.0.4` on the `Dragonmonk111/wasmvm` fork. Hand Jake the `replace` directive for v30's `go.mod`:
  ```
  replace github.com/CosmWasm/wasmvm/v3 => github.com/Dragonmonk111/wasmvm/v3 v3.0.4-bn254
  ```

## Patch-by-patch plan

| #  | Patch                                              | Risk of drift on v3 | Notes |
|----|----------------------------------------------------|--------------------:|-------|
| 00 | `00-rust-toolchain.toml.patch`                     |             **LOW** | Pin Rust 1.78. v3 may already pin a newer toolchain — check first. |
| 01 | `01-cosmwasm-std.imports.rs.patch`                 |          **MEDIUM** | Adds `extern "C"` declarations. Surface stable across minor versions; might collide with new BLS additions in v3. |
| 02 | `02-cosmwasm-std.traits.rs.patch`                  |          **MEDIUM** | Adds `Api::bn254_*` methods. Trait surface area is the most-likely-to-have-moved between 2.2 → 3.0. |
| 03 | `03-cosmwasm-std.testing.mock.rs.patch`            |             **LOW** | Mock impl. Self-contained. |
| 04 | `04-cosmwasm-std.Cargo.toml.patch`                 |          **MEDIUM** | Workspace inheritance. v3 likely uses a different `[package]` layout. |
| 05 | `05-cosmwasm-vm.imports.rs.patch`                  |            **HIGH** | Host-fn registration table. Most likely to need rewrite. |
| 06 | `06-cosmwasm-vm.instance.rs.patch`                 |          **MEDIUM** | Instance setup. Depends on v3's environment-builder shape. |
| 07 | `07-cosmwasm-vm.compatibility.rs.patch`            |            **HIGH** | Capability strings. v3 may have introduced a new capability vocabulary. |
| 08 | `08-cosmwasm-vm.Cargo.toml.patch`                  |          **MEDIUM** | Same as 04. |
| 09 | `09-cosmwasm-crypto-bn254-new-crate.patch`         |             **LOW** | New file (52K), no upstream conflict. |
| 10 | (new) `10-wasmvm.api.rs.patch`                     |             **N/A** | Was `wasmvm.api.rs.patch.dropped`; revive against v3. |
| 11 | (new) `11-wasmvm.lib.go.patch`                     |             **N/A** | Was `wasmvm.lib.go.patch.dropped`; revive against v3. |

The HIGH-risk patches (`05`, `07`) are the ones to validate first — if either of those needs a non-trivial rewrite (i.e., the upstream API shape has moved enough that we can't just `--3way` it), the timeline expands.

## Tooling needed

- `wasmvm-fork/patches/check-baseline-v3.sh` — same as `check-baseline.sh` but pinned to `cosmwasm` v3.0.1 + `wasmvm` v3.0.4.
- `wasmvm-fork/patches/rebase-track-b.sh` — full clone-apply-test against v3 stack. Mirror of `rebase-track-a.sh`.
- `wasmvm-fork/patches/v3.0.x/README.md` — patch-set manifest, mirroring `v2.2.7/README.md`.

## Cross-references

- [`UPSTREAM_ISSUE_DRAFTS.md`](../../docs/UPSTREAM_ISSUE_DRAFTS.md) §Issue 2 — the wasmvm-side issue body that will reference this v3 series once forward-port is complete.
- [`JAKE_DM_TRACK_B_CLARIFY.md`](../../docs/JAKE_DM_TRACK_B_CLARIFY.md) — the DM template explaining the three branches (a/b/c). This worklog is the realisation of branch (a) "we do the forward-port."
- [`JUNO_V30_PR_ASSESSMENT.md`](../../docs/JUNO_V30_PR_ASSESSMENT.md) — the Moultbook-style review of PR #1202; identifies the v3 pin as the constraint forcing this work.
- [`docs/JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md`](../../docs/JUNO_DAOCONTRACTS_PR_928_929_ANALYSIS.md) — context on Jake's parallel work shipping on `DA0-DA0/dao-contracts` against the same v30 timeline.

## Decision: when to start

**Start the day-1 baseline check after the user reviews this worklog.** No commit or upstream activity until the user confirms the timeline budget. This is a 3-5 working-day commitment; we shouldn't start it without explicit go-ahead.

The day-1 baseline check itself is cheap (~30 min, no commit needed for the discovery). But the rewrite work that follows is substantive and should be a clear sprint commitment, not an opportunistic task.

---

*Apache-2.0. Worklog tracks state across sessions; updated after every working day on Track B.*
