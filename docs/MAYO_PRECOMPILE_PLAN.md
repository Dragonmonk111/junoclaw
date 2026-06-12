# MAYO Precompile Plan

## Goal
Add a `mayo2_verify` host function to the BN254-patched devnet, reducing on-chain MAYO-2 verification gas from ~356k (wasm) to ~50-100k (native).

## Context
We already have:
- `junoclaw-bn254-1` devnet with BN254 pairing precompile
- `wasmvm-fork/` with the host-function plumbing pattern established
- `junoclaw-mayo-verify` crate with a working pure-Rust MAYO-2 verifier
- Benchmark proving the pattern works: BN254 precompile cuts ZK verify gas by 1.82×

## Architecture

### 1. Host Function (Go → Rust VM bridge)

**File:** `wasmvm-fork/cosmwasm-std-bn254-ext/src/lib.rs`

Add alongside existing `bn254_pairing`:

```rust
// MAYO-2 verify host function
// Inputs: pk (4912 bytes), message (arbitrary), signature (328 bytes)
// Output: 0 = valid, 1 = invalid, 2 = error
#[no_mangle]
pub extern "C" fn mayo2_verify(
    pk_ptr: u64,
    msg_ptr: u64,
    sig_ptr: u64,
    result_ptr: u64,
) -> u64 {
    // ... deserialize inputs via Region
    // ... call junoclaw_mayo_verify::verify(pk, message, signature)
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

### Phase 1: Host Function (1 day)

1. **Add `mayo2_verify` to `cosmwasm-std-bn254-ext`**
   - Import `junoclaw-mayo-verify` crate as dependency
   - Implement FFI wrapper (deserialize Regions, call verify, serialize result)
   - Build and verify `.a` archive is produced

2. **Update `wasmvm-fork/libwasmvm` build**
   - Link `junoclaw-mayo-verify` into `libwasmvm.a`
   - Rebuild devnet image
   - Verify host function is callable from Go

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
  cosmwasm-std-bn254-ext/src/lib.rs          # add mayo2_verify host fn
  cosmwasm-std-bn254-ext/Cargo.toml          # add junoclaw-mayo-verify dep
  libwasmvm/build.rs                          # link mayo verify crate

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

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `junoclaw-mayo-verify` doesn't compile to `no_std` | Medium | Feature-gate `std` deps, add `#![no_std]` gate |
| Host function signature mismatch (Go ↔ Rust) | Low | Copy exact pattern from `bn254_pairing` |
| WASM memory limits (512 KB) with 4.9 KB PK | Low | PK is passed by pointer, not copied into wasm memory |
| Contract binary size exceeds 512 KB | Low | Pure wasm path already 313 KB; precompile path is smaller |

## Success Criteria

- [ ] `mayo2_verify` host function callable from contract
- [ ] Precompile-enabled `jclaw-credential` deploys to devnet
- [ ] `VerifyMayoAttestation` gas < 100k on devnet
- [ ] 5-sample benchmark reproducible
- [ ] Docs updated with measured numbers

## Estimated Effort

**Total: 2-3 days** (1 day host fn + 1 day contract + 1 day benchmark/docs)

**Parallelizable:** Host fn and contract work can proceed in parallel (same interface).

## Comparison: BN254 vs MAYO Precompile

| Aspect | BN254 (done) | MAYO (planned) |
|--------|--------------|----------------|
| Algorithm | Pairing-friendly curve | Multivariate quadratic |
| Core op | Miller loop + final exp | AES-CTR + GF(16) matrix ops |
| Speedup achieved | 1.82× | Target: 5-7× |
| Effort | ~1 week | ~2-3 days (pattern established) |
| Status | ✅ Working on devnet | 📋 Planned |

## Next Action

Start **Phase 1** by modifying `wasmvm-fork/cosmwasm-std-bn254-ext/src/lib.rs` to add the `mayo2_verify` host function. This is the critical path — once the host function works, the contract side is straightforward.
