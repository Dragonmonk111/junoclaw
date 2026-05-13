# Patch series — `cosmwasm` v3.0.x + `wasmvm` v3.0.x

*Forward-port of [`v2.2.7/`](../v2.2.7/README.md) onto the v3.x stack consumed by Juno v30 (per [`CosmosContracts/juno#1202`](https://github.com/CosmosContracts/juno/pull/1202) which pins `wasmvm/v3 v3.0.4` directly).*

**Status:** SKELETON — no patches yet. Worklog at [`../FORWARD_PORT_V3.md`](../FORWARD_PORT_V3.md). Baseline-discovery script and rewrites land in subsequent commits.

---

## Target tags

| Repo                    | Pinned tag              | Source-of-pin                                  |
|-------------------------|-------------------------|------------------------------------------------|
| `CosmWasm/cosmwasm`     | `v3.0.1` (transitive)   | wasmvm v3.0.4's `Cargo.toml`                   |
| `CosmWasm/wasmvm`       | `v3.0.4`                | Juno v30 PR #1202's `go.mod`                   |

`v3.0.1` is the cosmwasm tag pinned by wasmvm v3.0.4. Confirm by checking `wasmvm/Cargo.lock` after `cd /tmp/wasmvm-v3 && go mod download`.

## Difference from `v2.2.7/`

The v3 series is **not** a drop-in of the v2.2.7 series. Three categories of change expected:

1. **Surface rewrites.** Patches `05-cosmwasm-vm.imports.rs.patch` and `07-cosmwasm-vm.compatibility.rs.patch` likely need substantial rewrites — the host-fn registration table and capability vocabulary commonly move between major versions. See [`../FORWARD_PORT_V3.md`](../FORWARD_PORT_V3.md) for the per-patch risk grading.

2. **Cargo workspace migration.** Patches `04-cosmwasm-std.Cargo.toml.patch` and `08-cosmwasm-vm.Cargo.toml.patch` will need to follow whatever workspace-inheritance shape v3 uses (which typically tightens further from one major to the next).

3. **wasmvm-side patches return.** The two `*.dropped` patches in `v2.2.x` (Go wrappers for the BN254 host fns) become live again because v3.x has the BLS12-381 Go-wrapper analogue we were mirroring. They're added as patches `10-wasmvm.api.rs.patch` and `11-wasmvm.lib.go.patch`.

## Verification protocol (when patches land here)

1. `bash ../check-baseline-v3.sh` — fast `git apply --check` per patch against v3.0.1 / v3.0.4. Must be 12/12 clean (10 cosmwasm-side + 2 wasmvm-side).
2. `bash ../rebase-track-b.sh` — full clone-apply-test loop. Targets:
   - `cargo +stable test` in `cosmwasm-bn254/` (the patched cosmwasm checkout): must pass 22/22 crypto-bn254 + ≥311/311 cosmwasm-vm.
   - `make build && make test` in `wasmvm-bn254/` (the patched wasmvm checkout): must build the C shims and pass the Go FFI sanity tests.
3. `bash ../../devnet/scripts/reproduce-benchmark.sh` against the patched binary on a single-validator devnet: must reproduce the 1.823× gas reduction (203,266 SDK gas / proof) within ±5%.

## Output (when complete)

A tagged release on the `Dragonmonk111/wasmvm` fork:

```
git tag -a v3.0.4-bn254 -m "Forward-port BN254 patches onto wasmvm v3.0.4 (Juno v30 path)"
git push origin v3.0.4-bn254
```

Plus a one-line `replace` directive for Juno v30's `go.mod`:

```
replace github.com/CosmWasm/wasmvm/v3 => github.com/Dragonmonk111/wasmvm/v3 v3.0.4-bn254
```

Hand the directive to Jake via the same Telegram thread that landed PR-1202 communication.

---

*Apache-2.0. This file describes the target state. The patches themselves arrive in subsequent commits as the forward-port work proceeds.*
