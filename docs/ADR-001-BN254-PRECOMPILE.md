# ADR-001: BN254 (alt_bn128) host functions for CosmWasm

**Status:** Proposed — pending CosmWasm maintainer feedback (Phase 1 of [POST_VOTE_EXECUTION_PLAN](./POST_VOTE_EXECUTION_PLAN.md))
**Date proposed:** 2026-05-05
**On-chain mandate:** Juno governance proposal [#374](https://ping.pub/juno/gov/374) — passed ~80% Yes, 22% Abstain, 0.003% No-with-Veto, 44.05% turnout
**Authors:** VairagyaNodes (deployer), Cascade (coding agent)
**Reviewers (target):** Ethan Frey, Simon Warta (CosmWasm core); Dimi (Juno validator, optional)

---

## Context

CosmWasm contracts that verify zero-knowledge proofs today rely on pure-Wasm implementations of elliptic-curve arithmetic and pairing operations. For BN254 (alt_bn128) — the curve used by Groth16 proofs from `snarkjs`, `circom`, `gnark`, and `ark-groth16` — this means executing every field operation through the WebAssembly interpreter.

The cost is substantial. Measured on Juno mainnet (uni-7) and reproduced on devnet:

- **Pure-Wasm Groth16 verification (1 public input, SquareCircuit):** 370,719 SDK gas per `VerifyProof`
- **Reference baseline (a SHA-256 store, no curve work):** ~200,000 SDK gas

Almost half a million gas to verify a proof that, on Ethereum, is verified for ~150,000 gas via the EIP-1108 precompile. For a use case where every off-chain agent task should produce a proof and have that proof verified, this gas asymmetry is the difference between "we sample 1 in 10 proofs" and "we verify every proof."

The proposed precompile path lands at ~187,000 SDK gas (3-pair circuit) to ~223,000 (4-pair), based on EIP-1108 constants × 100 (matching wasmd's existing gas-multiplier convention). This is a ~2× reduction, comparable to the ratio Ethereum saw post-EIP-1108.

The Juno community signaled support for this work via on-chain proposal #374, which closed on 2026-05-05 with ~80% Yes. The proposal is signaling-only — it commits the Juno chain to nothing, and crucially, it commits CosmWasm to nothing. It is evidence of demand, not authority.

---

## Decision

Add three host functions to `cosmwasm-vm`, with guest-side declarations in `cosmwasm-std`, behind a new `cosmwasm_2_3` feature flag:

| Host function              | Ethereum precompile | Input    | Output | SDK gas             |
|----------------------------|---------------------|----------|--------|---------------------|
| `bn254_add`                | `0x06` ECADD        | 128 B    | 64 B   | 150                 |
| `bn254_scalar_mul`         | `0x07` ECMUL        | 96 B     | 64 B   | 6,000               |
| `bn254_pairing_equality`   | `0x08` ECPAIRING    | 192·N B  | bool   | 45,000 + 34,000·N   |

Add a new chain capability string: `bn254`. Chains that advertise this capability accept contracts that import the new host functions; chains that don't, reject them at instantiation. Existing contracts compile unchanged.

Implement the curve operations in a new `no_std`-friendly Rust crate, `cosmwasm-crypto-bn254`, layered over `ark-bn254 0.5`. The crate ships with conformance vectors lifted from EIP-196 / EIP-197 / EIP-1108 and Criterion benchmarks that derive the gas constants empirically.

Wire the host functions through `wasmvm` via `libwasmvm/src/bn254_ffi.rs` (CGo shims) and `internal/api/bn254.go` (Go-side wrappers), following the same pattern as the existing BLS12-381 entry points.

---

## Alternatives considered

### Alternative A: BLS12-381 only

CosmWasm already has `bls12_381_pairing_equality`. Why not require all ZK tooling to migrate to BLS12-381?

**Rejected.** The existing Groth16 ecosystem is overwhelmingly BN254. `snarkjs`, `circom`, `gnark` default to BN254. Asking projects to retool circuits for BLS12-381 is a months-long migration with subtle correctness risks. The point of a precompile is to lower friction; making chains pay BLS12-381 prover overhead instead of accepting BN254 verifier work is a net loss.

BLS12-381 remains the right choice for new BLS aggregate-signature work (stronger curve, longer-term security margin). BN254 remains the right choice for verifying existing Groth16 proofs.

### Alternative B: pure-Wasm arkworks (status quo)

Keep the existing pure-Wasm implementation; ship it as a CosmWasm helper crate.

**Rejected.** The 370,719 gas figure is fundamental to the Wasm interpreter, not to our specific implementation. Optimizing the host doesn't help — the cost is in the execution path, not the algorithm. The only way to bring it down is native execution.

### Alternative C: Cosmos SDK-level module (`x/zk-verifier`)

Implement BN254 verification in a chain-level Cosmos SDK module instead of CosmWasm. Contracts would call `MsgVerifyProof` cross-module.

**Rejected on three grounds:**

1. **Composability.** Contracts that need to verify a proof inline (escrow release conditional on proof validity) can't easily call out to another module synchronously without significant `wasmd` plumbing. A precompile keeps the verification call inside the contract's own execution context.
2. **Generality.** A precompile benefits every CosmWasm chain. An `x/zk-verifier` benefits only the chain that adopts it. The precompile path scales the work across the ecosystem.
3. **Maintenance surface.** Cosmos SDK modules are tightly coupled to chain upgrades. A CosmWasm host function rides on the existing wasmvm cadence. Less maintenance burden for both us and Juno's validators.

### Alternative D: native Cosmos SDK precompile + EVM ABI

Some Cosmos chains (Berachain, Sei) ship with EVM precompiles directly. Mirror that approach.

**Rejected.** Juno has no EVM. Adding one to host four precompiles is the wrong order of magnitude. CosmWasm is Juno's execution layer; the host functions belong there.

---

## Gas methodology

### Schedule derivation

Constants in `cosmwasm-crypto-bn254/src/gas.rs` mirror EIP-1108 (Istanbul fork) at a 100× multiplier, matching:

- wasmd's `DefaultGasMultiplier = 100`
- The existing `bls12_381_pairing_equality` gas accounting

| Operation | EIP-1108 constant | Wasm gas (×100) | SDK gas (÷100) |
|-----------|-------------------|-----------------|----------------|
| `bn254_add` | 150 | 15,000 | 150 |
| `bn254_scalar_mul` | 6,000 | 600,000 | 6,000 |
| `bn254_pairing_equality` (base) | 45,000 | 4,500,000 | 45,000 |
| `bn254_pairing_equality` (per pair) | 34,000 | 3,400,000 | 34,000 |

### Headroom check (criterion benchmarks vs schedule)

Wall-clock measurements on a 2023 M2 Pro, `release` profile, `ark-bn254 0.5` with `curve` feature, no manual asm tuning. SDK-gas equivalent uses `1 ns ≈ 1 wasm gas / 100 = 0.01 SDK gas`.

| Primitive | Wall-clock (ns) | Equivalent SDK gas | Scheduled SDK gas | Headroom |
|---|---:|---:|---:|---:|
| `bn254_add` | 4,340 | ~44 | 150 | 3.4× |
| `bn254_scalar_mul` | 44,264 | ~443 | 6,000 | 13.5× |
| `bn254_pairing_equality` (3 pairs) | 2,014,971 | ~20,150 | 147,000 | 7.3× |

The 3.4× headroom on `bn254_add` is the binding constraint. Tightening it is a follow-up change once there's consensus on the methodology for deriving CosmWasm gas from native runtime — not part of this proposal.

### Why headroom matters

Hardware varies. Validators on a server without BMI/ADX may run arkworks 3× slower than the M2 Pro reference. The headroom absorbs that variance without requiring a chain upgrade to recalibrate gas. Tighter schedules look better on paper but invite upgrade churn.

---

## Security considerations

### Curve correctness

- **Points not on curve** → return `Err(NotOnCurve)` before any arithmetic
- **G2 cofactor-torsion points** → after `is_on_curve`, call `is_in_correct_subgroup_assuming_on_curve`. G1 has cofactor 1, no subgroup check needed.
- **Non-canonical coordinates (≥ p)** → return `Err(InvalidFieldElement)`
- **Scalar ≥ r** → reduce silently (matches EIP-196 ECMUL semantics; specifying error here would diverge from Ethereum)
- **Empty pairing input** → return `Ok(true)` (matches EIP-197)

### Determinism

- No RNG anywhere in the host path
- No wall-clock reads
- No threads
- Single-threaded `ark-bn254` operations are bit-deterministic across platforms

### Differential testing

Conformance is asserted by:

1. **Static vectors** — 24+ test fixtures from Ethereum's `tests/GeneralStateTests/stPreCompiledContracts/`, covering positive and negative cases for ECADD, ECMUL, ECPAIRING.
2. **Dynamic differential** — 1,000 randomly generated Groth16 proofs run through both the pure-Wasm verifier (`ark-groth16` from a contract) and the precompile-backed verifier. Asserts identical accept/reject decisions on every proof.

The differential test is mandatory before each release of the precompile crate and is included in the upstream PR's CI matrix.

### Supply-chain

- `ark-bn254 0.5` is the same crate that audited Cosmos projects (Drand, Penumbra) already depend on
- `cargo deny check` runs in CI to flag advisories on transitive dependencies
- The crate has zero `unsafe` blocks outside the `#[no_mangle] extern "C"` shims that bridge to libwasmvm

### Scope boundary

- We **do not** add `bn254_hash_to_g1` or `bn254_hash_to_g2`. Hash-to-curve has its own RFC track and domain-separation conventions; it deserves a separate proposal.
- We **do not** add BN254 BLS aggregate-signature verification. `bls12_381_pairing_equality` already covers that on a stronger curve.
- We **do not** modify any existing host function. The change is purely additive.

---

## Migration

### For chains

Chains opt in by adding `bn254` to their accepted-capabilities set. On Juno, this is the entire content of the v30 chain-upgrade handler:

```go
func CreateUpgradeHandler(...) upgradetypes.UpgradeHandler {
    return func(ctx sdk.Context, plan types.Plan, fromVM module.VersionMap) (module.VersionMap, error) {
        keepers.WasmKeeper.SetParams(ctx, wasmtypes.Params{
            ...,
            // Add "bn254" to accepted capabilities
        })
        return mm.RunMigrations(ctx, configurator, fromVM)
    }
}
```

The handler does **two** things and only two: bump the wasmvm dependency to one that includes BN254 host imports, and register the capability. No state migrations. No param changes. No cleanups.

### For contracts

Contracts opt in by enabling the `cosmwasm_2_3` feature flag in their `Cargo.toml`:

```toml
[dependencies]
cosmwasm-std = { version = "2.3", features = ["bn254", "cosmwasm_2_3"] }
```

Contracts that don't enable the feature compile and run unchanged on chains with the new wasmvm. Contracts that do enable the feature **must** be deployed only on chains that advertise the `bn254` capability — otherwise instantiation fails fast at upload time with a clear error.

### For wasmd

`x/wasm` consumers gain access to the new entry points by upgrading their `wasmvm` dependency. No `x/wasm` source change required.

---

## Open questions (for maintainer review)

These are deliberately structured as questions, not assertions. Each is something we'd happily change based on maintainer preference:

1. **Module path.** Crate named `cosmwasm-crypto-bn254` and lives at `packages/crypto-bn254/`. Acceptable, or do you prefer a different layout?
2. **Feature flag.** New flag `cosmwasm_2_3`. Does this match your release-train convention, or should it be `bn254` directly, or something else?
3. **Capability string.** Chain capability `bn254`. Acceptable name?
4. **Function naming.** Three function names: `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality`. Match the BLS12-381 naming style; happy to revise to whatever shape fits cleanest.
5. **Pairing input cap.** We propose capping pairing input at 64 pairs (12,288 bytes) to bound worst-case runtime. Is the cap right, too tight, too loose?
6. **Empty-pairing semantics.** Returning `Ok(true)` for empty input (EIP-197 conformance). Confirm preference?
7. **Gas methodology.** EIP-1108 × 100 multiplier. Sound, or do you have a different derivation in mind for new precompiles?
8. **Differential test.** Should the 1,000-proof differential live in `cosmwasm-crypto-bn254/tests/` or in `cosmwasm-vm/tests/` (i.e., is it a crate property or a VM property)?

---

## Implications

### For Juno

If accepted upstream and shipped in a wasmvm release, Juno schedules a v30 chain upgrade that bumps the wasmvm dependency and registers the `bn254` capability. JunoClaw's `zk-verifier` contract is redeployed (or migrated) with the precompile feature flag enabled, dropping `VerifyProof` cost from ~371k gas to ~200k. Every on-chain agent task gets verified, not sampled.

### For other Cosmos chains

Any chain on the new wasmvm release can opt in by registering the capability. Specifically:

- **Stargaze, Osmosis, Neutron**, et al. that have CosmWasm contracts doing ZK verification (mostly NFT-credential and bridge-light-client work) get the same gas reduction.
- **Provenance** and other regulated-asset chains get cheaper Groth16-based KYC/credential verification.

### For CosmWasm

CosmWasm catches up to Ethereum's BN254 precompile parity, which has been on the "Bonus Points" list of issue #751 for years. The PR is intentionally minimal so it lands without disrupting v2.x compatibility.

### For the wider ZK ecosystem

`snarkjs`, `circom`, `gnark`, and `ark-groth16` users target CosmWasm chains with the same proof artifacts they use on Ethereum. No tool changes. No circuit changes. The chain-by-chain tax on ZK verification falls.

---

## What this ADR explicitly does NOT decide

- The exact tag of `wasmvm` we'll target — that depends on the rebase outcome in [POST_VOTE_EXECUTION_PLAN](./POST_VOTE_EXECUTION_PLAN.md) Phase 0.1.
- Whether Dimi co-signs the v30 chain upgrade — that's an offer, not a partnership; we earn it via clean execution.
- Whether other Cosmos chains will adopt — that's their decision, not ours.
- The follow-up roadmap (`bn254_hash_to_curve`, BLS12-381 tightening) — separate ADRs.

---

## References

- **On-chain mandate:** Juno proposal [#374](https://ping.pub/juno/gov/374)
- **Companion proposal text:** [`JUNO_GOVERNANCE_PROPOSAL_BN254.md`](./JUNO_GOVERNANCE_PROPOSAL_BN254.md)
- **Technical case (long-form):** [`BN254_PRECOMPILE_CASE.md`](./BN254_PRECOMPILE_CASE.md)
- **Measured baseline:** [`BN254_BENCHMARK_RESULTS.md`](./BN254_BENCHMARK_RESULTS.md)
- **Algebraic projection:** [`BN254_BENCHMARK_PROJECTED.md`](./BN254_BENCHMARK_PROJECTED.md)
- **Build & test recipe:** [`../wasmvm-fork/BUILD_AND_TEST.md`](../wasmvm-fork/BUILD_AND_TEST.md)
- **Upstream issue #751 (meta):** [CosmWasm/cosmwasm#751](https://github.com/CosmWasm/cosmwasm/issues/751)
- **EIP-196:** [eips.ethereum.org/EIPS/eip-196](https://eips.ethereum.org/EIPS/eip-196)
- **EIP-197:** [eips.ethereum.org/EIPS/eip-197](https://eips.ethereum.org/EIPS/eip-197)
- **EIP-1108:** [eips.ethereum.org/EIPS/eip-1108](https://eips.ethereum.org/EIPS/eip-1108)

---

*Apache-2.0. Comments and revisions welcome via PR against `docs/ADR-001-BN254-PRECOMPILE.md` on `Dragonmonk111/junoclaw`.*
