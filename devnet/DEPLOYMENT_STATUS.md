# Deployment Status — 2026-06-12

## Testnet (uni-7) — ✅ All Contracts Deployed

**Chain ID**: `uni-7`
**RPC**: `https://juno.rpc.t.stavr.tech`
**Deployer**: `juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6`

### Deployed Contracts

| Contract | Code ID | Address |
|----------|---------|---------|
| **zk-verifier (pure)** | 78 | `juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2` |
| **zk-verifier (precompile)** | — | *Skipped — uni-7 lacks bn254_add precompile* |
| **jclaw-credential** | 79 | `juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r` |
| **moultbook** | 80 | `juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4` |
| **jclaw-credential (multi-variant MAYO)** | 81 | `juno1zj39neajvynzv4swf3a33394z84l6nfduy5sntw58re3z7ef9p4q3w4y47` |

### MAYO Multi-Variant Benchmark (2026-06-12, code ID 81)

| Variant | NIST | Verify gas | Verify tx |
|---------|------|------------|-----------|
| MAYO-2 | L1 | 356,368 | `1C96D7824C4F5C474FE439695F5F58FF319C44F76B39BCD42AA78229D6A5AA81` |
| MAYO-3 | L3 | 457,221 | `04A948673618FB4830FCCA75DEDECB59638A02AA972A2FA0AF6AC171BC8258BB` |
| MAYO-5 | L5 | 798,803 | `83F49BE3E2B0E0FACA660822799F8269C971C35EDAB6D23A5E52BEC07312C86C` |

Full results: `deploy/mayo-benchmark-results.json` (reproduce with `node deploy/benchmark-mayo-variants.cjs`).

**Notes**:
- Moultbook wired to pure verifier + jclaw-credential registry
- STAVR RPC endpoint used (Polkachu was unreachable from this environment)
- Gas price: 0.075 ujunox; store fee: 1.125 JUNOX per wasm
- jclaw-credential raw build had `reference-types` — used optimized devnet artifact instead

**Files**:
- `deploy/deployed-testnet.json` — testnet deployment artifacts
- `deploy/deploy-all-testnet.cjs` — deploy script (re-runnable, idempotent)

---

## Devnet (junoclaw-bn254-1) — ⚠️ Partial

**RPC**: `http://localhost:26657`
**Container**: `junoclaw-bn254-devnet`
**Admin**: `juno1chtrk3kgcgj09w53g0hqwqdlqs4rra637v0qd7`

| Contract | Code ID | Address | Status |
|----------|---------|---------|--------|
| zk-verifier (pure) | 1 | `juno14hj2...juwg8` | ✅ |
| zk-verifier (precompile) | 2 | `juno1nc5t...ev2p` | ✅ |
| jclaw-credential | 3 | `juno17p9r...8fr9` | ✅ |
| moultbook | — | — | ❌ Blocked by WSL2 restart loop |

## Devnet Stability Issues

The `junoclaw-bn254-devnet` on WSL2 suffers from **recurring consensus stalls**
caused by WSL2 clock jumps. Symptoms:

- `junod` process exits with code 255 every ~30–60 seconds
- Container stays alive (Docker `restart: unless-stopped` + entrypoint loop)
- RPC intermittently drops; mempool txs are lost on restart

**Mitigations applied** (`init-genesis.sh`):
1. `timeout_commit = "0s"` for instant block production
2. `client.toml` → `tcp://127.0.0.1:26657` to avoid IPv6 resolution hangs
3. `while true` restart loop inside the container to keep ports mapped

## Next Steps
See `NEXT_STEPS.md` for the work-up plan.
