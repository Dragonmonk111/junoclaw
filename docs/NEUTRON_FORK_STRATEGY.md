# Neutron Fork Strategy
## JunoClaw — Inheriting Dead DeFi for Juno

**Context**: Jake Hartnell (March 17, 2026):
> "Neutron is dead now so the agent could probably fork some of those projects as well."
> "We should point the agent at projects like reviving Junoswap, etc."

---

## What Happened to Neutron

Neutron was a Cosmos consumer chain (Replicated Security via Cosmos Hub) focused on DeFi. It hosted several protocols that migrated from Terra and built natively. As of early 2026, Neutron's activity has collapsed — protocols are abandoned, TVL drained, development halted.

**But the code is open source.** Every contract, every frontend, every integration guide — all Apache-2.0 or MIT licensed on GitHub.

---

## Forkable Neutron Projects

| Project | What it does | GitHub | Fork value for Juno |
|---------|-------------|--------|---------------------|
| **Astroport** | DEX (concentrated liquidity, stable pools) | github.com/astroport-fi | Replace/revive Junoswap with modern AMM |
| **Mars Protocol** | Lending/borrowing (Red Bank) | github.com/mars-protocol | Juno-native lending market |
| **Apollo DAO** | Yield vaults, auto-compounding | github.com/apollodao | DeFi yield layer on Juno |
| **Neutron DAO** | SubDAO governance framework | github.com/neutron-org/neutron-dao | Advanced governance for JunoClaw |
| **Drop Protocol** | Liquid staking (dATOM, etc.) | github.com/hadronlabs-org | Liquid staking for JUNO |
| **Duality DEX** | Orderbook-style DEX | github.com/neutron-org/dex | Alternative DEX model |

---

## What "Fork" Means Here

Not just copy-paste. The WAVS operator adds a **verification layer** that the originals never had:

```
Original (on Neutron):
  User → Protocol → Trust the protocol code

Forked (on Juno + JunoClaw):
  User → Protocol → WAVS operator verifies state →
  Hardware-attested proof → On-chain attestation →
  Protocol action confirmed by TEE
```

The agent doesn't just run the protocol — it **watches and verifies** it. Every price feed, every liquidation trigger, every pool rebalance can be TEE-attested.

---

## Priority Forks

### 1. Junoswap Revival (Highest Priority — Jake's explicit direction)

**What**: Fork Astroport's concentrated liquidity AMM contracts, deploy on Juno, rename to Junoswap v2.

**Why**: Juno had Junoswap (original Juno DEX by @CoreDevDAO). It died. Astroport's contracts are more advanced (CL pools, stable pools, multi-hop). Forking them gives Juno a modern DEX immediately.

**WAVS integration**: The operator verifies:
- Pool price feeds (TEE-attested TWAP oracles)
- Large swap impact warnings
- Liquidity depth checks
- Cross-chain price consistency (vs CoinGecko/Osmosis)

**New WASI component needed**: `junoclaw-dex-verifier`
- Triggers on: `wasm-swap`, `wasm-provide_liquidity`, `wasm-withdraw_liquidity`
- Produces: price attestations, slippage checks, manipulation alerts

### 2. Lending Market (Mars Protocol Fork)

**What**: Fork Mars Protocol's Red Bank, deploy on Juno.

**Why**: Juno has no lending market. Mars was the best in Cosmos. The contracts are production-tested.

**WAVS integration**: The operator verifies:
- Oracle price feeds for collateral valuation
- Liquidation threshold checks
- Interest rate model verification
- Bad debt detection

**New WASI component needed**: `junoclaw-lending-verifier`
- Triggers on: `wasm-deposit`, `wasm-borrow`, `wasm-liquidate`
- Produces: collateral ratio attestations, liquidation validity proofs

### 3. Liquid Staking (Drop Protocol Fork)

**What**: Fork Drop's liquid staking for JUNO (produce stJUNO or similar).

**Why**: Liquid staking is table stakes for any L1. Juno doesn't have a native solution.

**WAVS integration**: The operator verifies:
- Staking/unstaking ratios
- Validator set health
- Exchange rate accuracy

---

## Upgradeable Neutron Parameters

Several Neutron-originated parameters and modules can be integrated into Juno governance via JunoClaw proposals:

### Chain-Level Parameters (Governance Proposals)

| Parameter | Neutron value | Juno current | Upgrade path |
|-----------|--------------|--------------|-------------|
| **ICA (Interchain Accounts)** | Enabled | Enabled | Already compatible — fork protocols can use ICA |
| **ICQ (Interchain Queries)** | Native module | Not native | Requires chain upgrade OR WAVS workaround |
| **TokenFactory** | Enabled | Enabled (v47+) | Already compatible — fork protocols can create tokens |
| **Cron module** | Native (Begin/End block) | Not native | WAVS operator replaces this — TEE-attested cron |
| **Feeburner** | Burns fees | Standard distribution | Governance proposal to change fee distribution |
| **ContractManager** | Sudo callbacks | Not available | WAVS operator can simulate via event watching |

### Key Insight: WAVS Replaces Missing Chain Modules

Neutron had native chain modules (ICQ, Cron, ContractManager) that Juno doesn't. But **WAVS can replicate their functionality off-chain with TEE attestation**:

| Missing module | WAVS replacement |
|---------------|-----------------|
| **ICQ (Interchain Queries)** | WAVS component queries remote chains via HTTP, returns TEE-attested result |
| **Cron (scheduled execution)** | WAVS operator watches block height, triggers at intervals, TEE-attested |
| **ContractManager (sudo callbacks)** | WAVS watches contract events, executes callback logic, submits result |
| **Oracle module** | WAVS fetches price data from multiple sources, TEE-attests the median |

This is the core insight: **you don't need to fork Neutron's chain modules if you have WAVS**. The operator IS the missing infrastructure.

---

## Implementation Plan

### Phase 1 — Junoswap Revival (Weeks 1–2)
1. Fork Astroport contracts from `github.com/astroport-fi/astroport-core`
2. Rename to Junoswap v2, rebrand
3. Deploy on Juno testnet (uni-7)
4. Write `junoclaw-dex-verifier` WASI component
5. WAVS operator verifies swap events with TEE attestation
6. Tweet: "Junoswap is back. Now with hardware-attested price verification."

### Phase 2 — Lending Market (Weeks 3–4)
1. Fork Mars Protocol from `github.com/mars-protocol/red-bank`
2. Deploy on Juno testnet
3. Write `junoclaw-lending-verifier` WASI component
4. WAVS operator verifies collateral, liquidations, oracle feeds

### Phase 3 — Liquid Staking (Weeks 5–6)
1. Fork Drop Protocol
2. Deploy stJUNO contract on Juno testnet
3. WAVS operator verifies staking ratios and validator health

### Phase 4 — Mainnet Governance (Week 7+)
1. Submit Juno governance proposal: "Deploy Junoswap v2 + JunoClaw verification layer"
2. Validators already running sidecars from the proposal thread
3. Community votes → protocols go live on mainnet

---

## The Narrative

> Neutron built great DeFi infrastructure and then abandoned it. The code is open source. JunoClaw forks the best of it, deploys on Juno, and adds something Neutron never had: hardware-attested verification of every protocol action.
>
> Juno doesn't need to build from scratch. It needs to inherit smartly and verify trustlessly.
>
> The WAVS operator is the difference between "we forked their code" and "we forked their code and every transaction is TEE-verified."

---

## Files to Create

- [ ] `junoclaw/contracts/junoswap-v2/` — Forked Astroport contracts (renamed)
- [ ] `junoclaw/wavs/components/dex-verifier/` — WASI component for DEX verification
- [ ] `junoclaw/wavs/components/lending-verifier/` — WASI component for lending verification
- [ ] `junoclaw/docs/JUNOSWAP_REVIVAL.md` — Detailed Junoswap v2 plan
- [ ] `junoclaw/docs/NEUTRON_FORK_GUIDE.md` — Step-by-step fork guide for each protocol
