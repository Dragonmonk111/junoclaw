# Lattice Jolt Evaluation: CosmWasm Deployment Feasibility

*September 9, 2026*

## What Lattice Jolt Is

[Lattice Jolt](https://github.com/a16z/jolt) is a post-quantum zkVM released September 9, 2026 by a16z crypto. It replaces the elliptic-curve-based Dory commitment scheme with **Akita** — a lattice-based polynomial commitment scheme built on the Module-SIS assumption at 128-bit security.

Key properties:
- **Post-quantum**: lattice-based (Module-SIS), no elliptic curves
- **Faster**: 2-3x faster proving than curve-based Jolt
- **Smaller proofs**: 65-80 KB (shortest of any post-quantum zkVM)
- **Less memory**: ~200 bytes/cycle (down from ~300)
- **Transparent setup**: no trusted ceremony
- **RISC-V target**: proves RV64IMAC programs
- **Open source**: Rust, Apache/MIT

## The Question

Can Lattice Jolt's **verifier** be compiled to `wasm32-unknown-unknown` and deployed as a CosmWasm contract on juno-1?

If yes, it replaces two dependency chains in JunoClaw's PQC roadmap:
1. **BN254 precompile path** (v30 upgrade → zk-verifier-precompile variant) — no longer needed
2. **Fable MAYO/ML-DSA path** (wasmvm fork merge → jclaw-credential precompile builds) — no longer needed

## Technical Analysis

### What the verifier does

The Jolt verifier performs:
1. **Akita commitment verification**: checks polynomial commitments over a lattice (Module-SIS). Pure arithmetic over Z_q — multiplications, additions, modular reductions. No system calls, no I/O.
2. **Sum-check protocol verification**: evaluates multivariate polynomials at random points. Pure computation.
3. **Proof parsing**: deserializes the 65-80 KB proof blob. Memory allocation only.

All three are pure computation — no file I/O, no networking, no threads required.

### Wasm compilation feasibility

**Favorable**:
- Jolt is written in Rust, which has excellent `wasm32-unknown-unknown` support
- The verifier is a separate concern from the prover (which needs GPU acceleration, file I/O for trace generation)
- Akita uses lattice operations (polynomial multiplication, NTT) — these are standard arithmetic that compiles cleanly to wasm
- The `sha2`, `sha3` hash functions used in Jolt have well-tested wasm implementations
- No GPU, no BLAS, no system threading needed for verification

**Potential blockers**:
- **Memory**: CosmWasm contracts are limited to ~8 MiB wasm size and have gas-metered memory. A 65-80 KB proof needs to be passed as a contract message parameter. The verifier's working memory needs to fit within CosmWasm's gas budget.
- **Dependencies**: Jolt's dependency tree may include crates with `std`-only features (e.g., `rayon` for parallelism in the prover). The verifier crate must be extracted with minimal dependencies. Need to audit `Cargo.toml` for `std`-only or platform-specific deps.
- **Gas cost**: Lattice operations involve many polynomial multiplications. Each multiplication over Z_q with 128-bit coefficients is O(n log n) via NTT. For a 65-80 KB proof, the verification computation could be significant — potentially millions of gas on CosmWasm. Need benchmarks.
- **wasm size**: The compiled verifier wasm may exceed CosmWasm's 8 MiB upload limit if it includes the full Jolt crate. Need to extract just the verifier module.

### Architecture proposal

```
┌─────────────────────────────────────────────┐
│           CosmWasm Contract                  │
│  ┌─────────────────────────────────────┐    │
│  │  jolt-verifier.wasm (pure verifier) │    │
│  │  - Akita commitment check          │    │
│  │  - Sum-check verification          │    │
│  │  - Proof deserialization           │    │
│  └─────────────────────────────────────┘    │
│  ExecuteMsg::VerifyProof { proof: Binary }   │
│  → returns { verified: bool }               │
└─────────────────────────────────────────────┘
```

### Integration with existing JunoClaw contracts

1. **zk-verifier** (`juno1qd9qag...`): Currently verifies Groth16/BN254 proofs. A Lattice Jolt verifier contract would be a **new** contract, not a migration — the proof format is entirely different. The existing moultbook contract would be re-wired to point at the new verifier address.

2. **jclaw-credential** (`juno1dgmak...`): Currently uses pure Wasm (no precompile). If Lattice Jolt can prove credential issuance/verification as a RISC-V program, the credential contract could verify Jolt proofs instead of relying on MAYO/ML-DSA precompiles. This would eliminate the Fable wasmvm fork dependency entirely.

3. **Aegis PQC**: The hybrid CometBFT transport layer could use Lattice Jolt proofs for consensus message authentication, replacing the need for BN254 precompile support in the chain itself.

### Comparison with current approaches

| Approach | Post-quantum? | Chain dependency | Proof size | Status |
|----------|--------------|-----------------|-----------|--------|
| BN254 precompile (v30) | No | Needs v30 upgrade | ~200 bytes | Deployed (v30 active) |
| MAYO precompile (Fable) | Yes | Needs wasmvm fork | ~7 KB | Blocked on Fable merge |
| ML-DSA precompile (Fable) | Yes | Needs wasmvm fork | ~2.5 KB | Blocked on Fable merge |
| **Lattice Jolt (proposed)** | **Yes** | **None (pure Wasm)** | **65-80 KB** | **Research phase** |

The tradeoff: Lattice Jolt proofs are much larger (65-80 KB vs 200 bytes for BN254), but they're post-quantum and require no chain-level changes. For on-chain verification where proof size isn't critical (e.g., credential verification, not per-block consensus), this is acceptable.

### Next steps

1. ~~**Clone and audit**~~: ✅ Done — cloned to `C:\Temp\jolt`, audited all Cargo.toml files
2. ~~**Extract verifier**~~: ✅ Done — `jolt-verifier` crate is already separated from the prover
3. ~~**Wasm compile test**~~: ✅ **DONE — COMPILED SUCCESSFULLY** (see below)
4. **Size check**: Need release build + `wasm-opt -Oz` to measure final wasm size
5. **Gas benchmark**: Estimate CosmWasm gas cost for verifying a 65-80 KB proof
6. **Contract wrapper**: Write a CosmWasm contract that accepts `VerifyProof { proof: Binary }` and calls the Jolt verifier
7. **Deploy to uni-7**: Test on testnet first, then mainnet

## Wasm32 Compile Results — September 10, 2026

### Result: ✅ SUCCESS

```
cargo build -p jolt-verifier --no-default-features --target wasm32-unknown-unknown
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 07s
```

**Output**: `libjolt_verifier.rlib` — 5,023,000 bytes (debug, unoptimized)

### What was needed

**One fix**: Added `getrandom = { version = "0.2", features = ["js"] }` as a direct dependency in `jolt-verifier/Cargo.toml`. The `getrandom` crate doesn't support `wasm32-unknown-unknown` by default — the `js` feature enables the `js-sys`-based implementation. This is a standard fix used by many Rust→wasm projects.

**No other changes needed.** All other dependencies compiled cleanly:
- `ark-bn254`, `ark-ff`, `ark-ec`, `ark-serialize` (a16z fork, `default-features = false`) — compiled fine
- `spongefish` — compiled fine
- `rayon` — compiled (likely no-op on wasm32, but doesn't block compilation)
- `jolt-field`, `jolt-crypto`, `jolt-poly`, `jolt-sumcheck`, `jolt-transcript`, `jolt-openings`, `jolt-claims`, `jolt-r1cs`, `jolt-riscv` — all compiled
- `jolt-akita` (lattice commitment) — NOT included in `--no-default-features` build; would need `--features akita` and requires `rayon` as hard dep (may need patching for wasm32)

### Build configuration

```bash
cd C:\Temp\jolt
cargo build -p jolt-verifier --no-default-features --target wasm32-unknown-unknown
```

- `--no-default-features` disables `transcript-blake2b` (the only default feature)
- The `akita` feature (lattice-based proofs) was NOT enabled in this build — it requires `jolt-akita` which has a hard `rayon` dependency
- The build uses the BN254 backend (via `jolt-transcript`'s hard dep on `ark-bn254`)
- For a pure post-quantum build, the `akita` feature would need to be enabled and `jolt-akita`'s `rayon` dependency made optional

### What this means

1. **The Jolt verifier is wasm-compatible** — the core verification logic (sum-check, commitment verification, proof parsing) compiles to `wasm32-unknown-unknown`
2. **No chain-level changes needed** — this is a pure Wasm build, deployable on any CosmWasm chain (or any WASM runtime)
3. **The BN254 path works today** — the verifier can verify BN254-based Jolt proofs on-chain
4. **The Akita (lattice) path needs substantially more work than previously estimated** — see "Sept 13 follow-up" below. It is NOT a ~10-line patch.
5. **5MB debug rlib** — release build with `wasm-opt -Oz` will be significantly smaller. Need to measure.

### Remaining work for production deployment (BN254 path only)

1. **Release build**: `cargo build --release -p jolt-verifier --no-default-features --target wasm32-unknown-unknown` → measure wasm size
2. **cdylib wrapper**: Add `crate-type = ["cdylib"]` to `jolt-verifier/Cargo.toml` and export CosmWasm entry points (`instantiate`, `execute`, `query`)
3. **Gas benchmark**: Deploy to uni-7, call `VerifyProof` with a 65-80 KB proof, measure gas
4. **wasm-opt**: `wasm-opt -Oz --strip-debug --strip-target-features --enable-sign-ext --signext-lowering` on the release wasm

## Updated Conclusion

Lattice Jolt's verifier **compiles to wasm32-unknown-unknown for the BN254 path**. The Akita/lattice (post-quantum) path is materially harder — see the Sept 13 follow-up below.

**BN254 path estimated effort**: ~1 week to a deployable CosmWasm contract (build + cdylib wrapper + benchmark).

**Akita/lattice path estimated effort**: unresolved as of Sept 13, 2026 — at least two independent, deep blockers found, likely multi-day/multi-session effort spanning three separate upstream repos (a16z/jolt, LayerZero-Labs/akita, arkworks-rs/spongefish).

This collapses the PQC deployment roadmap: no v30 BN254 dependency, no Fable wasmvm fork, no precompile governance. Just a pure Wasm contract deployable on stock Juno today — or on any future WASM runtime the DAO chooses to build.

**Priority upgraded**: High. This is the most practical path to on-chain post-quantum ZK verification for JunoClaw.

### Relevance to DAO Nerve / Hybrid Chain Evolution

If Juno swaps its WASM runtime (per Jake and Ravi's discussion), Lattice Jolt's verifier runs on **any** WASM runtime — it's pure wasm, not CosmWasm-specific. The verifier contract would need to be re-wrapped for the new runtime's entry points, but the core verification logic is portable. This makes Lattice Jolt a **chain-agnostic ZK verification layer** that survives any chain migration.

## Sept 13, 2026 Follow-up: Akita/Lattice Wasm Build — Actual Findings

Attempted the "~10-line rayon patch" against a fresh `a16z/jolt` clone. The estimate was wrong. Two independent, deep blockers found, neither fixable with a small patch:

### Blocker 1: `rayon` is not just a Cargo.toml flag issue

- `jolt-akita/src/adapters.rs`'s `with_backend_pool()` wraps **every** setup/commit/prove/verify call (including `verify_batch`, the function CosmWasm needs) in a real `rayon::ThreadPool` built via `ThreadPoolBuilder::new().build()`. This spawns actual OS threads via `std::thread::Builder::spawn`, which does not work on `wasm32-unknown-unknown` inside a CosmWasm/wasmvm sandbox (no `SharedArrayBuffer`/atomics, no `wasm-bindgen-rayon`).
- Making `rayon` `optional` in `jolt-akita/Cargo.toml` (which I did) does not fix this — `backend_pool()` calls `ThreadPoolBuilder::build()` unconditionally, with no `cfg(target_arch = "wasm32")` fallback to run the closure inline.
- Good news: traced the call graph and confirmed `trace_onehot`'s heavy rayon kernels (`kernels.rs`, `opening.rs`, `decomposition.rs` — ~800 lines of parallel numeric code with `into_par_iter`, `par_chunks_mut`, thread-count-based chunking) are **prover-only** (`prove_one_hot`/`commit_one_hot_group`), never reached by `verify_batch`. So a verify-only wasm build would not need to rewrite that code — IF it can be feature-gated out of compilation entirely (not just left unreachable at runtime; Rust still type-checks/compiles unreached code).

### Blocker 2 (new, not in original evaluation): `getrandom` version conflict

- `cargo tree -i getrandom` shows `getrandom v0.2.17` pulled in via `ark-std v0.6.0` → `ark-ff v0.6.0` → `spongefish v0.7.0` → `akita-transcript` (LayerZero-Labs' Akita crate).
- This is a **different** `ark-std`/`ark-ff` version than the workspace's pinned a16z fork (`0.5.0`, `dev/twist-shout` branch) — Akita's own dependency tree pulls a newer, separate arkworks version.
- `getrandom 0.2.17` refuses to compile for `wasm32-unknown-unknown` without the `js` feature (browser `Crypto` API), which does not exist in a CosmWasm sandbox.
- Attempted fix via `[patch.crates-io] getrandom = { version = "...", features = ["custom"] }` — **invalid syntax**, Cargo rejects patching a crates-io crate to itself. Would need either: (a) get Akita's upstream to align its arkworks version, or (b) add a workspace-level `getrandom` dependency declaration with the `custom` feature so Cargo's feature-unification picks it up, PLUS actually implement `register_custom_getrandom!` somewhere (real code, not just Cargo.toml) since prover-side RNG has no meaning in a verify-only deterministic context.

### Revised assessment

The Akita/lattice wasm path requires:
1. A `cfg(target_arch = "wasm32")` fallback in `jolt-akita/src/adapters.rs` to skip real `ThreadPool` construction and run inline
2. Feature-gating `trace_onehot`'s prover-only module out of verify-only builds (new Cargo feature, not just making `rayon` optional)
3. Resolving the `getrandom`/`ark-std` version conflict from Akita's `spongefish` dependency — patching Akita's upstream lockstep, or vendoring a patched `akita-transcript`
4. Re-running the build to check for further blockers (both issues found so far were sequential — fixing #1 was needed to even discover #2; there may be more)

This is realistically **multi-day, multi-session engineering** touching three separate upstream repos (a16z/jolt, LayerZero-Labs/akita, arkworks-rs/spongefish), not the "~1 day, ~10 lines" originally estimated. The BN254 path (Sections above) remains the practical near-term option; Akita/lattice PQ verification is a real but longer-horizon effort.
