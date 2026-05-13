# `memory/track-b-forward-port.md`

## Summary (3 lines)

Track B is the forward-port of the BN254 patches from `cosmwasm` v2.2.7 to `cosmwasm` v3.0.1 (the version pinned by `wasmvm` v3.0.4, which is the version pinned by Juno v30 via PR #1202). Day-2 (2026-05-13 PM) produced **10/10 CLEAN** against v3.0.1; complete patch series lives in `wasmvm-fork/patches/v3.0.x/`. Outstanding: cargo test verification against patched v3.0.1, plus the wasmvm-side wrapper patches (10/11).

## Key facts

| Item | Value |
|---|---|
| Source patch series | `wasmvm-fork/patches/v2.2.7/` |
| Target tag | `cosmwasm` v3.0.1 (`74a568d38`) |
| Day-1 baseline result | 7 CLEAN / 2 3-way-OK / 1 needs reanchor |
| **Day-2 outcome** | **10/10 CLEAN** in `wasmvm-fork/patches/v3.0.x/` |
| The one big rewrite | `01-cosmwasm-std.imports.rs.patch` — file moved to `packages/std/src/exports/imports.rs` (upstream commit `7f63657e7`); reanchored at line 102 (extern "C") and line 710 (Api impl, before `fn debug`) |
| Two manual regenerations | `04` and `08` Cargo.toml patches (v3 dropped `workspace = true`; `--3way` failed because v2.2.7 SHAs aren't in v3 repo) |
| One v3-specific finding | `00-rust-toolchain.toml.patch` had to bump from 1.78 to 1.82 (v3 deps `wasmer 5.0.6` need 1.81, `icu_provider 2.0.0` need 1.82) |
| Patches that passed despite plan-flagged HIGH risk | `05-cosmwasm-vm.imports.rs.patch`, `07-cosmwasm-vm.compatibility.rs.patch` (both CLEAN) |
| Tooling | `wasmvm-fork/patches/check-baseline-v3.{sh,ps1}`, `regen-patch-01-v3.ps1`, `regen-patches-cargo-v3.ps1`, `finalize-v3-series.ps1`, `apply-and-test-v3.ps1` |
| Build dir | `${HOME}/junoclaw-build/cosmwasm-bn254` (Linux/macOS) or `%USERPROFILE%\junoclaw-build\cosmwasm-bn254` (Windows) |

## Per-patch result table

| #  | Patch                                              | Day-1 result | Plan risk → Actual risk |
|----|----------------------------------------------------|--------------|------------------------:|
| 00 | `00-rust-toolchain.toml.patch`                     | **CLEAN**    | LOW → CLEAN |
| 01 | `01-cosmwasm-std.imports.rs.patch`                 | **CONFLICT** | MEDIUM → REWRITE |
| 02 | `02-cosmwasm-std.traits.rs.patch`                  | **CLEAN**    | MEDIUM → CLEAN |
| 03 | `03-cosmwasm-std.testing.mock.rs.patch`            | **CLEAN**    | LOW → CLEAN |
| 04 | `04-cosmwasm-std.Cargo.toml.patch`                 | **3WAY-OK**  | MEDIUM → 3WAY |
| 05 | `05-cosmwasm-vm.imports.rs.patch`                  | **CLEAN**    | **HIGH → CLEAN** |
| 06 | `06-cosmwasm-vm.instance.rs.patch`                 | **CLEAN**    | MEDIUM → CLEAN |
| 07 | `07-cosmwasm-vm.compatibility.rs.patch`            | **CLEAN**    | **HIGH → CLEAN** |
| 08 | `08-cosmwasm-vm.Cargo.toml.patch`                  | **3WAY-OK**  | MEDIUM → 3WAY |
| 09 | `09-cosmwasm-crypto-bn254-new-crate.patch`         | **CLEAN**    | LOW → CLEAN |

## Full context

### Why Track B exists

Juno PR #1202 pins `wasmvm/v3 v3.0.4` directly with no `replace` directive. For our BN254 patches to land in Juno v30, we must have a `Dragonmonk111/wasmvm` v3.0.4-bn254 tag that v30's `go.mod` can `replace`-reference. That tag requires the patch series to apply against `cosmwasm` v3.0.1 (which `wasmvm` v3.0.4 pins).

### What was discovered day 1

The reorganization in v3 cosmwasm is **localized**, not pervasive:

1. `packages/std/src/imports.rs` was moved into `packages/std/src/exports/imports.rs` (upstream commit `7f63657e7`, "Group imports and exports in exports module"). This is the only structural change that breaks our patches — and it only affects one of them.
2. Inside the moved file, BLS12-381 host fns were added (the v2.3 capability bundle). Our BN254 anchor (`ed25519_batch_verify`) is still present and at a stable line number; reanchor is mechanical.
3. Cargo.toml drift in two places (`04` and `08`) resolves cleanly via `git apply --3way`.
4. **Critically:** the v2-fork-side `cosmwasm-vm` host-fn registration table (`05-cosmwasm-vm.imports.rs.patch`) and the capability vocabulary (`07-cosmwasm-vm.compatibility.rs.patch`) — both feared HIGH-risk in the plan — applied CLEAN. These are the deepest internals; them being stable is the biggest risk-mitigation win of day 1.

### Plan deltas vs measurement

| | Plan | Measured |
|---|---|---|
| Patches CLEAN | "1-2 of 10" | 7 of 10 |
| Patches needing 3-way | "3-4 of 10" | 2 of 10 |
| Patches needing rewrite | "3-5 of 10" | 1 of 10 |
| Forward-port effort | 3-5 working days | 1.5-2 working days |

### Day-2 outcome (2026-05-13 PM)

1. ✅ **Reanchored `01`** via `regen-patch-01-v3.ps1`. Two hunks plus appended helper. Generated patch verifies clean against v3.0.1.
2. ✅ **Regenerated `04` and `08`** via `regen-patches-cargo-v3.ps1`. Manual text insertion against v3's non-`workspace=true` Cargo.toml syntax (since `--3way` failed on v2.2.7 SHAs).
3. ✅ **Bumped `00`** rust-toolchain pin from 1.78 to 1.82. Surfaced when first `cargo test` run failed during dep compilation (wasmer 5.0.6 needs 1.81, icu_provider 2.0.0 needs 1.82).
4. ✅ **`check-baseline-v3.ps1` against `v3.0.x/`: 10/10 CLEAN.**
5. 🚧 **`cargo test` verification** in progress at end of session; results capture pending. Logs: `${BuildDir}/cargo-test-{crypto-bn254,vm}-v3.log`.

### Day-3 plan

Add wasmvm-side wrapper patches (`10-wasmvm.api.rs.patch`, `11-wasmvm.lib.go.patch`) — the dropped v2.2.x ones revived against v3.0.4. v3.0.4 has the BLS12-381 wrapper analogue we mirror. Tag `Dragonmonk111/wasmvm v3.0.4-bn254`. Hand Jake the `replace` directive for v30's `go.mod`:

```
replace github.com/CosmWasm/wasmvm/v3 => github.com/Dragonmonk111/wasmvm/v3 v3.0.4-bn254
```

## Reproduction

From the junoclaw repo root:

```powershell
# Windows
powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1
```

```bash
# Linux / macOS
bash wasmvm-fork/patches/check-baseline-v3.sh
```

Both scripts are idempotent. First run clones cosmwasm (~30s); subsequent runs reuse the existing checkout (~5s).

## Cross-references

- [`wasmvm-fork/patches/DRIFT_REPORT_V3.md`](../wasmvm-fork/patches/DRIFT_REPORT_V3.md) — full long-form drift report.
- [`wasmvm-fork/patches/FORWARD_PORT_V3.md`](../wasmvm-fork/patches/FORWARD_PORT_V3.md) — original 5-day plan + ongoing worklog.
- [`memory/bn254-precompile.md`](./bn254-precompile.md) — sibling memory: BN254 precompile context.
- [`memory/v30-upgrade-pr-1202.md`](./v30-upgrade-pr-1202.md) — the upstream pin that drove this work.
- [`docs/JAKE_DM_TRACK_B_CLARIFY.md`](../docs/JAKE_DM_TRACK_B_CLARIFY.md) — DM template; update with the new 1.5-2 day timeline.
- [`docs/UPSTREAM_ISSUE_DRAFTS.md`](../docs/UPSTREAM_ISSUE_DRAFTS.md) §Issue 2 — wasmvm-side issue body.

---

*Apache-2.0.*
