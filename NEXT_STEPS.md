# Next Steps — JClaw / junoclaw-bn254

> Generated after 2026-06-12 testnet deployment session.
> All 4 contracts deployed to uni-7 testnet. MAYO attestation tested end-to-end.

---

## ✅ Completed (2026-06-12)

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | MAYO attestation test on uni-7 | ✅ | Bud = 336,659 gas, Verify = 355,771 gas, tampered rejected |
| 2 | All 4 contracts deployed to testnet | ✅ | zk-verifier (78), jclaw-credential (79), moultbook (80) |
| 3 | $JClaw token design resolved | ✅ | Three-layer: credential (soulbound) + economic (TokenFactory) + no CW20 |
| 4 | WSL2 devnet restart loop mitigated | ✅ | `while true` restart loop, `timeout_commit=0s` |
| 5 | MAYO PQC article for Telegram/Medium | ✅ | `articles/JUNOCLAW_MAYO_PQC_LIVE_2026_06_12.md` |
| 6 | ZK-verifier benchmark on uni-7 | ✅ | VK store = 212,823 gas; Verify = 371,129 gas; ~3.2s per verify |
| 7 | MAYO-signed moultbook attestation test | ✅ | End-to-end: Bud, hash match, valid verify, tampered rejected |
| 8 | PQC competitive positioning (Marius) | ✅ | `docs/PQC_COMPETITIVE_ANALYSIS.md` + public comms drafted |
| 9 | **Multi-variant MAYO (L1/L3/L5)** | ✅ | Measured ladder: 356k / 457k / 799k gas. Code ID 81. |
| 10 | **Benchmark script + RPC fallback** | ✅ | `deploy/benchmark-mayo-variants.cjs` — idempotent, multi-RPC |
| 11 | **Article v2 (measured ladder + prompts)** | ✅ | Updated with full table, $/attestation, Midjourney prompts |
| 12 | **MAYO precompile VM-side (`do_mayo_verify`)** | ✅ | `cosmwasm-crypto-mayo` crate + VM patch + std patch (2026-06-13) |

---

## Immediate Priority (This Week)

### 13. Fix Devnet Stability 🔴
**Problem**: WSL2 clock jumps stall CometBFT consensus; `junod` exits code 255
every ~30–60 s. Mitigation works but moultbook deploy still fails (store→instantiate gap exceeds ~60s restart window).

**Options**:
- **A. Native Linux VM** — migrate devnet to a VM with stable monotonic clock.
- **B. Docker Desktop on Windows** — run devnet directly on Windows Docker.
- **C. CI runner** — use GitHub Actions / self-hosted runner for devnet + deploy.

**Success criteria**: Container runs >30 min without restart; `deploy-moultbook.sh` completes end-to-end.

---

## Short-Term (Next 1-2 Weeks)

### 9. MAYO-3 / MAYO-5 Parameter Set Support
~~`junoclaw-mayo-verify` currently only implements MAYO-2. Extend to MAYO-3/5.~~
**✅ DONE** — streaming `expand_pk` shipped with multi-variant contract (code ID 81).

### 12. MAYO Precompile (devnet) — closes the gas gap ✅ VM-side DONE
Guest-side wrapper (`mayo_verify_call`) is in `cosmwasm-std-bn254-ext`.
VM-side `do_mayo_verify` + `cosmwasm-crypto-mayo` crate + gas constants + registration ✅ (2026-06-13).

**Remaining**: Rebuild devnet image, deploy precompile-enabled contract, benchmark vs pure-wasm.

**Target gas**: L1 ~50k, L5 ~120k (7× reduction).

### 13. L5 Wasm Optimization (lower priority — precompile is the real fix)
If precompile stalls or you need L5 *before* upstream acceptance:

- **wasm-opt -O3 for speed** instead of -Os for size (CosmWasm meters per
  instruction, smaller ≠ cheaper; `-O3` may cut 10-20%)
- **Bitsliced AES-128** in `expand_pk`: process 8 CTR blocks in parallel
  instead of sequential; ~30-40% expand speedup, minimal code change
- **Batch verification**: amortize tx overhead by verifying N signatures in
  one call; per-sig cost drops to ~500-600k for L5

**Won't help** (spec mandated): caching expanded keys (PKs differ per caller),
skipping P2/P3 partial expansion (verifier needs them), reducing matrix dims
(spec fixed).

### 14. MAYO CLI (`junoclaw-cli`)
Add `keygen` and `sign` commands to the CLI (using `sriracha-mayo` C crate):
```bash
junoclaw-cli mayo keygen --output alice.mayo
junoclaw-cli mayo sign --key alice.mayo --message "hello" --output sig.bin
```
**Build req**: CMake + C toolchain (not available on wasm32).

### 15. Frontend Demo Scaffolding
From `docs/FRONTEND_PLAN_COSMOWARP_DEMO.md`:
- CosmWasm client (CosmJS or `@cosmjs/stargate`)
- Wallet connect (Keplr)
- MAYO attestation flow UI
- Moultbook entry viewer

---

## Medium-Term (Next Month)

### 16. ZK Circuit for MAYO Verification
Instead of running MAYO verify inside wasm (expensive), generate a Groth16
proof off-chain that proves "I verified this MAYO signature" and submit the
ZK proof on-chain. This makes MAYO attestation cheap enough for mainnet and
verifiable on any chain with BN254 precompiles.

**Approach**:
- Arkworks circuit that wraps `junoclaw-mayo-verify::verify`
- Groth16 trusted setup
- BN254 precompile for on-chain proof verification

### 17. IBC Cross-Chain MAYO Signatures
MAYO signatures should be verifiable on any IBC-connected chain.
- Pack MAYO verify as a light-client predicate or wasm light client?
- Alternative: relay attestation proofs via IBC + verify on destination chain.

### 18. Governance Integration
Connect `jclaw-credential` Bud/weight tree to the existing `cw4-group`-style
agent-company roster. Add `Member`, `ListMembers`, `TotalWeight` queries to
`jclaw-credential` so it can serve as a drop-in replacement for governance
weight lookup.

---

## Deferred / Optional

### 15. Economic Token (`ujclaw`)
Per `JCLAW_TOKEN_DESIGN.md` decision: the governance credential is already
soulbound (non-transferable) via the agent-company roster. A tradeable
TokenFactory `ujclaw` is optional and deferred until there is a clear economic
use case.

---

## Blockers & Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| WSL2 devnet instability | **High** — blocks live testing | Move to native Linux / CI |
| MAYO-3/5 memory in wasm | ~~Medium~~ **Resolved** | Streaming expand_pk shipped |
| `sriracha-mayo` C dep | **Medium** — won't compile to wasm32 | Keep C for CLI only; pure-Rust for on-chain |
| No frontend developer | **Low** — slows demo | CosmJS + React is well-documented |
| Precompile upstream acceptance | **Medium** — political / governance timeline | CWIP spec + Juno fork fallback |
| L5 gas too high for mainnet (wasm) | **Medium** — 799k may hit gas limits | Precompile is fix; wasm-opt as band-aid |

---

## Contract Addresses

### Testnet (uni-7) — ✅ Live

```env
# zk-verifier (pure)
PURE_CODE_ID=78
PURE_ADDR=juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2

# jclaw-credential
JCLAW_CODE_ID=79
JCLAW_ADDR=juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r

# moultbook
MOULTBOOK_CODE_ID=80
MOULTBOOK_ADDR=juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4
```

### Devnet (junoclaw-bn254-1) — ⚠️ Partial

```env
# zk-verifier
PURE_CODE_ID=1
PURE_ADDR=juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8
PRECOMPILE_CODE_ID=2
PRECOMPILE_ADDR=juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p

# jclaw-credential
JCLAW_CODE_ID=3
JCLAW_ADDR=juno17p9rzwnnfxcjp32un9ug7yhhzgtkhvl9jfksztgw5uh69wac2pgszu8fr9

# moultbook
# NOT DEPLOYED — blocked by devnet restart loop
```

### Multi-variant benchmark (code ID 81) — ✅

```env
BENCH_CODE_ID=81
BENCH_ADDR=juno1zj39neajvynzv4swf3a33394z84l6nfduy5sntw58re3z7ef9p4q3w4y47
```

| Variant | Verify gas | Verify tx |
|---------|------------|-----------|
| MAYO-2 (L1) | 356,368 | `1C96D78...D6A5AA81` |
| MAYO-3 (L3) | 457,221 | `04A9486...C8258BB` |
| MAYO-5 (L5) | 798,803 | `83F49BE...12C86C` |

---

*Last updated: 2026-06-12*
