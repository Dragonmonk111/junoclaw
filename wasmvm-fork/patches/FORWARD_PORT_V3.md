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
  - The originally-feared HIGH-risk patches (`05`, `07`) applied **structurally CLEAN** — but see the day-2.5 finding for `05`.
- [x] **2026-05-14 (day 2.5 fix complete):** Rewrote the `do_bn254_*` host-fn impls in `05-cosmwasm-vm.imports.rs.patch` to use v3's 4-arg `write_region(env, &mut store, ptr, data)` instead of v2.2.x's 2-arg `write_region(&memory, ptr, data)`. Also removed three dead `let _memory = data.memory(&store);` bindings (legacy from v2.2.x where the memory reference was needed at the call site; v3's `read_region` / `write_region` take `(env, store, ptr, ...)` and resolve memory internally). Templated against v3's `do_bls12_381_aggregate_g1` which uses the same pattern. Hunk-2 line count adjusted from 94 to 89.
  - **Verification.**
    - `check-baseline-v3.ps1`: **10/10 still CLEAN** — the fix is body-local; hunk anchors unchanged.
    - `cargo test -p cosmwasm-crypto-bn254 --no-default-features`: **rc=0, 22/22 pass** (our target suite).
    - `cargo test -p cosmwasm-vm`: **315 passed, 1 failed.** The one failure is `wasm_backend::compile::tests::contract_with_floats_passes_check` — verified to **fail identically on vanilla unpatched v3.0.1** (Windows / Rust 1.82 / wasmer 5.0.6 environment issue, not introduced by our patches). 315/316 is effectively 100% for our purposes.
  - Logs captured at `${BuildDir}/cargo-test-{crypto-bn254,vm}-v3.log`.
- [x] **2026-05-14 (day 2.5 PM, strategic findings reframe day 3):**
  - **Finding A: the wasmvm-side wrapper patches are not needed.** `check-baseline-v3-wasmvm.ps1` against wasmvm v3.0.4 surfaced two conflicts — expected. What was *not* expected: `Get-ChildItem -Recurse -Filter *.go | Select-String Bls12` returns **zero matches** anywhere in wasmvm v3.0.4. Same for `bls12_381` in `*.h`. The BLS12-381 host fns live exclusively in the cosmwasm-vm Rust layer (which we already patched). There is no parallel Go-API surface to mirror. The dropped `wasmvm.{api.rs,lib.go}.patch.dropped` files were always optional non-vm-tooling sugar; in v3 they're moot. **We will not forward-port them.**
  - **Finding B: wasmvm v3.0.4 resolves cosmwasm to v3.0.6, not v3.0.1.** Reading `libwasmvm/Cargo.toml`: `cosmwasm-std = { version = "3.0.5", ... }` and `cosmwasm-vm = { version = "3.0.5", ... }` — these are caret requirements, so Cargo resolves to whichever 3.x.y >= 3.0.5 is highest in the registry. As of 2026-05-14 the highest is **v3.0.6**. Therefore our patches must apply against v3.0.6 (or v3.0.5; both are acceptable Cargo-side, but v3.0.6 is what an unconstrained `cargo update` will land on).
  - **Re-baseline against v3.0.6:** `check-baseline-v3.ps1 -CosmwasmTag v3.0.6` returns **7 CLEAN / 3 3-way-OK / 0 CONFLICTS.** Patch 05's day-2.5 fix carries cleanly. The 3 patches needing 3-way merge are: `01-cosmwasm-std.imports.rs.patch` (likely a 1-2 line shift from the diff `wc -l`), `04-cosmwasm-std.Cargo.toml.patch` (Cargo.toml version-bump drift), `08-cosmwasm-vm.Cargo.toml.patch` (same).
  - **Cosmwasm-vm `imports.rs` and `memory.rs` are byte-identical v3.0.1 ↔ v3.0.5.** Our deepest patch (05) needs no further work to retarget to v3.0.6.
- [ ] **Day 3 (refreshed plan):**
  - Regenerate `01`, `04`, `08` against cosmwasm v3.0.6 via `--3way` or fresh anchor. Land them in `wasmvm-fork/patches/v3.0.x/` (replacing the v3.0.1-anchored versions, with a tracked git-diff for clarity).
  - Re-run `apply-and-test-v3.ps1 -CosmwasmTag v3.0.6`. Expect 22/22 crypto-bn254 + 315/316 cosmwasm-vm (same float-test pre-existing fail).
  - **Decide the publication shape for Jake.** Two options:
    - **(P1) Fork `cosmwasm` to `Dragonmonk111/cosmwasm-bn254`, tag `v3.0.6-bn254`.** Hand Jake a `[patch.crates-io]` section to add to wasmvm v3.0.4's libwasmvm/Cargo.toml — i.e., we fork wasmvm too (`Dragonmonk111/wasmvm v3.0.4-bn254`) just to inject the patch directive; libwasmvm itself is unchanged. Replace directive for v30's `go.mod`:
      ```
      replace github.com/CosmWasm/wasmvm/v3 => github.com/Dragonmonk111/wasmvm/v3 v3.0.4-bn254
      ```
    - **(P2) Ship the patches as a v30-side application step.** Jake adds a `[patch.crates-io]` directly in v30's Go module fetching script (or in his vendored wasmvm build). No fork needed on our side. Cleaner for him; we just publish the patch series.
    - P1 is the user-friendly path (one git tag for Jake to consume); P2 is the lower-blast-radius path (no maintenance burden on us). Recommend P1 for shipping speed.

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

*Update (2026-05-14):* This day-0 plan target was **wasmvm v3.0.4 + cosmwasm v3.0.1**. As of day-2.5, the actual target is **wasmvm v3.0.4 (unchanged) + cosmwasm v3.0.6** (caret resolution from wasmvm's `"3.0.5"` dep string). Drift v3.0.1 ↔ v3.0.6 is minimal in the patched files — `packages/vm/src/imports.rs` and `packages/vm/src/memory.rs` are byte-identical.

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
