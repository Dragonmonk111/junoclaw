# MAYO Precompile Plan (v2 — Multi-Variant + Upstream + Commonware Backdoor)

## Goal
Add a **multi-variant** `mayo_verify` host function to the BN254-patched devnet,
reducing on-chain MAYO verification gas from ~356k (MAYO-2 wasm) to ~50-100k
(native), at all three NIST levels (MAYO-2/L1, MAYO-3/L3, MAYO-5/L5).

Then drive it on two strategic rails:
- **Rail A (Juno-native):** upstream the host function as a Juno governance
  proposal — making Juno the first PQC-native CosmWasm chain and removing the
  "migrate for PQC" argument entirely.
- **Rail B (Commonware backdoor):** keep `junoclaw-mayo-verify` the portable,
  dependency-free verifier so the same code drops into a Commonware runtime
  if the ecosystem ever moves. We win either outcome.

## Context
We already have:
- `junoclaw-bn254-1` devnet with BN254 pairing precompile
- `wasmvm-fork/` with the host-function plumbing pattern established
- `junoclaw-mayo-verify` crate with a working pure-Rust MAYO-2 verifier
- Benchmark proving the pattern works: BN254 precompile cuts ZK verify gas by 1.82×

## Architecture

### 1. Host Function (Go → Rust VM bridge)

**File:** `wasmvm-fork/cosmwasm-std-bn254-ext/src/lib.rs`

Add alongside existing `bn254_pairing` — one function, variant-dispatched
(mirrors the contract's `MayoVariant` enum so the ABI never changes when we
tune parameters):

```rust
// Multi-variant MAYO verify host function
// variant: 1 = MAYO-1, 2 = MAYO-2, 3 = MAYO-3, 5 = MAYO-5
// Inputs: pk / msg / sig regions (lengths validated per variant)
// Output: 0 = valid, 1 = invalid, 2 = error (bad lengths / unknown variant)
#[no_mangle]
pub extern "C" fn mayo_verify(
    variant: u32,
    pk_ptr: u64,
    msg_ptr: u64,
    sig_ptr: u64,
    result_ptr: u64,
) -> u64 {
    // ... deserialize inputs via Region
    // ... dispatch: verify::<Mayo1|Mayo2|Mayo3|Mayo5>(msg, sig, pk)
    // ... write result back via Region
}
```

### 2. Wasm Contract Side

**File:** `contracts/jclaw-credential/src/contract.rs`

Add a feature-gated backend:

```rust
#[cfg(feature = "mayo-precompile")]
fn verify_mayo_signature(pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<bool, ContractError> {
    // Call host function via cosmwasm-std-bn254-ext
}

#[cfg(not(feature = "mayo-precompile"))]
fn verify_mayo_signature(pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<bool, ContractError> {
    // Pure wasm fallback (current implementation)
}
```

### 3. Gas Cost Model

| Operation | Pure Wasm | Precompile | Reduction |
|-----------|-----------|------------|-----------|
| `expand_pk` (AES-128-CTR) | ~160k gas | ~10k gas | 16× |
| `calculate_ps` / `calculate_sps` | ~125k gas | ~20k gas | 6× |
| `compute_rhs` + compare | ~50k gas | ~5k gas | 10× |
| CosmWasm overhead | ~21k gas | ~15k gas | 1.4× |
| **Total `VerifyMayoAttestation`** | **~356k gas** | **~50k gas** | **7×** |

**Target:** 50-100k gas per MAYO-2 verify (vs 356k today).

## Implementation Steps

### Phase 1: Host Function ✅ DONE (2026-06-13)

1. **Create `cosmwasm-crypto-mayo` crate** (`wasmvm-fork/cosmwasm-crypto-mayo/`)
   - Wraps `junoclaw-mayo-verify` with a VM-friendly API
   - `mayo_verify(variant, pk, msg, sig) -> Result<bool, MayoError>`
   - Per-variant gas constants in `gas.rs`

2. **Guest-side shim already done** (`cosmwasm-std-bn254-ext`)
   - `mayo_verify_call` wrapper with status-code handling ✅ (2026-06-12)

3. **Update `cosmwasm-vm.imports.rs.patch`** ✅
   - `do_mayo_verify` in `packages/vm/src/imports.rs`
   - `env.mayo_verify` in `packages/vm/src/compatibility.rs`
   - `mayo_verify` import registration in `packages/vm/src/instance.rs`
   - `cosmwasm-crypto-mayo` dependency in `packages/vm/Cargo.toml`

4. **Update `cosmwasm-std.imports.rs.patch`** ✅
   - `mayo_verify` extern declaration
   - `Api::mayo_verify` trait method on `ExternalApi`
   - `mayo_error_from_code` helper

5. **Remaining: rebuild devnet image**
   - Apply patch series to wasmvm/cosmwasm fork
   - Build `libwasmvm` with `cosmwasm-crypto-mayo` linked
   - Deploy to devnet and verify end-to-end

### Phase 2: Contract Backend (1 day)

1. **Add `mayo-precompile` feature to `jclaw-credential`**
   - Feature-gated `mayo_backend.rs` module
   - Host function call path (precompile enabled)
   - Pure wasm fallback path (precompile disabled)
   - Build both variants

2. **Update `deploy-moultbook-full.sh`**
   - Deploy precompile-enabled `jclaw-credential` to devnet
   - Verify `Bud` + `VerifyMayoAttestation` work end-to-end

### Phase 3: Benchmark & Document (1 day)

1. **Run `benchmark-mayo-devnet.sh`**
   - 5 samples of `VerifyMayoAttestation` with precompile
   - Compare to testnet pure-wasm numbers
   - Update `docs/PQC_COMPETITIVE_ANALYSIS.md` Path A with measured results

2. **Update docs**
   - `docs/MAYO.md`: add devnet precompile section
   - `docs/PQC_COMPETITIVE_ANALYSIS.md`: replace estimate with measured
   - `devnet/README.md`: add MAYO precompile instructions

## Files to Touch

```
wasmvm-fork/
  cosmwasm-crypto-mayo/ (new)               # VM-side MAYO wrapper crate
    Cargo.toml, src/{lib,errors,gas}.rs
  cosmwasm-std-bn254-ext/src/lib.rs          # guest-side mayo_verify_call ✅
  patches/cosmwasm-vm.imports.rs.patch       # do_mayo_verify + instance.rs ✅
  patches/cosmwasm-std.imports.rs.patch       # Api::mayo_verify ✅
  libwasmvm/build.rs                          # link crypto-mayo (next)

contracts/jclaw-credential/
  Cargo.toml                                  # add mayo-precompile feature
  src/contract.rs                             # feature-gate verify path
  src/mayo_backend.rs (new)                 # host fn + fallback

devnet/
  scripts/deploy-moultbook-full.sh            # deploy precompile variant
  scripts/benchmark-mayo.sh (new)           # MAYO benchmark script

docs/
  MAYO_PRECOMPILE_PLAN.md                   # this file
  PQC_COMPETITIVE_ANALYSIS.md               # update Path A with measured
  MAYO.md                                   # add devnet precompile section
```

## Rail A — Upstream Path (Juno Governance)

The devnet precompile is the **evidence**, not the destination. Sequence:

1. **Devnet proof (Phases 1-3 below):** measured multi-variant gas table,
   pure-wasm vs precompile, reproducible benchmark scripts.
2. **Spec write-up:** a short CWIP-style spec for `cosmwasm_mayo_verify`
   modeled on `secp256k1_verify` / `ed25519_verify` in `cosmwasm-vm`:
   deterministic, constant gas per variant, no allocation surprises.
   Cite NIST Round 4 status of MAYO.
3. **Upstream PR to CosmWasm (`cosmwasm-vm` + `wasmd`):** propose as an
   optional capability (`cosmwasm_2_x` style feature flag), so chains opt in
   via their wasmd config. Pure-Rust verifier = no C toolchain objections.
4. **Juno Commonwealth/forum post + governance prop:** enable the capability
   on uni-x testnet first, then mainnet. Pitch: "Juno becomes the first
   PQC-native CosmWasm chain — X gas ≈ $0.000Y per quantum-safe attestation."
5. **Fallback if upstream stalls:** Juno can carry the patch in its own wasmd
   fork (it already patches wasmd); contracts keep the pure-wasm fallback so
   nothing breaks either way.

**Key design rule:** the contract's `mayo_backend` abstraction (Phase 2) makes
the precompile *transparent* — same `ExecuteMsg`, same vectors, same tests.
Only gas changes. This is what makes the governance ask low-risk.

## Rail B — Commonware Backdoor

`junoclaw-mayo-verify` is already `#![no_std]` + `forbid(unsafe_code)` with
zero chain dependencies. To keep the door open:

1. **Keep the crate runtime-agnostic:** no `cosmwasm-std` imports ever; the
   FFI/Region glue lives only in `cosmwasm-std-bn254-ext`.
2. **Publish to crates.io** (after the benchmark article) so any runtime —
   including a Commonware-based stack — can depend on it. First-mover: the
   canonical Rust MAYO verifier is ours regardless of which chain wins.
3. **Optional spike (1 day, low priority):** a `commonware-cryptography`
   adapter implementing their signature-verification trait over our verifier.
   Do this only when/if Commonware momentum is real; the crate design already
   guarantees it's cheap.

This is the "utter determinism" play: Rail A wins if Juno stays sovereign;
Rail B wins if the ecosystem migrates. Both rails run on the same crate.

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `junoclaw-mayo-verify` doesn't compile to `no_std` | Medium | Feature-gate `std` deps, add `#![no_std]` gate |
| Host function signature mismatch (Go ↔ Rust) | Low | Copy exact pattern from `bn254_pairing` |
| WASM memory limits (512 KB) with 4.9 KB PK | Low | PK is passed by pointer, not copied into wasm memory |
| Contract binary size exceeds 512 KB | Low | Pure wasm path already 313 KB; precompile path is smaller |

## Success Criteria

- [x] `mayo_verify` host function implemented VM-side (`do_mayo_verify`) and std-side (`Api::mayo_verify`)
- [x] `cosmwasm-crypto-mayo` crate created with per-variant gas constants
- [x] Guest-side shim (`mayo_verify_call`) already callable from contract (all variants)
- [x] Precompile-enabled `jclaw-credential` deploys to devnet (code_id 2, store + instantiate succeed → `env.mayo_verify` import accepted)
- [~] `VerifyMayoAttestation` gas < 100k on devnet (MAYO-2) — **NOT met**: measured 310k total tx gas (vs 356k pure). The <100k target conflated isolated-verifier cost with total tx gas (≈150-200k fixed overhead) and assumed a near-zero host charge; the conservative VM gas schedule (5M/7M/12M) offsets the L1 win. Tunable — see results doc.
- [x] Multi-variant gas table: pure-wasm vs precompile — **measured on devnet** (MAYO-2 1.15×, MAYO-3 1.77×, MAYO-5 2.21×); see `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`
- [x] Benchmark reproducible (`devnet/scripts/benchmark-mayo-devnet.sh`, idempotent via results JSON)
- [x] Docs updated with measured numbers (`docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`)
- [ ] CWIP-style spec drafted for the upstream PR (Rail A step 2)
- [ ] (follow-up) Tune `cosmwasm-crypto-mayo` gas constants to widen L1/L3 margins

## Estimated Effort

**Total: 2-3 days** (1 day host fn + 1 day contract + 1 day benchmark/docs)

**Parallelizable:** Host fn and contract work can proceed in parallel (same interface).

## Comparison: BN254 vs MAYO Precompile

| Aspect | BN254 (done) | MAYO (done) |
|--------|--------------|-------------|
| Algorithm | Pairing-friendly curve | Multivariate quadratic |
| Core op | Miller loop + final exp | AES-CTR + GF(16) matrix ops |
| Speedup achieved | 1.82× | 1.15× (L1) / 1.77× (L3) / **2.21× (L5)** |
| Effort | ~1 week | ~2-3 days (pattern established) |
| Status | ✅ Working on devnet | ✅ Working on devnet (measured) |

## Next Action

Integration is complete and benchmarked (see `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`).
Remaining follow-ups: (1) tune the `cosmwasm-crypto-mayo` gas schedule to widen the
L1/L3 margins, and (2) draft the CWIP-style spec for the upstream CosmWasm PR (Rail A).
