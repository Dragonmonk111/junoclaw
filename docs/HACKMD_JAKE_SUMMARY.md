# JunoClaw — Status Summary for Jake

*April 13, 2026 — Everything that's happened since Proposal #373*

---

## Quick Links

| Resource | Link |
|----------|------|
| **GitHub** | https://github.com/Dragonmonk111/junoclaw |
| **Medium: ZK Precompile** | https://medium.com/@tj.yamlajatt/killing-eth-with-love-why-juno-is-stealing-ethereums-best-idea-1875f1879a7c |
| **Medium: Cosmos MCP** | https://medium.com/@tj.yamlajatt/the-first-ai-that-speaks-cosmos-building-juno-new-2a0253cbce91 |
| **Proposal #373** | https://ping.pub/juno/gov/373 |

---

## What JunoClaw Is

An open-source agentic AI platform built natively on Juno. AI agents operate under on-chain governance — every action is proposed, verified (via WAVS TEE), attested on-chain, and executed only after meeting quorum. Apache 2.0. Built by VairagyaNodes.

---

## What's Been Built (Testnet uni-7)

### Core Contracts (9 contracts, 109 tests passing)

| Contract | Code ID | Status |
|----------|---------|--------|
| agent-company v4 | 63 | Live — DAO governance hub, 5+ proposals executed |
| Junoswap v2 factory | 61 | Live — pair creation, fee config, JunoClaw hook |
| Junoswap v2 pair (JUNOX/USDC) | 60 | Live |
| Junoswap v2 pair (JUNOX/STAKE) | 60 | Live |
| zk-verifier | 64 | Live — pure CosmWasm Groth16 BN254 verifier |
| agent-registry | — | Built — agent identity + reputation |
| task-ledger | — | Built — task lifecycle with atomic callbacks |
| escrow | — | Built — non-custodial payment tracking |
| builder-grant | — | Built — builder grant lifecycle |

### WAVS TEE Integration (Proven)

- WASI verification component: 494KB, deterministic
- **Hardware-attested on Intel SGX** (Azure DCsv3) — Proposal 4 on uni-7
- TEE attestation TX: `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22`
- 5 autonomous verification workflows: swap verification, sortition, outcome verification, governance watch, migration watch

### Akash Decentralized Infrastructure

- 4 containers: WAVS operator, aggregator, warg-registry, IPFS
- Cost: ~US$8.76/month, funded with 63.77 AKT
- Zero centralized cloud dependency

### Governance: Genesis → 13 Buds

- Genesis address: `juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m`
- Currently 100% weight (pre-budding)
- Budding: 13 buds × 769 weight, genesis keeps 3 (symbolic)
- Code upgrades require 67% supermajority (9 of 13)
- Weight change guardrails (Rattadan hardening): delta cap 20%, cooldown 500 blocks, floor 1 bps

---

## What's New Since Proposal #373

### 1. ZK Precompile PoC (zk-verifier contract)

**Article**: https://medium.com/@tj.yamlajatt/killing-eth-with-love-why-juno-is-stealing-ethereums-best-idea-1875f1879a7c

Pure CosmWasm Groth16 BN254 verifier — the same elliptic curve pairing Ethereum has had since Byzantium (2017). No EVM, no fork, pure Rust + arkworks compiled to WASM.

- **Deployed on uni-7**: Code ID 64
- **Verification key stored**: 296 bytes on-chain
- **Proof verified on-chain**: TX `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA`
- **Gas cost**: 371,486 gas (works, but impractical without a precompile)
- **The ask**: Three host functions in wasmvm — `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_check` — implemented in Go using gnark-crypto. This would drop gas from 371K to ~3K, enabling privacy protocols, zkRollups, and credential verification natively on Juno.

### 2. Rattadan Variable Hardening (v4)

After Rattadan's structural audit, three hardening passes were applied:

| Variable | Risk Before | Fix |
|----------|-------------|-----|
| `attestation_hash` | Any hex string accepted blindly | On-chain SHA-256 re-computation |
| `status` | Independent per-contract, desync possible | Atomic cross-contract callbacks |
| `weight` | No cap, no cooldown — 51% coalition can zero minorities | Delta cap (20%) + cooldown (500 blocks) + floor (1 bps) |

### 3. Cosmos MCP Server (juno.new)

**Article**: https://medium.com/@tj.yamlajatt/the-first-ai-that-speaks-cosmos-building-juno-new-2a0253cbce91

The first Model Context Protocol server for the entire Cosmos ecosystem. Any AI assistant (Claude, Windsurf, Cursor) can now:
- Query any Cosmos chain (balance, contract state, TX lookup, block height)
- Deploy contracts (upload WASM, instantiate, migrate)
- Scaffold CosmWasm projects from 9 DAO templates

**16 tools, 5 chains (Juno, Osmosis, Stargaze, Neutron, uni-7), 9 DAO templates.**

- 12/12 live smoke tests pass against uni-7
- Write-path signing validated on-chain: TX `EE9A8FA6E7E6F6A77301DE6DC9A9E6A27D398AE7D071CAFCA2934352B8FB9327`
- Code: https://github.com/Dragonmonk111/junoclaw/tree/main/mcp

---

## Proposal #373 — Status

- **Submit TX**: `FAE98E6EAD6C23440FF614DE5973FA8D9A109FF68CDA6991031E1D8598DB3C9C`
- **Deposit**: 5,000 JUNO (1,000 initial + 4,000 top-up)
- **Type**: Signaling / text proposal (no code execution, no fund request)
- **What it asked**: Recognize JunoClaw as ecosystem infrastructure, endorse Junoswap revival, support Akash + validator sidecar architecture
- **Vote link**: https://ping.pub/juno/gov/373

---

## Architecture Diagram (Text)

```
┌─────────────────────────────────────────────────────┐
│                    JUNO CHAIN                        │
│                                                     │
│  agent-company ──── junoswap-factory                │
│       │                   │                         │
│   proposals            pairs ──── WAVS events       │
│   attestations         swaps                        │
│       │                                             │
│  task-ledger ←──→ escrow ←──→ agent-registry        │
│  (atomic callbacks)                                 │
│                                                     │
│  zk-verifier (Groth16 BN254)                       │
└─────────────────┬───────────────────────────────────┘
                  │ events
          ┌───────┴────────┐
          │  WAVS OPERATOR │  ← Akash (software mode)
          │  + aggregator  │  ← Validator sidecar (TEE mode)
          │  + warg-registry│
          │  + IPFS        │
          └────────────────┘
                  │
          ┌───────┴────────┐
          │  COSMOS MCP    │  ← Any AI assistant
          │  (16 tools)    │
          └────────────────┘
```

---

## What's Next

1. **Mainnet deployment** (pending governance signal)
2. **13-bud distribution** (WeightChange proposal)
3. **Validator sidecar rollout** (TEE attestation across validator set)
4. **BN254 precompile proposal** to wasmvm (the ZK article's ask)
5. **IBC tools** in the MCP server (cross-chain transfers)
6. **juno.new web UI** (browser frontend wrapping the scaffold tool)

---

## Who's Building This

**VairagyaNodes** — Juno staker since December 30, 2021. Validator candidate (unbonded, position 6). Solo developer. Validator role in Juno Discord.

- GitHub: https://github.com/Dragonmonk111
- All code: Apache 2.0

---

*Questions? Feedback? This document is editable — share and annotate freely.*
