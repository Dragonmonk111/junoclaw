# Patch set — CosmWasm / wasmvm BN254 integration

These are the upstream patches that wire the `cosmwasm-crypto-bn254`
crate into the wider CosmWasm stack so that on-chain contracts can call
`bn254_add`, `bn254_scalar_mul`, and `bn254_pairing_equality` through
`deps.api.*`.

The patches are ordered — apply them in the listed sequence to a fresh
clone of the upstream repos.

## Targets

| Patch                            | Upstream repo           | Version pin     | Effect                                                        |
|----------------------------------|-------------------------|-----------------|---------------------------------------------------------------|
| `cosmwasm-vm.imports.rs.patch`   | CosmWasm/cosmwasm       | v2.2.0          | Registers the three BN254 host functions with the VM.         |
| `cosmwasm-std.imports.rs.patch`  | CosmWasm/cosmwasm       | v2.2.0          | Adds guest-side `extern "C"` declarations to `cosmwasm-std`.  |
| `cosmwasm-std.traits.rs.patch`   | CosmWasm/cosmwasm       | v2.2.0          | Adds `Api::bn254_*` methods with default-to-mock impls.       |
| `wasmvm.api.rs.patch`            | CosmWasm/wasmvm         | v2.2.0          | C-ABI shims in `libwasmvm` so Go/cgo can call the Rust crate. |
| `wasmvm.lib.go.patch`            | CosmWasm/wasmvm         | v2.2.0          | Go wrappers callable by `x/wasm` and by consumers.            |

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
