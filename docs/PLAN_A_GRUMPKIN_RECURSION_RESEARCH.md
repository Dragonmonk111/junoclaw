# Plan A: Grumpkin Recursion — Removing the TEE Dependency

## Status: Research track (low priority — Plan D works today)

## The Problem

JunoClaw's current aggregation circuit (`circuits/proof-aggregation/src/lib.rs`) uses **Plan D**: a TEE (Intel SGX / AMD SEV-SNP) verifies three Groth16 pairing checks and attests to the result. The ZK circuit itself only proves knowledge of the public inputs and cross-tier consistency (~8K constraints). The TEE handles the expensive cryptographic verification for free.

This works. It's deployed. 9/9 TEE attestation tests pass with real Ed25519 verification. But it requires trusting the TEE hardware manufacturer and the TEE attestation chain. If TEE trust must be eliminated entirely, we need **full cryptographic recursion** — verifying proofs inside proofs without hardware.

---

## What Plan A Is

**BN254/Grumpkin elliptic curve cycle** for recursive ZK proofs without TEE.

The key mathematical property: **Grumpkin's scalar field = BN254's base field.** This means BN254 G1 point operations (pairing checks, scalar multiplications) become native field arithmetic inside a Grumpkin circuit. No non-native field arithmetic for the critical operations.

The cycle works as follows:
1. **BN254 circuit** — proves the robot's safety constraints (SensorSafety, IntentConsistency, ConsensusMembership). This is what we already have.
2. **Grumpkin circuit** — verifies a BN254 Groth16 proof. Because BN254's base field is Grumpkin's scalar field, the pairing check is native arithmetic.
3. **Fold** — use a folding scheme (Nova or CycleFold) to fold multiple Grumpkin instances into one, producing a single proof that N BN254 proofs all verify correctly.
4. **Final proof** — compress the folded instance into a single Groth16 proof over BN254 for on-chain verification.

Result: one on-chain Groth16 verification proves that all three tier proofs are cryptographically valid — no TEE required.

---

## Three Approaches (from research)

### 1. Nova + CycleFold (recommended)

**Nova** (CRYPTO 2022, Kothapalli/Setty/Tzialla) introduces folding schemes — instead of verifying a proof inside a circuit (expensive), fold two instances into one. The verifier circuit is constant-sized (~10,000 multiplication gates in original Nova).

**CycleFold** (ePrint 2023/1192, Kothapalli/Setty) improves this: only the scalar multiplications need the second curve, reducing the Grumpkin circuit to ~1,000-1,500 multiplication gates (nearly 10× improvement over original Nova).

**Why this fits JunoClaw:**
- Rust implementation exists: [privacy-ethereum/Nova](https://github.com/privacy-ethereum/Nova) supports BN254/Grumpkin
- No trusted setup required (unlike Groth16)
- Prover work per fold: two multi-exponentiations of size O(|F|) — fast
- Proof size: O(|F|) group elements before compression, O(log |F|) after
- Can produce a final Groth16 proof over BN254 for on-chain verification

**Estimated costs for JunoClaw:**
- Grumpkin verifier circuit: ~1,500 constraints (CycleFold)
- Augmented circuit (folding + BN254 proof verification): ~12-15K constraints
- Final compression: Groth16 over BN254, 128 bytes, verifiable on existing `zk-verifier` contract
- Proving time: estimated 500ms-2s per fold (vs 187ms current parallelized proving)
- On-chain verification: same gas as current aggregation proof (~203K with precompiles)

### 2. MicroNova (on-chain optimized)

**MicroNova** (IEEE S&P 2025, Zhao/Setty/Cui) optimizes the on-chain verification cost of folding-based proofs:
- Compressed proof: O(log N) group elements
- On-chain verification: O(log N) scalar multiplications + 2 pairings
- Ethereum gas: ~2.2M gas (measured on-chain)
- Requires universal trusted setup (KZG)

**Why this might not fit JunoClaw:**
- 2.2M gas is 10× our current ~203K — too expensive for per-batch verification
- KZG trusted setup is a different trust assumption than TEE (not necessarily better for our use case)
- The on-chain verifier would need a new CosmWasm contract (not just reusing `zk-verifier`)

### 3. Direct composition (no recursion, no TEE)

Not technically "Plan A" but the simplest no-TEE path: **one big Groth16 circuit** with all constraints from all three tier proofs plus the aggregation consistency checks.

- ~50K constraints total (vs ~8K current + TEE)
- 128-byte proof, same on-chain verification
- Proving time: ~5-10s (vs 187ms current)
- No new infrastructure — works today with arkworks 0.5
- No Grumpkin, no folding, no new curves

**Trade-off:** 5-10s proving time is well within the 2.8s settlement window's batch cadence (batches are every ~300ms from coordination, but settlement is every 2.8s). The proof doesn't gate physics — it gates the next intent. A 10s proof means the robot waits 10s before its next high-level decision instead of 187ms. For a delivery robot, that's fine. For a humanoid doing dynamic manipulation, that's too slow.

---

## Recommended Path

```
Now (2026)          → Plan D (TEE) — shipped, working, 187ms
                     ↓
Near-term (2026-27) → Add direct composition as no-TEE mode
                     → User selects: TEE for speed, composition for pure crypto
                     ↓
Future (2027-28)    → Nova + CycleFold over BN254/Grumpkin
                     → arkworks 0.6+ required (Grumpkin support)
                     → Replaces both TEE and composition
                     → ~500ms-2s proving, same on-chain gas
```

### Why this order:
1. **Plan D works today** — TEE attestation is real (Ed25519, 9/9 tests), hardware is being provisioned
2. **Direct composition is the simplest no-TEE fallback** — no new dependencies, just a bigger circuit. Ship it as a feature flag: `--no-tee` mode for users who don't want hardware trust
3. **Nova + CycleFold is the endgame** — but requires arkworks 0.6 (Grumpkin curve support), which is not yet stable. The `privacy-ethereum/Nova` repo has a working implementation, but it's research-grade. Production readiness expected 2027-28.

---

## Technical Requirements for Nova + CycleFold

### Dependencies
- `arkworks 0.6+` — Grumpkin curve support (not in 0.5, our current version)
- `nova-snark` crate — from [privacy-ethereum/Nova](https://github.com/privacy-ethereum/Nova)
- KZG commitment scheme (for final proof compression) — or use Groth16 decider

### New circuits needed
1. **Grumpkin verifier circuit** (~1,500 constraints) — verifies a BN254 Groth16 proof using CycleFold's scalar-multiplication-only approach
2. **Augmented folding circuit** (~12-15K constraints) — folds instances, checks cross-tier consistency (same logic as current `AggregationCircuit`)
3. **Decider circuit** — compresses folded instance into final Groth16 proof over BN254

### On-chain changes
- **No new contract needed** — the final proof is Groth16 over BN254, verifiable by existing `zk-verifier`
- **Same gas cost** — ~203K with precompiles, same as current aggregation proof
- **Remove TEE verification call** — `tee-attestation-verifier.VerifyAttestation` no longer needed in the settlement flow (but keep the contract for optional TEE mode)

### Estimated effort
- arkworks 0.6 upgrade: 2-3 days (breaking changes from 0.5)
- Grumpkin verifier circuit: 3-5 days
- Augmented folding circuit: 2-3 days
- Decider + integration: 3-5 days
- Testing + benchmarks: 3-5 days
- **Total: 13-21 days** (research-grade, not production)

---

## Comparison Table

| Approach | TEE? | Proving Time | On-Chain Gas | New Dependencies | Effort | Status |
|----------|------|-------------|-------------|-----------------|--------|--------|
| **Plan D (current)** | Yes | 187 ms | ~203K | None | Done | Shipped |
| **Direct composition** | No | 5-10 s | ~203K | None | 2-3 days | Ready to build |
| **Nova + CycleFold** | No | 500ms-2s | ~203K | arkworks 0.6, nova-snark | 13-21 days | Research |
| **MicroNova** | No | ~1 s | ~2.2M | KZG setup, new verifier | 15-25 days | Research |
| **On-chain multi-verify** | No | 187 ms (parallel) | ~609K (3×) | None | 1 day | Ready to build |

---

## Open Questions

1. **arkworks 0.6 timeline** — Grumpkin support is the blocker. Check [arkworks-rs/curves](https://github.com/arkworks-rs/curves) for Grumpkin implementation status. If not available, we'd need to implement Grumpkin ourselves (~2-3 days for curve + pairing).

2. **Sonobe as alternative** — [Sonobe](https://sonobe.pse.dev) (PSE) implements Nova+CycleFold with an on-chain decider that produces Groth16 over BN254. Could be a faster path than building from scratch. Worth evaluating their Rust crate maturity.

3. **Proof size for folding** — Nova proofs before compression are O(|F|) group elements. For our ~15K constraint circuit, that's ~15K Grumpkin points (~480KB). Compression to Groth16 brings it back to 128 bytes. Need to verify compression time doesn't dominate.

4. **Batch folding** — instead of folding 3 tier proofs, could we fold N cycle proofs into one batch proof? This is Track F in `PLAN_NEXT_TRACKS_2026_08_18.md`. Nova's IVC is designed for exactly this — fold 1,000 cycle proofs into one, then verify on-chain once. This would eliminate the need for separate BatchSafety circuit.

5. **Groth16 vs PLONK for decider** — Sonobe uses Groth16 for the final decider proof (compatible with our `zk-verifier`). But PLONK with custom gates could reduce constraint count for the miller loop. Trade-off: new verifier contract vs. reuse existing.

---

## Decision Matrix

| When to use | Approach |
|-------------|----------|
| **Now** | Plan D (TEE) — fastest, shipped, works |
| **User refuses TEE** | Direct composition — slow but pure crypto, no new deps |
| **User wants cheap on-chain** | On-chain multi-verify — 3× gas but simplest, no TEE |
| **2027-28, arkworks 0.6 stable** | Nova + CycleFold — best of all worlds |
| **Never (probably)** | MicroNova — gas too expensive for per-batch use |

---

## References

- [Nova: Recursive Zero-Knowledge Arguments from Folding Schemes](https://eprint.iacr.org/2021/370) — CRYPTO 2022
- [CycleFold: Folding-scheme-based recursive arguments over a cycle of elliptic curves](https://eprint.iacr.org/2023/1192) — 2023
- [MicroNova: Folding-based arguments with efficient on-chain verification](https://eprint.iacr.org/2024/2099) — IEEE S&P 2025
- [privacy-ethereum/Nova](https://github.com/privacy-ethereum/Nova) — Rust implementation with BN254/Grumpkin
- [Sonobe (PSE)](https://sonobe.pse.dev) — Nova+CycleFold with on-chain decider
- [Revisiting the Nova Proof System on a Cycle of Curves](https://eprint.iacr.org/2023/969) — 2023
- Existing: `circuits/proof-aggregation/src/lib.rs` — Plan D aggregation circuit (shipped)
- Existing: `drafts/HACK_ZK_TRACKS_SUMMARY_2026_08_18.md` — 5 alternatives analysis
- Existing: `drafts/PLAN_NEXT_TRACKS_2026_08_18.md` — Track F (batch folding) plan

---

*Research document version: 2026-08-20. Plan D (TEE) is shipped and working. This document defines the path to remove the TEE trust assumption when arkworks 0.6 and Nova+CycleFold reach production maturity. Direct composition is available as an immediate no-TEE fallback.*
