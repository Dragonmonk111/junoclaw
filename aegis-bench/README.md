# aegis-bench

Decision model for **Project Aegis** (`docs/PROJECT_AEGIS_JUNO_FULL_PQC.md` §5.1):
ML-DSA-44 vs ML-DSA-65 at the Juno consensus root.

It answers the one question that decides the consensus parameter set: *how much
does each option cost in signature bandwidth and block-data growth?* — because
every block's commit carries one signature per validator (`N × sig_size`).

## Run (offline, dependency-free)

```
cargo run            # from this directory
```

Prints:
- fixed primitive sizes (Ed25519 / secp256k1 / ML-DSA-44 / ML-DSA-65 + hybrids),
- commit signature payload per block at 50 / 100 / 150 validators,
- block-data growth per block / day / year at N=100,
- the ML-DSA-44 vs 65 delta and recommendation.

These are spec constants (FIPS 204 / RFC 8032 / SEC) plus arithmetic, so the
default build needs no crates and no network.

## Run with real timing

```
cargo run --features timing
```

Adds measured keygen / sign / **verify** wall-clock time for ML-DSA-44 and 65
via the pure-Rust [`fips204`](https://crates.io/crates/fips204) crate (one-time
crates.io fetch). Verify time is the consensus-relevant figure.

## What this does *not* do

It does **not** measure on-chain gas. That is Phase B: add `ml_dsa_verify` to the
`wasmvm-fork` beside `mayo_verify` and benchmark on the `junoclaw-bn254-1`
devnet with the existing harness ("prove on the Juno fork first").

This crate is standalone (its own `[workspace]`) and is not part of the root
Cargo workspace.
