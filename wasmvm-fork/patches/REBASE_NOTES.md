# Rebase notes — Track A

## Current state (2026-05-10)

Two patch series live here:

* `v2.2.2/` — pinned to the cosmwasm tag that wasmvm v2.2.4 / Juno mainnet
  consume. **10/10 clean** against `v2.2.2` (verified by
  `bash wasmvm-fork/patches/check-baseline.sh v2.2.2`). This is the audit /
  governance reference set.

* `v2.2.7/` — forward-ported to the latest cosmwasm 2.2.x tag at time of
  capture. **10/10 clean** against `v2.2.7` (verified by
  `bash wasmvm-fork/patches/check-baseline.sh v2.2.7`). Drift was minimal:
  two `Cargo.toml` patches (04 and 08) needed regeneration because v2.2.7
  switched `cosmwasm-crypto` to workspace inheritance. The other 8 patches
  apply byte-for-byte unchanged. See `v2.2.7/README.md` for the full diff
  description and `regen-v227.sh` for the reproduction script.

The wasmvm Go wrappers (`wasmvm.api.rs.patch` and `wasmvm.lib.go.patch`)
remain dropped on Track A. wasmvm v2.2.x lacks the BLS12-381 Go wrapper
scaffolding our patches were trying to mirror; Go bindings are deferred to
Track B (cosmwasm/wasmvm v3.x).

## Tooling

* `check-baseline.sh` — fast `git apply --check` per patch, no cargo test.
  Use this to answer "do the current patches still apply to upstream tag X?"
  in 30 seconds. Accepts a tag arg (default `v2.2.2`) and `PATCH_DIR=...`
  override.
* `regen-v227.sh` — deterministic regeneration of the v2.2.7 series from
  the v2.2.2 source. Idempotent.
* `rebase-track-a.sh` — full clone-apply-test loop. Slow (cargo test on
  cosmwasm-vm runs the full 311-test suite). Use when you actually need to
  validate behaviour, not just patch alignment.

## History (older entries)

### 2026-05-10

* `v2.2.7/` series captured. 10/10 clean. See `v2.2.7/README.md`.
* `check-baseline.sh` and `regen-v227.sh` added.

### 2026-05-06

* `v2.2.2/` series captured in-session. 10/10 clean. Tests passed
  (22/22 crypto-bn254, 311/311 cosmwasm-vm). See `v2.2.2/README.md`.

### 2026-05-05 (legacy attempt against v2.2.4 with old loose patches)

* The original loose patches at `wasmvm-fork/patches/*.patch` (Track A
  pre-numbering) targeted cosmwasm v2.2.0 and a wasmvm Go-side surface that
  v2.2.4 lacks. Result of the 2026-05-05 rebase attempt:
    * OK: `cosmwasm-vm.imports.rs.patch`
    * OK: `cosmwasm-std.imports.rs.patch`
    * OK: `cosmwasm-std.traits.rs.patch`
    * CONFLICT: `wasmvm.api.rs.patch` (now at `*.dropped`)
    * CONFLICT: `wasmvm.lib.go.patch` (now at `*.dropped`)
* Resolution path: capture a numbered series against the wasmvm-pinned
  cosmwasm tag, drop the wasmvm Go-side patches as Track B work. Done in
  the 2026-05-06 session above.
