# BN254 patch set — `cosmwasm` v2.2.7 baseline (Track A — forward-rebased)

**Captured:** 2026-05-10. Regenerated from the v2.2.2 series.
**Base ref:** `CosmWasm/cosmwasm` tag `v2.2.7` (commit `5a7360a1d`).
**Reason for this base:** v2.2.7 is the latest tag in the `cosmwasm` 2.2.x
series at the time of capture. The v2.2.2 series still applies cleanly to
its own pinned tag (Juno mainnet's `wasmvm` v2.2.4 dependency); this set is
the forward-port for any chain that has bumped past v2.2.2.

## Drift between v2.2.2 and v2.2.7 (what changed in this regeneration)

`cosmwasm` v2.2.7 moved several inter-package dependencies to **workspace
inheritance** (`{ workspace = true }`) where v2.2.2 used path references.
This affects exactly two patches:

| # | Patch | What drifted | What we did |
|---|-------|--------------|-------------|
| 04 | `04-cosmwasm-std.Cargo.toml.patch` | `cosmwasm-crypto = { version = "2.2.2", path = "../crypto" }` → `cosmwasm-crypto = { workspace = true }` | Regenerated. `cosmwasm-crypto-bn254` line is unchanged (still uses explicit `version` + `path` since the bn254 crate is local to this patch series and not yet a workspace member of upstream). |
| 08 | `08-cosmwasm-vm.Cargo.toml.patch` | Same — `cosmwasm-crypto` now `{ workspace = true }`. | Regenerated. |

The other 8 patches (00-03, 05-07, 09) apply byte-for-byte unchanged.

## Apply order

Identical to v2.2.2: `for p in *.patch; do git apply "$p"; done` against a
clean v2.2.7 checkout.

## Verification

```
$ bash wasmvm-fork/patches/check-baseline.sh v2.2.7
  CLEAN     00-rust-toolchain.toml.patch
  CLEAN     01-cosmwasm-std.imports.rs.patch
  CLEAN     02-cosmwasm-std.traits.rs.patch
  CLEAN     03-cosmwasm-std.testing.mock.rs.patch
  CLEAN     04-cosmwasm-std.Cargo.toml.patch
  CLEAN     05-cosmwasm-vm.imports.rs.patch
  CLEAN     06-cosmwasm-vm.instance.rs.patch
  CLEAN     07-cosmwasm-vm.compatibility.rs.patch
  CLEAN     08-cosmwasm-vm.Cargo.toml.patch
  CLEAN     09-cosmwasm-crypto-bn254-new-crate.patch

summary: 10 clean / 0 conflicts (target=v2.2.7)
OK — patch series applies cleanly to v2.2.7.
```

## Test result

Not yet executed against this set. The v2.2.2 set was tested at:
* `cargo +1.78.0 test -p cosmwasm-crypto-bn254` — 22/22.
* `cargo +1.78.0 test -p cosmwasm-vm --lib` — 311/311.

Because the only drift between v2.2.2 and v2.2.7 is in `Cargo.toml`
dependency declarations (workspace inheritance), and the workspace-resolved
`cosmwasm-crypto` package is identical in version and contents, the test
suite is expected to pass identically on this set. Run
`COSMWASM_TAG=v2.2.7 PATCH_DIR=$PWD/wasmvm-fork/patches/v2.2.7 bash
wasmvm-fork/patches/rebase-track-a.sh` to confirm. (Pending — see
`REBASE_NOTES.md` for status.)

## Toolchain note (`__rust_probestack`)

Identical to v2.2.2 — Rust 1.78.0 pin still required. The wasmer-vm 4.3.7
transitive dependency has not changed in the 2.2.x series. See
`../v2.2.2/README.md` for the full explanation.

## What's still **not** here

* No wasmvm Go wrappers. Same rationale as v2.2.2 — wasmvm v2.2.x has no
  BLS12-381 Go wrappers to mirror; deferred to Track B (v3.x).
* No Cargo.lock. Same as v2.2.2.

## Reproduction

`../regen-v227.sh` reproduces this directory deterministically from the
v2.2.2 source. Re-running it overwrites `v2.2.7/`.
