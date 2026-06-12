# Project Fable — MAYO-5 Roadmap & PQC Reassessment

> Closing the loop: from MAYO-2 (NIST Level 1, live on uni-7) to MAYO-5
> (NIST Level 5, maximum security) — and an honest reassessment of whether
> MAYO is still the right horse.
>
> Context: Jake Hartnell referenced Commonware (commonware.xyz) as his
> engineering quality bar; Marius claims he's "reaching commonware levels"
> with his chain-native Falcon work. Fable is our answer: ship Level 5 PQC
> that runs on EXISTING chains, with commonware-grade rigor.

---

## 1. Where We Stand (verified against codebase, 2026-06-12)

### Already built and proven
- `crates/junoclaw-mayo-verify` — pure-Rust, `no_std`, `forbid(unsafe_code)`
  verifier, **generic over `ParameterSet`**. `Mayo1/2/3/5` param structs
  already defined in `params.rs` with full NIST Round 4 constants.
- C cross-check tests (`test-c` feature) validate bit-for-bit against
  `sriracha-mayo` reference implementation — MAYO-2 only so far.
- `crates/junoclaw-core/src/mayo.rs` — off-chain keygen/sign for **all four
  variants** already works (wraps sriracha C crate).
- `contracts/jclaw-credential` — `Bud { mayo_pk }`, `VerifyMayoAttestation`,
  `MayoPkHash` query. **Hardcoded to MAYO-2.** 4 passing multi-test cases.
- Live on uni-7: Bud = 336,659 gas; Verify = 355,771 gas.
- BN254 precompile pattern proven on devnet (1.82× measured reduction).

### The single real blocker: wasm memory

Expanded public key (P1+P2+P3 limbs) per variant:

| Variant | Level | sig | compact PK | expanded PK | `ps` buf | peak est. | 512 KB wasm |
|---------|-------|-----|-----------|-------------|----------|-----------|-------------|
| MAYO-2 | 1 | 186 B | 4,912 B | ~104 KB | ~10 KB | ~127 KB (measured) | ✅ |
| MAYO-3 | 3 | 681 B | 2,986 B | ~384 KB | ~71 KB | ~470 KB | ⚠️ borderline |
| MAYO-5 | 5 | 964 B | 5,554 B | **~839 KB** | ~130 KB | >1 MB | ❌ needs streaming |

### Why streaming works (from code inspection)

`calculate_ps` in `verify.rs` consumes P1 **strictly sequentially**
(`p1_used` advances monotonically through upper-triangular rows) and P2
row-major (`row * o + j`). P1 and P2 are nothing but AES-128-CTR keystream;
the keystream can be generated incrementally at any counter offset.
Therefore:

- Generate P1/P2 **one row at a time** inside the `calculate_ps` row loop.
- Row buffer for MAYO-5: max `v * m_vec_limbs * 8` = 142×72 ≈ **10.2 KB**.
- New peak for MAYO-5: ~10 KB row + 130 KB `ps` + 19 KB sps/acc ≈ **~165 KB**. Fits.
- P3 comes directly from the compact PK (no AES) — 5.6 KB, trivial.

No algorithm change, no new crypto — pure dataflow refactor.

---

## 2. Reassessment: Is MAYO Still the Best Option?

### NIST Level 5 landscape (verification on-chain, wasm32)

| Scheme | Status | Sig | PK | Verify cost | wasm feasibility |
|--------|--------|-----|-----|-------------|------------------|
| **MAYO-5** | Round 4 candidate | **964 B** | 5,554 B | AES-CTR + GF(16) | ✅ proven pattern (us) |
| Falcon-1024 (Marius) | FIPS 206 draft (FN-DSA) | 1,280 B | 1,793 B | NTT, int-only verify | ✅ feasible, nobody shipped wasm |
| ML-DSA-87 (Dilithium5) | **FIPS 204 standard** | 4,627 B | 2,592 B | NTT | ✅ feasible, large sigs |
| SLH-DSA (SPHINCS+) | **FIPS 205 standard** | ~30-50 KB | 64 B | hash-heavy | ❌ sig too big for tx |

### Honest assessment

**For MAYO:**
- Smallest Level 5 signatures of any candidate (964 B beats Falcon's 1,280 B).
- Our verifier exists, is memory-optimized, cross-checked vs C, audited shape
  (`no_std`, no unsafe). Sunk cost is real engineering capital.
- Generic `ParameterSet` design means MAYO-5 is an *extension*, not a rewrite.

**Against MAYO:**
- Round 4 candidate, **not yet a NIST standard**. If MAYO is broken or not
  selected, we redo the crypto layer. (Mitigation: multivariate schemes have
  taken hits historically — Rainbow broke in 2022. MAYO's whipped-up oil+vinegar
  design specifically addresses that attack family, but risk is nonzero.)
- Falcon-1024 is on a standards track (FN-DSA / FIPS 206 draft) and Marius is
  validating it chain-natively. ML-DSA is *already* FIPS 204.

### Verdict: keep MAYO as flagship, hedge with pluggable verifier trait

1. **MAYO-5 remains the headline** — smallest Level 5 sigs, deploys on existing
   chains today, and the verifier crate is ours end-to-end. Nobody else has
   a wasm PQC verifier at any level in Cosmos. First-mover story intact.
2. **Hedge cheaply:** extract a `PqcVerifier` trait in the contract layer so a
   `falcon-verify` or `ml-dsa-verify` crate can slot in later without touching
   contract storage or messages (algorithm tag byte in stored hash preimage).
3. **Do not build Falcon/ML-DSA verifiers now** — wait for FIPS 206 final and
   market signal. Revisit at next NIST announcement.

---

## 3. The Plan: Five Phases to MAYO-5

### Phase 0 — Test vectors for all variants (½ day)
- Extend `test-c` cross-check tests to MAYO-1, MAYO-3, MAYO-5
  (the test harness is already generic; add three `#[test]` fns).
- Generate deterministic test vectors (seed=[42;32]) for each variant,
  export hex to `devnet/mayo_vector_{1,3,5}.hex` for contract tests.
- **Gate:** all C cross-checks pass for all four parameter sets.

### Phase 1 — MAYO-3 in pure wasm, measure reality (1 day)
- MAYO-3 *may* fit without streaming (~470 KB peak, borderline).
- Wire `Mayo3` into a test contract build; run under `cw-multi-test` with
  wasm memory instrumentation; deploy to devnet and attempt verify.
- **Outcome A (fits):** ship MAYO-3 immediately as Level 3 option.
- **Outcome B (OOM):** proceed straight to Phase 2; MAYO-3 becomes the
  first consumer of streaming.
- **Gate:** measured peak memory + gas numbers for MAYO-3 documented.

### Phase 2 — Streaming `expand_pk` (3-4 days, the core work)
- New `verify_streaming<P>` path in `junoclaw-mayo-verify`:
  - `AesCtrStream` struct: wraps AES-128-CTR with explicit counter offset,
    yields P1/P2 m-vec rows on demand.
  - Refactor `calculate_ps` to pull P1 rows from the stream inside the row
    loop (consumption is already sequential — verified in code).
  - P2 rows likewise; P3 decoded from cpk directly (already cheap).
- Keep the existing materialized path for MAYO-1/2 (it's faster — one AES
  pass vs interleaved); feature-select by `P::EPK_LIMBS` threshold.
- Re-run ALL C cross-checks against streaming path.
- **Gate:** MAYO-5 verify passes C cross-check in `no_std` with peak
  memory < 200 KB (instrumented).

### Phase 3 — Multi-variant contract support (2 days)
- `jclaw-credential` changes:
  - `Bud { mayo_pk: Option<Vec<u8>>, mayo_variant: Option<MayoVariant> }`
    (default Mayo2 for backward compat with deployed testnet contract).
  - Stored hash becomes `SHA-256(variant_tag || compact_pk)`.
  - `VerifyMayoAttestation` dispatches on stored variant.
  - `PqcVerifier` trait extraction (the hedge from §2).
- Migration message for the deployed uni-7 contract (existing entries
  default to Mayo2 tag).
- **Gate:** 4 existing tests + 8 new variant tests pass; wasm binary
  still < 512 KB after `wasm-opt`.

### Phase 4 — Deploy + benchmark ladder (1-2 days)
- Devnet first (stable as of today), then uni-7 testnet.
- Benchmark matrix: {Mayo2, Mayo3, Mayo5} × {Bud, Verify} × 3 samples.
- Expected gas (extrapolating from MAYO-2's 356k and op-count scaling):
  - MAYO-3 verify: ~700k-900k gas (m=108, k=11 vs m=64, k=4)
  - MAYO-5 verify: ~1.2-1.6M gas — **this is why the precompile matters**
- Fold MAYO precompile (docs/MAYO_PRECOMPILE_PLAN.md) into the same devnet
  image so we benchmark wasm vs precompile for MAYO-5 in one pass.
- **Gate:** published benchmark table, all variants verified live on-chain.

### Phase 5 — Docs + comms (1 day)
- Update `docs/MAYO.md`, `docs/PQC_COMPETITIVE_ANALYSIS.md` with Level 5 numbers.
- Article: "NIST Level 5 post-quantum attestations on a live Cosmos chain —
  no fork required." Direct response to the Commonware/Falcon thread:
  Marius needs a new chain for Level 5; we do it on uni-7 today.
- Telegram reply to Jake/Marius thread with the benchmark table.

**Total effort: ~8-10 working days.**

---

## 4. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| MAYO-5 gas > block gas limit on public chains | Medium | High | Precompile path; or verify-once-store-hash pattern |
| Streaming refactor introduces subtle keystream offset bug | Medium | High | Bit-for-bit C cross-check is the gate; `expand_pk` equality test already exists |
| MAYO-3 borderline memory fails on real chain (not multi-test) | Medium | Low | Phase 1 measures on devnet before committing |
| MAYO cryptanalysis event (Round 4 ongoing) | Low | Critical | `PqcVerifier` trait hedge; variant tag in storage enables migration |
| sriracha C crate lacks MAYO-3/5 NIST-current params | Low | Medium | Verified: `junoclaw-core` already keygens all 4 variants |

---

## 5. Immediate Next Actions

1. **Phase 0 now:** add MAYO-1/3/5 cross-check tests to
   `crates/junoclaw-mayo-verify/src/lib.rs` (generic harness exists).
2. Generate MAYO-3/5 test vectors for contract tests.
3. Phase 1 memory measurement for MAYO-3.

---

*Project Fable, day one. The fable: everyone said you need a new chain for
post-quantum security. We're shipping NIST Level 5 on chains that already exist.*
