# Juno Governance Proposal — JunoClaw: Verifiable AI Agents + Junoswap Revival

## Proposal Metadata

- **Title**: JunoClaw — Verifiable AI Agents with TEE-Attested Junoswap Revival on Juno
- **Type**: Signaling / Text Proposal
- **Proposer**: VairagyaNodes (validator, unbonded active, waiting position 6)
- **Deposit**: Required JUNO deposit (standard governance minimum)
- **Chain**: juno-1 (mainnet)

---

## Summary

JunoClaw is an open-source agentic AI platform built natively on Juno Network. We are proposing to revive Junoswap with TEE-attested verification, deploy verifiable AI agents on Akash decentralized compute, and establish a DAO-governed constitutional framework where both code upgrades and voting-power redistribution require 67% supermajority.

This proposal signals community support for JunoClaw's roadmap and requests recognition as official Juno ecosystem infrastructure.

---

## What JunoClaw Does

JunoClaw deploys AI agents that operate under on-chain governance. Every agent action — from DEX swap verification to data attestation — is:

1. **Proposed** by DAO members through weighted governance
2. **Verified** by WAVS operators running inside TEE hardware enclaves (Intel SGX / AMD SEV)
3. **Attested** on-chain with cryptographic proof that the computation was correct
4. **Executed** only after meeting quorum thresholds

This is not speculative — it is **proven on uni-7 testnet** with 5 executed proposals, TEE-hardware attestations, and live Junoswap v2 contracts.

---

## Three Integrations in One

### 1. Junoswap Revival

Junoswap v2 contracts (Astroport fork, Apache 2.0) are deployed and operational:

- **Factory**: Manages pair creation, fee configuration, JunoClaw hook integration
- **Pair contracts**: XYK AMM with 0.30% swap fee, TEE-attested price verification
- **Every swap** generates a WAVS verification event — the operator re-computes the expected output inside an enclave and attests correctness on-chain

This brings **verifiable DeFi** to Juno — no other Cosmos DEX has hardware-attested swap verification.

### 2. WAVS TEE Integration

WAVS (by Layer.xyz, founded by Ethan Frey + Jake Hartnell) provides:

- **Hardware attestation**: Computations run inside SGX/SEV enclaves
- **WASI components**: Deterministic verification logic compiled to WebAssembly
- **Operator network**: Multiple operators can verify the same computation
- **On-chain proofs**: Attestation hashes are stored in the agent-company contract

**Milestone achieved**: Proposal 4 on uni-7 was verified by a WAVS component running inside an SGX enclave on Azure DCsv3 hardware. The attestation hash is on-chain and independently verifiable.

### 3. Akash Decentralized Compute

The WAVS operator stack (operator + aggregator + IPFS) is packaged as an Akash SDL for deployment on decentralized compute. Akash is a marketplace where anyone can rent servers from independent providers — no single company controls the infrastructure.

- **No single point of failure**: Operator runs on any Akash provider; if one dies, redeploy to another
- **LIVE**: Aggregator at `http://provider.akash-palmito.org:31812` — deployed March 17, 2026
- **Payment in AKT**: 63.77 AKT funded (~3-5 months of operation at US$7.85/month)
- **Cost-efficient**: 3 containers (operator + aggregator + IPFS) on provider.akash-palmito.org
- **Not TEE (yet)**: Akash currently provides regular compute. TEE was proven separately on Azure (Proposal 4). When Akash providers offer AMD SEV hardware, we upgrade — the operator code is the same either way

This means JunoClaw's verification layer runs on **fully decentralized infrastructure** — Juno contracts + Akash compute + WAVS attestation. No Microsoft, no AWS, no single jurisdiction.

---

## Governance Architecture

### Genesis → 13 Buds Model

JunoClaw uses a novel "budding" governance model:

1. **Genesis phase**: A single genesis address prepares the entire contract database
2. **Budding**: Genesis distributes voting weight to 13 initial "buds" (DAO members)
3. **Post-budding**: The 13 buds vote on all further changes
4. **Constitutional proposals (`CodeUpgrade` + `WeightChange`) require 67% supermajority** — 9 of 13 buds must agree
5. **Further budding**: The DAO can vote to add more members, redistributing weight

This is implemented as a **CodeUpgrade proposal kind** with atomic multi-action execution — the DAO can bundle contract migrations, instantiations, and config changes into a single governance-gated proposal.

### Why 67% Supermajority

Constitutional changes are high-impact. Requiring 67% for code upgrades and weight redistribution ensures no small faction can unilaterally change infrastructure or voting-power structure.

---

## Vairagya Node Context

- **Validator status**: Unbonded, active, waiting position 6
- **Juno Discord**: Validator role held
- **Commitment**: Long-term Juno ecosystem builder
- **Open source**: All code at github.com/Dragonmonk111/junoclaw (Apache 2.0)
- **Jake Hartnell** (Juno co-founder, WAVS/Layer.xyz): Reviewed and endorsed — "very cool!", "junoclaw was long overdue", and publicly in Juno Telegram (4,581 members): **"Juno is going to be run by an AI soon."**

---

## What We're Asking For

This is a **signaling proposal**. We are asking the Juno community to:

1. **Recognize JunoClaw** as official Juno ecosystem infrastructure
2. **Support Junoswap revival** through TEE-attested verification
3. **Endorse decentralized compute** via Akash for Juno's verification layer
4. **Acknowledge the governance framework** — constitutional proposals (`CodeUpgrade` + `WeightChange`) with supermajority quorum
5. **Support the future validator sidecar proposal** — TEE-only Docker process alongside validator nodes for hardware-attested verification across the validator set

We are **not** asking for community pool funds at this time. JunoClaw is self-funded and operational.

---

## If This Proposal Passes

Passing this proposal signals that the Juno community recognizes VairagyaNodes as the steward of JunoClaw infrastructure. The following sequence begins:

1. **Root authority → Genesis address**: The genesis address (`juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m`) becomes the acknowledged root of JunoClaw on Juno mainnet.
2. **Genesis deploys mainnet contracts**: agent-company, Junoswap factory, escrow, task-ledger, agent-registry — all on juno-1.
3. **Genesis wires infrastructure**: Junoswap factory linked, WAVS operator configured, Akash deployment active.
4. **Genesis buds into 13**: WeightChange proposal distributes voting weight to 13 DAO members. Genesis retains symbolic weight (3/10000) and wasmd admin (emergency only).
5. **DAO governs**: The 13 buds control all future proposals. Constitutional changes (`CodeUpgrade` + `WeightChange`) require 67% supermajority (9 of 13).
6. **Validator sidecar proposal**: The DAO (not Genesis) asks validators to run TEE sidecars for hardware-attested verification.

**Timeline is decided by the Genesis Root.**

---

## Testnet Proof (uni-7)

| Item | Status | Evidence |
|------|--------|---------|
| Agent-company v3 | ✅ Live | Code ID 63, 5 proposals executed |
| Junoswap factory | ✅ Live | Code ID 61, 2 pairs created |
| Junoswap pairs | ✅ Live | JUNOX/USDC + JUNOX/STAKE |
| TEE attestation | ✅ Proven | Proposal 4, SGX enclave on Azure DCsv3 |
| CodeUpgrade prop | ✅ Executed | Proposal 5, Junoswap wired to agent-company |
| 34 unit tests | ✅ Passing | Including 3 supermajority quorum tests |
| WAVS component | ✅ Built | 355KB WASI, SHA-256 verification logic |
| Akash operator | ✅ LIVE | `http://provider.akash-palmito.org:31812` — 3 containers, chain healthy |
| Randomness/Sortition | ✅ Built | Dual-source: NOIS proxy (IBC) + WAVS drand. Fisher-Yates jury selection. 5 tests passing. |
| Validator sidecar | ✅ Proven | Same WASI component, TEE-only (SGX/SEV), docker-compose ready |
| Frontend | ✅ Built | Chat, DAO, DEX, Updates panels — 317KB Vite build |

---

## Timeline

| Date | Stage | Action | Who |
|------|-------|--------|-----|
| Mar 13, 2026 | 0 | Testnet contracts deployed (uni-7) | ✅ Genesis |
| Mar 16, 2026 | 1 | TEE proof on Azure — Proposal 4, SGX enclave | ✅ Genesis |
| Mar 17, 2026 | 2 | Junoswap deployed + wired — CodeUpgrade Proposal 5 | ✅ Genesis |
| Mar 17, 2026 | 3 | agent-company v3 migrated — code_id=63, supermajority 67% | ✅ Genesis |
| Mar 17, 2026 | 4 | WAVS operator deployed on Akash (regular compute, 63.77 AKT) | ✅ Genesis |
| Mar 17, 2026 | 5 | **This governance proposal submitted on juno-1** | VairagyaNodes |
| Mar 17–24, 2026 | — | Community voting period (~7 days) | Community |
| ~Mar 24, 2026 | 6 | If passes: Root authority → Genesis address acknowledged | Community |
| ~Mar 25–28, 2026 | 7 | Genesis deploys mainnet contracts on juno-1 | Genesis |
| ~Mar 28–31, 2026 | 8 | Genesis buds → 13 DAO members (WeightChange) | Genesis |
| ~Apr 1–7, 2026 | 9 | Validator sidecar proposal (TEE-only, by 13 buds) | DAO |
| ~Q2 2026 | 10 | JCLAW token launch (CW20 governance) | DAO |

*All post-prop timing decided by the Genesis Root.*

---

## Links

- **GitHub**: https://github.com/Dragonmonk111/junoclaw
- **TEE Milestone Article**: (published on Medium)
- **Akash Operator (LIVE)**: http://provider.akash-palmito.org:31812
- **Akash Integration Plan**: In repo at docs/AKASH_DEEP_INTEGRATION.md
- **Contracts**: All on uni-7, addresses in docs/GENESIS_BUDS_ARCHITECTURE.md

---

*Proposed by VairagyaNodes — Juno enthusiast since 30th Dec 2021.*
