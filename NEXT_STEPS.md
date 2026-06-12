# Next Steps — JClaw / junoclaw-bn254

> Generated after 2026-06-12 testnet deployment session.
> All 4 contracts deployed to uni-7 testnet. Devnet remains partial (moultbook blocked).

---

## Immediate Priority (This Week)

### 1. Live MAYO Attestation Test on uni-7 �
**Contract**: `jclaw-credential` (deployed at `juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r`)

Test steps:
1. `Bud` — store a MAYO-2 public key hash for the admin address.
2. `VerifyMayoAttestation` — submit a valid test-vector signature + message.
3. Verify gas usage and confirm on-chain acceptance.
4. Submit a tampered message and confirm rejection.

**Test vector**: `contracts/jclaw-credential/src/mayo_vectors.rs` (seed=[42;32])

### 2. ZK-Verifier Benchmark on uni-7 🟡
Run benchmark against the deployed pure verifier at `juno19jk0...vrfr2`:
- VK store gas
- Proof verification gas
- Compare against devnet precompile numbers

### 3. Fix Devnet Stability �
**Problem**: WSL2 clock jumps stall CometBFT consensus; `junod` exits code 255
every ~30–60 s.

**Options**:
- **A. Native Linux VM** — migrate devnet to a VM with stable monotonic clock.
- **B. Docker Desktop on Windows** — run devnet directly on Windows Docker.
- **C. CI runner** — use GitHub Actions / self-hosted runner for devnet + deploy.

**Success criteria**: Container runs >30 min without restart; deploy moultbook end-to-end.

---

## Short-Term (Next 2 Weeks)

### 4. ZK-Verifier Benchmark
Run `devnet/scripts/benchmark.sh` against the deployed pure + precompile
contracts. Capture:
- VK store gas
- Proof verification gas (pure wasm vs BN254 precompile)
- Block-space cost comparison

### 5. MAYO-3 / MAYO-5 Parameter Set Support
`junoclaw-mayo-verify` currently only implements MAYO-2. Extend to:
- MAYO-3 (streaming AES-CTR needed for larger PK)
- MAYO-5 (largest parameter set)

**Blocker**: `expand_pk` loads the full expanded PK into memory.
For MAYO-3/5 this may exceed wasm32 memory limits (~128 KB peak for MAYO-2).
Need streaming or chunked approach.

### 6. MAYO CLI (`junoclaw-cli`)
Add `keygen` and `sign` commands to the CLI (using `sriracha-mayo` C crate):
```bash
junoclaw-cli mayo keygen --output alice.mayo
junoclaw-cli mayo sign --key alice.mayo --message "hello" --output sig.bin
```
**Build req**: CMake + C toolchain (not available on wasm32).

### 7. Frontend Demo Scaffolding
From `docs/FRONTEND_PLAN_COSMOWARP_DEMO.md`:
- CosmWasm client (CosmJS or `@cosmjs/stargate`)
- Wallet connect (Keplr)
- MAYO attestation flow UI
- Moultbook entry viewer

---

## Medium-Term (Next Month)

### 8. ZK Circuit for MAYO Verification
Instead of running MAYO verify inside wasm (expensive), generate a Groth16
proof off-chain that proves "I verified this MAYO signature" and submit the
ZK proof on-chain. This makes MAYO attestation cheap enough for mainnet.

**Approach**:
- Arkworks circuit that wraps `junoclaw-mayo-verify::verify`
- Groth16 trusted setup
- BN254 precompile for on-chain proof verification

### 9. IBC Cross-Chain MAYO Signatures
MAYO signatures should be verifiable on any IBC-connected chain.
- Pack MAYO verify as a light-client predicate or wasm light client?
- Alternative: relay attestation proofs via IBC + verify on destination chain.

### 10. Publication / Article
Write the technical article covering:
- BN254-patched Juno devnet (what we built)
- Pure-Rust MAYO-2 verifier (bugs found, cross-check with C)
- On-chain PQC verification in CosmWasm
- Gas measurements and memory limits
- Why this matters for post-quantum blockchain security

---

## Deferred / Optional

### 11. Economic Token (`ujclaw`)
Per `JCLAW_TOKEN_DESIGN.md` decision: the governance credential is already
soulbound (non-transferable) via the agent-company roster. A tradeable
TokenFactory `ujclaw` is optional and deferred until there is a clear economic
use case.

### 12. Governance Integration
Connect `jclaw-credential` Bud/weight tree to the existing `cw4-group`-style
agent-company roster. This requires adding `Member`, `ListMembers`,
`TotalWeight` queries to `jclaw-credential` so it can serve as a drop-in
replacement for governance weight lookup.

---

## Blockers & Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| WSL2 devnet instability | **High** — blocks all live testing | Move to native Linux / CI |
| MAYO-3/5 memory in wasm | **Medium** — may exceed 512 KB wasm limit | Streaming expand_pk; ZK-proof approach |
| `sriracha-mayo` C dep | **Medium** — won't compile to wasm32 | Keep C for CLI only; pure-Rust for on-chain |
| No frontend developer | **Low** — slows demo | CosmJS + React is well-documented |

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

---

*Last updated: 2026-06-12*
