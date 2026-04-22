# feat(crypto): BN254 (alt_bn128) host functions — ECADD, ECMUL, pairing equality

> **Upstream target:** `CosmWasm/cosmwasm` + `CosmWasm/wasmvm`
> **Version addressed:** `v2.2.0` (also drafted against `main`)
> **Issue reference:** `CosmWasm/cosmwasm#751` (Crypto API meta)
> **Draft author:** JunoClaw contributors (<https://github.com/juno-claw/junoclaw>)

## Summary

Add three host functions to `cosmwasm-vm` and `wasmvm` that expose the
BN254 (alt_bn128) curve primitives that Ethereum has had since the
Byzantium fork:

| Host function              | Ethereum precompile | Input | Output | Gas (SDK)              |
|----------------------------|---------------------|-------|--------|------------------------|
| `bn254_add`                | `0x06` ECADD        | 128 B | 64 B   | 150                    |
| `bn254_scalar_mul`         | `0x07` ECMUL        | 96 B  | 64 B   | 6,000                  |
| `bn254_pairing_equality`   | `0x08` ECPAIRING    | 192·N B | bool | 45,000 + 34,000·N     |

The byte layout and gas schedule are deliberately lifted from
EIP-196 / EIP-197 / EIP-1108 so that existing Groth16 tooling
(`snarkjs`, `circom`, `gnark`, `ark-groth16`) can target CosmWasm
chains with **zero adaptation**.

## Why this PR

Issue #751 lists BN254 pairing primitives as "Bonus Points." This PR
turns that item into concrete, reviewed, benchmarked code.

The immediate concrete user is the JunoClaw zk-verifier contract
(`juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`
on uni-7). Today it verifies Groth16 proofs in pure CosmWasm and burns
**371 486 gas** per `VerifyProof`. On a patched chain, the same
verification lands at **~187 000 gas** — a **~2× reduction** that
matches Ethereum's precompile cost curve exactly.

More broadly: no pure Cosmos / CosmWasm chain has native BN254 pairing
support today. Sui has it. Ethereum has had it since 2017. This PR
lets CosmWasm catch up via a well-understood primitive, with no new
curves and no new supply-chain dependencies.

## What's in this PR

### `packages/crypto-bn254/` (new)

Standalone `no_std`-friendly Rust crate implementing the three
operations against `ark-bn254 0.5`:

- `src/bn254.rs` — public API + backend plumbing
- `src/errors.rs` — `Bn254Error` taxonomy (matches BLS12-381 style)
- `src/gas.rs` — gas constants, expressed in VM gas (100× SDK)
- `tests/vectors.rs` — 9 conformance tests covering round-trip,
  bilinearity, canonical-encoding rejection, and subgroup rejection
- `benches/bn254.rs` — Criterion benchmarks feeding the gas ceilings

### `packages/vm/` (imports.rs + instance.rs)

Three new `do_bn254_*` entry points with gas hooks, following the same
pattern as `do_bls12_381_pairing_equality`. Pairing input is capped at
64 pairs (12 288 bytes) to bound worst-case runtime.

### `packages/std/` (imports.rs + traits.rs)

Guest-side `extern "C"` declarations and `Api::bn254_*` trait methods
behind a new `cosmwasm_2_3` feature flag. `MockApi` implementation
included so contract unit tests work out of the box.

### `libwasmvm/` (new: `src/bn254_ffi.rs`)

`no_mangle` C-ABI shims that route the `ByteSliceView` →
`UnmanagedVector` convention through `cosmwasm-crypto-bn254`. Go-side
wrappers in `internal/api/bn254.go` and public-surface functions in
`lib_libwasmvm.go`.

## Determinism

- Points not on the curve → `NotOnCurve`
- G2 cofactor-torsion points → `NotInSubgroup`
  (G1 has cofactor 1 on BN254, no G1 subgroup check needed)
- Non-canonical coordinates (≥ p) → `InvalidFieldElement`
- Scalar ≥ r is silently reduced (matches EIP-196 ECMUL semantics)
- Empty pairing input → `Ok(true)` (matches EIP-197)
- No RNG, no wall-clock reads, no threads

The differential test in the benchmark harness runs 1 000 random
Groth16 proofs through the pure-Wasm verifier AND through the
precompile-backed verifier and asserts identical accept/reject
decisions. A reviewer can reproduce it via the devnet recipe in
`wasmvm-fork/BUILD_AND_TEST.md`.

## Gas methodology

The constants in `packages/crypto-bn254/src/gas.rs` mirror EIP-1108 at
a 100× multiplier (matching wasmd's `DefaultGasMultiplier = 100` and
the existing BLS12-381 metric).

Criterion numbers on a 2023 M2 Pro (release, `ark-bn254 0.5` with the
`curve` feature, no asm tuning):

| Benchmark                            | Median runtime | Headroom vs budget |
|--------------------------------------|----------------|---------------------|
| `bn254_add (G + G)`                  | ~4 µs          | > 1000×            |
| `bn254_scalar_mul (k·G, k=u64::MAX)` | ~85 µs         | > 100×             |
| `bn254_pairing_equality (3 pairs)`   | ~2.2 ms        | ~66×               |

The 66× headroom on pairing is the binding constraint — tightening it
further is a follow-up PR once there's agreement on a methodology for
deriving CosmWasm gas from native runtime.

## Review checklist

- [ ] `Fq::from_bigint` None-path surfaces `InvalidFieldElement`.
- [ ] `is_on_curve` called before any arithmetic.
- [ ] `is_in_correct_subgroup_assuming_on_curve` called after on-curve
      for every G2 decode.
- [ ] EIP-196/197 conformance vectors pass bit-for-bit (`tests/vectors.rs`).
- [ ] Gas is charged before the operation, never after.
- [ ] Empty pairing input returns true.
- [ ] Differential test on 1 000 random Groth16 proofs shows identical
      accept/reject against `ark-groth16`'s native verifier.

## Non-goals

- We do **not** add `bn254_hash_to_g1`. Circom circuits encode their
  own hashing and use ECMUL on a point they've already constructed;
  hash-to-curve is a separate can of worms (domain separation, draft
  RFC) best addressed in a dedicated follow-up.
- We do **not** add BLS signature verification on BN254. The existing
  `bls12_381_pairing_equality` covers the aggregate-signature use case
  on a stronger curve.
- We do **not** bump `cosmwasm-std`'s major version. The `Api::bn254_*`
  methods live under a `cosmwasm_2_3` feature so existing contracts
  compile unchanged.

## Breaking changes

None. This is purely additive behind a new feature gate.

## Test plan

1. `cargo test -p cosmwasm-crypto-bn254` — unit + conformance vectors
2. `cargo test -p cosmwasm-vm` — VM integration, including gas metering
3. `cargo test --target wasm32-unknown-unknown` on contract test crate
4. `make test` in the `wasmvm` Go module — cgo FFI sanity
5. `./devnet/scripts/run-devnet.sh && ./devnet/scripts/benchmark.sh` —
   end-to-end gas measurement on a single-validator devnet
6. Differential test: 1 000 random Groth16 proofs

Full reproducibility contract: `wasmvm-fork/BUILD_AND_TEST.md`.

## Follow-ups

- `bn254_hash_to_g1` + `bn254_hash_to_g2` with explicit domain separation
- Gas-schedule tightening once there's consensus on the native-time
  methodology
- Companion `x/wasm` capability flag so chains can advertise
  `bn254` via `Capabilities`

## Credit

This PR productizes work done by JunoClaw contributors while building
an on-chain ZK-verified attestation pipeline for verifiable AI agents.
The gas analysis that motivated the work is in
[`docs/BN254_PRECOMPILE_CASE.md`][case] on the JunoClaw repo; the
governance-track companion is [`docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md`][prop].

[case]: https://github.com/juno-claw/junoclaw/blob/main/docs/BN254_PRECOMPILE_CASE.md
[prop]: https://github.com/juno-claw/junoclaw/blob/main/docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md

cc @ethanfrey @webmaster128 (CosmWasm), @Reecepbcups @jakehartnell (Juno)
