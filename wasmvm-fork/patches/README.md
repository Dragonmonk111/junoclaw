# Patch set — CosmWasm / wasmvm BN254 integration

These are the upstream patches that wire the `cosmwasm-crypto-bn254`
crate into the wider CosmWasm stack so that on-chain contracts can call
`bn254_add`, `bn254_scalar_mul`, and `bn254_pairing_equality` through
`deps.api.*`.

> **Current canonical sets are the numbered series in `v2.2.2/` and
> `v2.2.7/`.** The loose `*.patch` files at the top level are the
> pre-numbering Track A drafts kept for the audit trail. Read the
> numbered-series READMEs first.

## Sets

| Series   | Upstream tag    | Status                                        | Reference            |
|----------|-----------------|-----------------------------------------------|----------------------|
| `v2.2.2/` | `CosmWasm/cosmwasm` v2.2.2 (`dd90b6f9c`) | 10/10 clean; tests pass (22/22, 311/311). The audit / governance reference set; matches what wasmvm v2.2.4 / Juno mainnet consume. | `v2.2.2/README.md`   |
| `v2.2.7/` | `CosmWasm/cosmwasm` v2.2.7 (`5a7360a1d`) | 10/10 clean; tests not yet run on this tag. Forward port; only 2 Cargo.toml patches (04, 08) differ from v2.2.2 due to workspace inheritance. | `v2.2.7/README.md`   |

`REBASE_NOTES.md` tracks state across sessions.

## Tooling

* `check-baseline.sh <tag>` — fast `git apply --check` per patch (no cargo
  test). Use for "do these still apply to upstream tag X?" in seconds.
* `regen-v227.sh` — idempotent regeneration of the `v2.2.7/` series from
  the `v2.2.2/` source. Pattern-matchable for future bumps.
* `rebase-track-a.sh` — full clone-apply-test loop, including
  `cargo +1.78.0 test`. Run when validating behaviour.

## Legacy targets (loose patches, Track A pre-numbering)

These are kept for the audit trail. **Do not apply against current upstream
— use the numbered series instead.**

| Patch                            | Upstream repo           | Original pin    | Effect                                                        |
|----------------------------------|-------------------------|-----------------|---------------------------------------------------------------|
| `cosmwasm-vm.imports.rs.patch`   | CosmWasm/cosmwasm       | v2.2.0          | Registers the three BN254 host functions with the VM.         |
| `cosmwasm-std.imports.rs.patch`  | CosmWasm/cosmwasm       | v2.2.0          | Adds guest-side `extern "C"` declarations to `cosmwasm-std`.  |
| `cosmwasm-std.traits.rs.patch`   | CosmWasm/cosmwasm       | v2.2.0          | Adds `Api::bn254_*` methods with default-to-mock impls.       |
| `wasmvm.api.rs.patch.dropped`    | CosmWasm/wasmvm         | v2.2.0          | (Dropped — wasmvm v2.2.x has no BLS12-381 Go-wrapper analogue to mirror; deferred to Track B.) |
| `wasmvm.lib.go.patch.dropped`    | CosmWasm/wasmvm         | v2.2.0          | (Dropped — same reason.) |

The patches assume the upstream repos are checked out side-by-side with
`junoclaw/` so relative paths line up:

```
parent/
├── junoclaw/                   ← this repo
├── cosmwasm-bn254/             ← CosmWasm/cosmwasm fork (apply 3 patches)
└── wasmvm-bn254/               ← CosmWasm/wasmvm fork (apply 2 patches)
```

## Applying

See `../BUILD_AND_TEST.md` — the `Patch application` section contains
the copy-pasteable commands.

## Line numbers

Line numbers in the `@@` hunk headers are approximate against the
pinned versions. Upstream moves, so applying against `main` will almost
certainly need a `git apply --3way` to rebase cleanly; that's expected
and fine.

## Why patches and not a fork

Three reasons:

1. **Upstreamability.** The goal is for these changes to land in
   `CosmWasm/cosmwasm` and `CosmWasm/wasmvm` eventually. Keeping the
   diffs small and readable makes that conversation easier.
2. **Auditability.** A reviewer can read exactly what we added with
   zero noise from unrelated code movement.
3. **Re-applicability.** If CosmWasm bumps a minor version while we're
   shepherding the Juno governance track, we re-run `git apply` rather
   than re-doing a merge.
