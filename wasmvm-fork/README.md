# wasmvm-fork — BN254 host-function patches for CosmWasm

> **Status:** Reference implementation + integration patches, ready to lift into
> a fork of `CosmWasm/cosmwasm` and `CosmWasm/wasmvm`. Benchmarks and proof of
> equivalence live under `benches/` and `tests/`.

This directory contains everything needed to add three BN254 (alt_bn128)
host functions to the CosmWasm VM, giving CosmWasm contracts the same
primitives that Ethereum exposes via EIP-196/197/1108:

| Host function              | Semantics (Ethereum precompile equivalent) | Input | Output | Gas (EIP-1108 SDK gas) |
|----------------------------|--------------------------------------------|-------|--------|------------------------|
| `bn254_add`                | ECADD at `0x06`                            | 128 B | 64 B   | 150                    |
| `bn254_scalar_mul`         | ECMUL at `0x07`                            | 96 B  | 64 B   | 6,000                  |
| `bn254_pairing_equality`   | ECPAIRING at `0x08`                        | 192·N B | 1 bit | 45,000 + 34,000·N     |

The shape, byte layout, and gas schedule are deliberately lifted from the
Ethereum precompiles so that existing Groth16 tooling (`snarkjs`, `circom`,
`gnark`) can target a CosmWasm chain with zero adaptation.

---

## Why this exists

The JunoClaw zk-verifier contract
(`juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` on uni-7)
verifies a Groth16 proof in pure CosmWasm using the `arkworks` crate family.
A single `VerifyProof` call burns **371 486 gas** on-chain — fine for a PoC,
heavy for a workload that wants to verify proofs at every block.

With the host functions in this crate the same Groth16 verification should
land around **~187 000 gas** — exactly a **~2× reduction** and, more
importantly, flat in proof complexity, matching Ethereum's cost curve
derived by Nebra:

```
gas ≈ (181 + 6 · l) kgas      (l = number of public inputs)
```

A full discussion of the gas numbers, the current Cosmos-ecosystem gap, and
the governance path is in `../docs/BN254_PRECOMPILE_CASE.md`.

---

## Layout

```
wasmvm-fork/
├── README.md                          ← this file
├── BUILD_AND_TEST.md                  ← reproducible build + bench recipe
├── cosmwasm-crypto-bn254/             ← standalone Rust crate (no_std-friendly)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                     ← public API + docs
│   │   ├── bn254.rs                   ← arkworks-backed implementations
│   │   ├── errors.rs                  ← Bn254Error taxonomy
│   │   └── gas.rs                     ← EIP-1108 mirror, expressed in Wasm gas
│   ├── tests/
│   │   └── vectors.rs                 ← Ethereum precompile test vectors
│   └── benches/
│       └── bn254.rs                   ← Criterion microbenchmarks
└── patches/
    ├── README.md                      ← patch index + apply order
    ├── cosmwasm-vm.imports.rs.patch   ← registers imports in packages/vm
    ├── cosmwasm-std.imports.rs.patch  ← guest-side extern "C" declarations
    ├── cosmwasm-std.traits.rs.patch   ← Api trait additions
    ├── wasmvm.api.rs.patch            ← Rust-side C ABI wrapper (bn254_ffi.rs)
    └── wasmvm.lib.go.patch            ← Go-side public + internal/api surface
```

The Go-level wrapper (`internal/api/bn254.go`) and the public top-level
`Bn254Add` / `Bn254ScalarMul` / `Bn254PairingEquality` functions are both
bundled into `patches/wasmvm.lib.go.patch` so that a single `git apply`
leaves the wasmvm fork in a buildable state.

The Rust crate is intentionally standalone: it has no `cosmwasm-vm`
dependency, so it can be vendored straight into `CosmWasm/cosmwasm` as
`packages/crypto-bn254/` without pulling in any JunoClaw code.

---

## Backend: why arkworks and not substrate-bn

Three reasons:

1. **Bit-identical equivalence.** The JunoClaw zk-verifier contract already
   uses `ark-bn254 0.5`. By picking the same backend for the precompile we
   guarantee that a proof accepted in pure Wasm is accepted identically by
   the native path — useful for differential testing during the rollout.
2. **Maintenance.** arkworks is actively maintained, audited by Zellic in
   2023, and already pulled into Cosmos chains via `cosmwasm-crypto` for the
   BLS12-381 host functions. No new supply-chain risk.
3. **Subgroup checks.** ark-bn254 exposes
   `is_in_correct_subgroup_assuming_on_curve` for G2 out of the box, which
   is required for soundness (see EIP-2537 discussion).

We do pay a ~20% runtime penalty versus `substrate-bn`'s assembly-tuned
Miller loop. That penalty is dwarfed by the 2× reduction we gain from
moving out of Wasm, and can be closed later by swapping the backend behind
the same public API.

---

## Gas schedule (EIP-1108 mirror)

CosmWasm's internal gas unit is **100× SDK gas** (see
`wasmd`'s `DefaultGasMultiplier = 100`). The table below shows both:

| Op                       | SDK gas            | CosmWasm VM gas      |
|--------------------------|--------------------|----------------------|
| `bn254_add`              | 150                | 15 000               |
| `bn254_scalar_mul`       | 6 000              | 600 000              |
| `bn254_pairing_equality` | 45 000 + 34 000·N  | 4 500 000 + 3 400 000·N |

The per-pair coefficient covers one Miller-loop iteration; the base cost
covers the final exponentiation. These numbers are **ceilings** — our
Criterion benchmarks on a 2023 M2 machine measure real runtimes
well below these budgets (see `benches/bn254.rs` output captured in
`BUILD_AND_TEST.md`).

---

## Determinism guarantees

| Concern                               | Mitigation                                                                 |
|---------------------------------------|----------------------------------------------------------------------------|
| Non-canonical field elements (≥ p)    | `Fq::from_bigint` returns `None`; we surface `InvalidFieldElement`.        |
| Points not on curve                   | `AffineRepr::is_on_curve()` guard before any operation.                    |
| G2 cofactor-torsion points            | `is_in_correct_subgroup_assuming_on_curve()` check on every G2 decode.    |
| Scalar ≥ r                            | Reduced mod r (`Fr::from_be_bytes_mod_order`) — matches EIP-196 semantics. |
| RNG / timing                          | No randomness in any code path; no `std::time` usage.                      |
| Allocator non-determinism             | All allocations are bounded by input length (pairing: `n` elements).       |

Differential testing against the pure-Wasm `zk-verifier` contract is the
acceptance criterion: see `tests/vectors.rs`.

---

## How to use this fork

Two paths are supported, tracked as separate todos in the upstream PR:

### Path A — Vendor the crate into `CosmWasm/cosmwasm`

1. Copy `cosmwasm-crypto-bn254/` to `packages/crypto-bn254/` in a
   `CosmWasm/cosmwasm` fork.
2. Apply `patches/cosmwasm-vm.imports.rs.patch` — registers `bn254_add`,
   `bn254_scalar_mul`, `bn254_pairing_equality` as VM imports with gas
   hooks.
3. Apply `patches/cosmwasm-std.imports.rs.patch` and
   `patches/cosmwasm-std.traits.rs.patch` — exposes them on the guest
   side as `Api::bn254_*`.
4. Cross-check with `tests/vectors.rs`, which includes the
   EIP-196/197 conformance vectors.

### Path B — Patch `CosmWasm/wasmvm` for a private devnet

1. Apply `patches/wasmvm.api.rs.patch` and `patches/wasmvm.lib.go.patch`
   to build a `libwasmvm.a` that exposes `bn254_*` symbols through cgo.
2. Drop the replacement `libwasmvm.a` into a `junod` build — see
   `../devnet/` for the full ephemeral-validator harness.

Path B is what the governance-signalling PR will cite for benchmark
evidence. Path A is what lands upstream once the signal passes.

---

## Safety / soundness review checklist

Anyone reviewing the upstream PR should tick all of these:

- [ ] `Fq::from_bigint` None-check path returns `InvalidFieldElement`.
- [ ] `G1Affine::is_on_curve` is called before any arithmetic.
- [ ] `G2Affine::is_in_correct_subgroup_assuming_on_curve` is called after
      on-curve check (subgroup is non-trivial on BN254 G2).
- [ ] Scalars above `r` are handled by reduction, not rejection (matches
      EIP-196).
- [ ] Empty pairing input returns `Ok(true)` (matches EIP-197).
- [ ] Gas is charged before the operation, not after.
- [ ] Test vectors from EIP-196 appendix pass bit-for-bit.
- [ ] Differential test against `zk-verifier` pure-Wasm verifies
      equivalent-proof acceptance on 1 000 random Groth16 proofs.

---

## Licence

Apache-2.0, matching CosmWasm. Copied files from `CosmWasm/cosmwasm` carry
their original copyright headers; new code is © 2025 JunoClaw contributors.
