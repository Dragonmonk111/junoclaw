# BN254 patch set — `cosmwasm` v2.2.2 baseline (Track A)

**Captured:** 2026-05-06.
**Base ref:** `CosmWasm/cosmwasm` tag `v2.2.2` (commit `dd90b6f9c`, "chore: Release").
**Reason for this base:** `wasmvm` v2.2.4 (the Juno-mainnet-pinned host) declares
`cosmwasm-std` and `cosmwasm-vm` from this exact tag in its `libwasmvm/Cargo.toml`.
Pinning to anything else creates trait-mismatch errors at the host/guest boundary.

## Apply order

The patches are numbered `00`-`09` so that `for p in *.patch; do git apply "$p"; done`
works against a clean `v2.2.2` checkout. The order matters because the toolchain pin
must land first, the std crate must compile before the vm crate, and the new
`crypto-bn254` crate is consumed by both.

| # | File | Target | What it does |
|---|------|--------|--------------|
| 00 | `00-rust-toolchain.toml.patch` | `<root>/rust-toolchain.toml` | Pins Rust 1.78.0. Required because wasmer-vm 4.3.7 (transitive dep) imports `__rust_probestack`, which Rust 1.81+ removed. See "Toolchain note" below. |
| 01 | `01-cosmwasm-std.imports.rs.patch` | `packages/std/src/imports.rs` | Declares the three BN254 host imports (`bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality`) and the safe Rust wrappers contracts call. |
| 02 | `02-cosmwasm-std.traits.rs.patch` | `packages/std/src/traits.rs` | Adds the BN254 method shims to the `Api` trait so contracts get them via `deps.api`. |
| 03 | `03-cosmwasm-std.testing.mock.rs.patch` | `packages/std/src/testing/mock.rs` | Provides a software fallback for the BN254 host calls in unit-test mocks (uses the new `cosmwasm-crypto-bn254` crate directly). |
| 04 | `04-cosmwasm-std.Cargo.toml.patch` | `packages/std/Cargo.toml` | Adds `cosmwasm-crypto-bn254` as a path dependency for the mock fallback above. |
| 05 | `05-cosmwasm-vm.imports.rs.patch` | `packages/vm/src/imports.rs` | The host-side implementation: `do_bn254_add`, `do_bn254_scalar_mul`, `do_bn254_pairing_equality`, plus the gas accounting (`charge_host_call_gas`), error-code mapping, and the size limits (`MAX_BN254_PAIRING_PAIRS = 64`). |
| 06 | `06-cosmwasm-vm.instance.rs.patch` | `packages/vm/src/instance.rs` | Registers the three new host imports in the Wasmer import-object so contracts can `extern "C"` them. |
| 07 | `07-cosmwasm-vm.compatibility.rs.patch` | `packages/vm/src/compatibility.rs` | Whitelists the new host imports in the contract-compatibility check (otherwise contracts that call them are rejected at upload). |
| 08 | `08-cosmwasm-vm.Cargo.toml.patch` | `packages/vm/Cargo.toml` | Adds `cosmwasm-crypto-bn254` as a path dependency (the host implementation lives in this crate). |
| 09 | `09-cosmwasm-crypto-bn254-new-crate.patch` | `packages/crypto-bn254/**` | The whole new `cosmwasm-crypto-bn254` crate: `bn254.rs` (arkworks-backed primitives), `gas.rs` (algebraic gas model), `errors.rs`, benches, EIP-196/197/1108 vector tests. Cargo.lock is intentionally excluded. |

## Test result on this set

* `cargo +1.78.0 test -p cosmwasm-crypto-bn254` — **22/22** (13 unit + 9 EIP vectors).
* `cargo +1.78.0 test -p cosmwasm-vm --lib` — **311/311** (full vm suite incl. all
  BLS12-381, secp256k1, Wasmer backend tests).

## Toolchain note (`__rust_probestack`)

The pin in `00-rust-toolchain.toml.patch` is **not** a project preference — it's a
hard compatibility floor. wasmer-vm 4.3.7's JIT trampolines reference the runtime
symbol `__rust_probestack` via inline assembly. That symbol was removed from
`compiler-builtins` in Rust 1.81 (rust-lang/rust#126985), replaced with the LLVM
`__llvm_stack_probe` intrinsic. Building this patch set on Rust ≥ 1.81 fails at
link time with:

```
rust-lld: error: undefined symbol: __rust_probestack
  >>> referenced by libcalls.rs:668
  >>>     wasmer_vm-*.rcgu.o:(wasmer_vm_probestack) in archive libwasmer_vm-*.rlib
```

This affects every source build of `cosmwasm` v2.2.x and `wasmvm` v2.2.x as of
the date captured. Validators running pre-built `libwasmvm.x86_64.so` are not
affected. The issue resolves itself on `wasmvm` v3.x (which moves to wasmer 5.x
with the new intrinsic) — relevant for Track B, not for this set.

If the upstream maintainers prefer to surface the constraint in
`Cargo.toml`'s `rust-version` instead, drop patch `00` and add the floor to
`[workspace.package]`.

## What's intentionally **not** here

* **No wasmvm Go wrappers.** The original v2.2.0-targeted patches included
  `wasmvm.api.rs.patch` and `wasmvm.lib.go.patch` to add cgo wrappers around
  the new host calls. Inspection of `wasmvm` v2.2.4's `lib_libwasmvm.go` and
  `internal/api/bindings.h` shows BLS12-381 itself has **no** Go-side wrappers
  in this version — the BN254 patches were trying to mirror a pattern that
  doesn't exist on this branch. Go wrappers are deferred to Track B (v3.x),
  where the wasmvm Go layer has the corresponding scaffolding. The dropped
  patches remain at `../wasmvm.api.rs.patch.dropped` /
  `../wasmvm.lib.go.patch.dropped` for the audit trail.

* **No Cargo.lock.** Excluded from patch 09 because it's a generated artefact
  that drifts on every dependency update. `cargo` will regenerate it when the
  patches are applied.

## Reproduction script

`../rebase-track-a.sh` automates the clone-checkout-apply-test loop. As of
this capture it falls back to manual conflict resolution at three sites
(`packages/std/Cargo.toml`, `packages/vm/Cargo.toml`,
`packages/vm/src/imports.rs`) which were resolved in-session. Future runs
should apply this `v2.2.2/` set cleanly without those conflicts.
