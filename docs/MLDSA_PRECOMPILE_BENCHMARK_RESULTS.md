# ML-DSA (FIPS 204) precompile benchmark — devnet results

> Auto-generated raw data by `deploy/benchmark-mldsa-devnet.cjs`. Re-run with
> `devnet/scripts/benchmark-mldsa-devnet.sh` (set `FRESH=1` to wipe prior results).

- **Chain:** `junoclaw-bn254-1` (single-validator devnet, `junod` linked against the ML-DSA-patched `libwasmvm`)
- **Signer:** `juno1xvr9zjxp9rekn0v7p6trmfmyv8m2mwzldnmcad` (in-container `admin`)
- **wasmvm-fork patches:** `v2.2.2/00-28` (BN254 `00-09` + MAYO `10-19` + ML-DSA `20-28`)
- **Build:** both flavours from identical source, differing only by `--features mldsa-precompile`
  (pure = `devnet/scripts/build-mldsa-pure.sh`, precompile = `devnet/scripts/build-mldsa-precompile.sh`)
- **Gas price:** `0.075ujuno`
- **Date:** 2026-06-15

## Headline — `VerifyMlDsaAttestation` gas (pure-Wasm vs precompile)

| Variant | NIST | PK (B) | Sig (B) | Pure-Wasm verify | Precompile verify | Speedup |
|---------|------|-------:|--------:|-----------------:|------------------:|--------:|
| ML-DSA-44 | L2 | 1,312 | 2,420 | 269,604 | 260,381 | 1.04× |
| ML-DSA-65 | L3 | 1,952 | 3,309 | 328,945 | 315,212 | 1.04× |
| ML-DSA-87 | L5 | 2,592 | 4,627 | 408,298 | 387,124 | 1.05× |

**The precompile speedup is negligible (~1.04–1.05×) and flat across parameter
sets — the opposite of MAYO.** This is the key result, and it is expected (see
Interpretation).

## Bud (child + PK-hash store) and SetMlDsaPk gas

| Variant | Pure Bud | Precompile Bud | Pure SetMlDsaPk | Precompile SetMlDsaPk |
|---------|---------:|---------------:|----------------:|----------------------:|
| ML-DSA-44 | 153,867 | 153,945 | 183,864 | 183,942 |
| ML-DSA-65 | 155,399 | 155,477 | 206,975 | 207,053 |
| ML-DSA-87 | 156,931 | 157,009 | 230,450 | 230,528 |

`Bud` and `SetMlDsaPk` store only the SHA-256 hash of the PK, so they are
identical across flavours (the ~78 gas delta is signer-nonce noise). The
precompile changes only the `VerifyMlDsaAttestation` path. `SetMlDsaPk` grows
with PK size (1,312 → 1,952 → 2,592 B) because the larger key is hashed.

## Deployment

| Flavour | code_id | wasm size | store gas | init gas | address |
|---------|--------:|----------:|----------:|---------:|---------|
| pure       | 4 | 472,512 B | 2,938,715 | 180,878 | `juno1ghd753shjuwexxywmgs4xz7x2q732vcnkm6h2pyv9s6ah3hylvrq722sry` |
| precompile | 5 | 391,752 B | 2,510,811 | 181,118 | `juno1eyfccmjm6732k7wp4p6gdjwhxjwsvje44j0hfx8nkgrm8fs7vqfs9a4ky4` |

The precompile binary is **~81 KB (17%) smaller** — it offloads the entire
`fips204` verifier (NTT + integer modular arithmetic) to the host, so the
contract no longer carries that code. This size win, **not** a speed win, is the
main on-chain benefit for ML-DSA.

## Verify tx hashes

Pure-Wasm:
- ML-DSA-44 `A90B15D88A2440C36DCB74B89BD35C9B6FF0F71BD01B5DAF2F3DC666938F7966`
- ML-DSA-65 `73877F1ECD192EC89560282D7B0B69395FEDEE1AB6A187D20607A43C2EAB89A9`
- ML-DSA-87 `7CF23066DA4E16CE9A8BBA4636AE0528AFE7FF8B4C4B1BC2A43E729DF8DA834B`

Precompile:
- ML-DSA-44 `9C1C9C55673A3E356353FCAF04E074298B741F938414ADA6E08B39612E2504CA`
- ML-DSA-65 `41072FEAA90904DF009907F9F510EE950AF3424661FF683C726B2B3599D56903`
- ML-DSA-87 `49108A98571B580E84D7CE64DD9B85F4749E38775E10659F144A5C32395EDC91`

## Interpretation

**End-to-end correctness is proven.** The precompile contract `store_code` and
`instantiate` both succeeded, which is only possible if `env.ml_dsa_verify` is in
the allowed-imports set (patch `22-cosmwasm-vm.compatibility.rs`) and the host
function is registered (patches `21`/`23`). All three variants then verified to
`valid=true` through the native host path — the in-image `cosmwasm-crypto-mldsa`
crate runs the verification natively instead of as metered Wasm.

**The precompile does not meaningfully reduce verify gas for ML-DSA, and this is
the expected result — not a regression.** ML-DSA (FIPS 204) verification is
lattice-based: number-theoretic transform plus integer modular arithmetic, which
compiles to cheap, branch-light Wasm. The dominant cost of the
`VerifyMlDsaAttestation` tx is the **fixed per-tx overhead** (signature check,
message routing, member load, PK-hash recompute + compare, marshalling the large
pk+sig regions into the contract), which the precompile does not touch. Moving
the already-cheap verify math to the host therefore shaves only a thin slice —
hence the flat ~1.04–1.05×.

### Contrast with MAYO — opposite precompile economics

| | MAYO (multivariate) | ML-DSA (lattice) |
|---|---|---|
| Verify math | GF(16) oil-and-vinegar matrix algebra (heavy in Wasm) | NTT + integer modular arithmetic (cheap in Wasm) |
| Precompile speedup | **1.15× → 1.77× → 2.21×** (grows with NIST level) | **~1.04× flat** |
| Why offload pays | The offloaded computation is large | The offloaded computation is small |
| Primary on-chain benefit | **Performance** | **Wasm size (~17%) + chain-alignment** |

The two schemes are mirror images. For MAYO, the heavier the parameter set the
more the precompile wins (because more work moves from metered Wasm to native).
For ML-DSA, the verify is so cheap in Wasm that there is almost nothing to win on
speed — what you gain is a **smaller contract** and a **single audited,
gas-metered native verifier** that contracts can share rather than each bundling
their own `fips204` copy.

### Consequence for Project Aegis

This **confirms** the `PROJECT_AEGIS_JUNO_FULL_PQC.md` §5.1 conclusion from a
second, independent direction. §5.1 already argued — from wall-clock timing
(~101 µs verify for ML-DSA-44) — that *"verify CPU is not the bottleneck;
bandwidth is."* The on-chain gas number now shows the same thing through the gas
meter: ML-DSA verify is cheap enough on-chain that a native precompile barely
beats pure Wasm. This **reinforces the recommendation of ML-DSA-44 at the
consensus root** (cheap verify, the cost is signature bandwidth) and reframes the
Phase B `ml_dsa_verify` precompile as a **standardization / wasm-size** play, not
a performance play (unlike the MAYO precompile that motivated the pattern).

## Reproduce

```bash
# 1. Build both flavours (host: WSL, uses cosmwasm/optimizer container)
bash devnet/scripts/build-mldsa-pure.sh        # -> devnet/jclaw_credential_pure.wasm
bash devnet/scripts/build-mldsa-precompile.sh  # -> devnet/jclaw_credential_mldsa.wasm

# 2. Boot the ML-DSA-patched devnet (picks up patches 20-28 automatically)
bash devnet/scripts/run-devnet.sh

# 3. Deploy both + benchmark
FRESH=1 bash devnet/scripts/benchmark-mldsa-devnet.sh
```

## Related artefacts

- `wasmvm-fork/patches/v2.2.2/20-28-*.patch` — the ML-DSA host-function diffs
- `wasmvm-fork/cosmwasm-crypto-mldsa/` — VM-side ML-DSA wrapper + gas schedule
- `wasmvm-fork/cosmwasm-std-bn254-ext/src/lib.rs` — guest-side `ml_dsa_verify_call`
- `contracts/jclaw-credential/src/contract.rs` — feature-gated verify dispatch
- `deploy/benchmark-mldsa-devnet.cjs` + `devnet/scripts/benchmark-mldsa-devnet.sh`
- `deploy/mldsa-devnet-benchmark-results.json` — raw measured data
- `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md` — the MAYO counterpart (contrast)
- `docs/PROJECT_AEGIS_JUNO_FULL_PQC.md` §5.1, §9 — the plan this benchmark feeds
