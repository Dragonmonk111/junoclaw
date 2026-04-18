# JunoClaw Staging Hierarchy — The Order of Operations

> Read this first. Everything else references this sequence.

---

## The Three Compute Layers (They Are NOT the Same Thing)

```
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│  LAYER 1: AKASH (the always-on worker)                             │
│  ├── Regular compute, NO TEE                                       │
│  ├── Runs 24/7 on rented Akash servers                             │
│  ├── Watches Juno → runs WASI verify → submits attestations       │
│  ├── Cost: ~15 AKT/month                                          │
│  ├── Purpose: Ensure verification never stops                      │
│  └── Status: SDL ready, 63.77 AKT funded, awaiting deployment     │
│                                                                    │
│  LAYER 2: VALIDATOR SIDECARS (the trust amplifiers)                │
│  ├── TEE ONLY — Intel SGX or AMD SEV hardware required             │
│  ├── Runs alongside a Juno validator node                          │
│  ├── Same WASI component, but inside a hardware enclave            │
│  ├── Attestation is hardware-signed (unforgeable)                  │
│  ├── Cost: Free to validators (uses their existing hardware)       │
│  ├── Purpose: Distributed TEE attestation across validator set     │
│  └── Status: Proven on Azure SGX (Proposal 4), awaiting adoption  │
│                                                                    │
│  LAYER 3: ON-CHAIN RANDOMNESS (the jury selector)                 │
│  ├── NOIS proxy (IBC-based drand) or WAVS-attested drand          │
│  ├── Runs inside the agent-company contract itself                 │
│  ├── Fisher-Yates shuffle to randomly select DAO members           │
│  ├── Purpose: Fair, verifiable jury selection for disputes          │
│  └── Status: Implemented, 5 tests passing, dual-source ready      │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

**Key distinction:**
- Akash = **availability** (keep the operator running)
- Sidecars = **trust** (hardware proves the computation)
- Randomness = **fairness** (nobody can pick the jury)

---

## The Staging Sequence

```
STAGE  DATE            WHAT                                      STATUS      WHO
─────  ──────────────  ────────────────────────────────────────  ────────    ────────
  0    Mar 13, 2026    Testnet contracts deployed (uni-7)        ✅ DONE     Genesis
  1    Mar 16, 2026    TEE proof on Azure (Proposal 4, SGX)      ✅ DONE     Genesis
  2    Mar 17, 2026    Junoswap deployed + wired (Proposal 5)    ✅ DONE     Genesis
  3    Mar 17, 2026    agent-company v3 migrated (code_id=63)    ✅ DONE     Genesis
  4    Mar 17, 2026    Akash LIVE: provider.akash-palmito.org:31812 ✅ DONE   You
  5    Mar 17, 2026    Juno governance proposal on juno-1        ✅ TODAY    VairagyaNodes
  ─── IF PROP PASSES ──────────────────────────────────────────────────────────────
  6    ~Mar 24, 2026   ROOT → GENESIS ADDRESS                    ⏳          Community
                       Juno community acknowledges VairagyaNodes
                       as steward. Genesis address is root.
  7    ~Mar 25-28      Genesis deploys mainnet contracts          ⏳          Genesis
                       agent-company + Junoswap on juno-1
  8    ~Mar 28-31      Genesis BUDS → 13 members                  ⏳          Genesis
                       WeightChange: 13 × 769 weight
                       Genesis drops to 3 (symbolic)
  9    ~Apr 1-7, 2026  Validator sidecar proposal (TEE-only)      ⏳          DAO (13 buds)
                       FreeText prop — SEPARATE from gov prop
 10    ~Q2 2026        JCLAW token launch (CW20 governance)       ⏳          DAO (13 buds)
```

---

## What "Root Moves to Genesis" Means

The Juno governance proposal (Stage 5) is a **signaling proposal**. It doesn't execute code on mainnet. Instead:

1. **Before prop passes**: VairagyaNodes is just another validator building stuff on testnet
2. **After prop passes**: The Juno community has formally acknowledged JunoClaw as Juno ecosystem infrastructure. VairagyaNodes' genesis address is the recognized root.
3. **Root authority**: The genesis address deploys mainnet contracts with itself as admin. It has 100% voting weight — total control during setup.
4. **Root relinquishes**: After wiring infrastructure, genesis buds into 13. Root keeps wasmd admin (emergency migrations) but loses voting power (3/10000 = 0.03%).

```
Juno Gov Prop passes
        │
        ▼
Genesis address = ROOT of JunoClaw on juno-1 (mainnet)
        │
        ├── Deploy contracts (agent-company, Junoswap, escrow, etc.)
        ├── Wire Junoswap factory into agent-company
        ├── Configure WAVS operator
        │
        ▼
Genesis submits WeightChange proposal (auto-passes at 100%)
        │
        ▼
13 BUDS now control governance
        │
        ├── Normal proposals: 51% (7 buds)
        ├── Code upgrades: 67% (9 buds)
        ├── Weight changes: 67% (9 buds)
        ├── Can add more buds via WeightChange (constitutional path)
        │
        ▼
DAO is sovereign. Genesis is symbolic.
```

---

## File Index (Read in This Order)

| # | File | What It Covers |
|---|------|---------------|
| 00 | `docs/00_STAGING_HIERARCHY.md` | **This file.** Master sequence. |
| 01 | `docs/01_VALIDATOR_SIDECARS.md` | What sidecars are. Why TEE-only. How they differ from Akash. |
| 02 | `docs/02_RANDOMNESS_SORTITION.md` | On-chain randomness. Fisher-Yates. NOIS vs WAVS drand. |
| — | `docs/AKASH_WALKTHROUGH.md` | Akash deployment: what we rent, run, connect. |
| — | `docs/GENESIS_BUDS_ARCHITECTURE.md` | Genesis → 13 buds weight distribution. |
| — | `docs/JUNO_GOVERNANCE_PROPOSAL.md` | The signaling prop text for juno-1. |
| — | `JUNOSWAP_GOVERNANCE_ARTICLE.md` | Medium article: updates + whole picture. |
| — | `VALIDATOR_PROPOSAL_THREAD.md` | Twitter thread for validator sidecar ask. |
