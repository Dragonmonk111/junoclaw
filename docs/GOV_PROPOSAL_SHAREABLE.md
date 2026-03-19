# JunoClaw Governance Proposal — Shareable Draft

> **Status**: Draft v2 — waiting for context refill before finalizing Part 2+
> **Format**: Broken into chunks for easy review and sharing
> **Target**: juno-1 mainnet signaling proposal via ping.pub

---

## TITLE

```
JunoClaw — The Technology Will Be Given to the People
```

---

## ABSTRACT

JunoClaw is verifiable AI infrastructure built on Juno. It revives Junoswap with TEE-attested swap verification, deploys 10 autonomous chain intelligence workflows on Akash decentralized compute, and governs AI agents through on-chain DAO proposals with supermajority protection. Human swaps settle in 1 block (~3 seconds). Agent verification attests within 3 blocks (~9 seconds). Every attestation is permanent, queryable, and open source. No other chain in Cosmos has this.

---

## PART 1: WHY JUNO

On December 30th, 2021, I staked my first JUNO. The promise was clear — the world's first fully interoperable smart contract platform. Community-owned. No VCs. 64 million supply. Technology given to the people.

That original Medium article from @JunoNetwork is still up: "Junø Tokenomics and Utility: Powering the world's first fully interoperable smart contract."

Four years later, I'm still here. Not because of the price. Because the thesis was right.

Juno gave smart contracts to the people. JunoClaw gives AI agents to the people — verifiable, governed, running on hardware the operator can't tamper with. Built on Juno because this is where it belongs.

---

## PART 2: WHAT WE BUILT

JunoClaw is an open-source agentic AI platform built natively on Juno. AI agents operate under on-chain governance — every action is proposed by DAO members, verified by WAVS operators running inside TEE hardware enclaves (Intel SGX / AMD SEV), attested on-chain with cryptographic proof, and executed only after quorum.

This is not speculative. As of March 17, 2026:

- agent-company v3 contract live on uni-7 (code ID 63, 5 governance proposals executed)
- Junoswap v2 factory + 2 trading pairs deployed (JUNOX/USDC, JUNOX/STAKE)
- TEE hardware attestation proven — Proposal 4 ran inside Intel SGX enclave
  TX: 6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22
- CodeUpgrade governance with 67% supermajority quorum (9 of 13 buds)
- On-chain randomness via NOIS/drand for fair jury selection (sortition)
- WAVS operator stack LIVE on Akash decentralized compute
- 34 unit tests passing, Apache 2.0 licensed

---

## PART 3: THREE INTEGRATIONS

**Junoswap Revival**

Junoswap v2 contracts are live on testnet. The factory is wired into DAO governance. Every swap triggers a WAVS verification event — the operator independently recomputes swap math and attests correctness on-chain. Human swaps settle in 1 block (~3 seconds). The WAVS agent catches, verifies, and attests within 3 blocks (~9 seconds). That's a verified DEX trade in under 10 seconds — no other Cosmos DEX does this. Jake Hartnell said it: "We should point the agent at projects like reviving Junoswap." That's exactly what this does.

**WAVS TEE Integration**

WAVS (Layer.xyz, co-founded by Jake Hartnell) provides hardware-attested off-chain compute. JunoClaw's WASI verification component runs inside SGX/SEV enclaves — the hardware signs the computation, not just the operator. Proposal 4 is the first hardware-attested WAVS result submitted to a Cosmos chain.

**Akash Decentralized Compute**

The WAVS operator stack (operator + aggregator + IPFS) runs LIVE on Akash Network at http://provider.akash-palmito.org:31812 — permissionless, decentralized, can't be shut down. Cost: US$7.85/month. No VC funding needed. Just like Juno itself.

---

## PART 4: CHAIN INTELLIGENCE MODULE — 10 ACTIVE WORKFLOWS

The WAVS operator runs autonomous verification workflows. No human in the loop. Every result attested permanently on-chain.

**DEX Intelligence**

1. **Swap Verification** — Every Junoswap swap independently verified. XYK math recomputed, constant-product invariant (k) checked, price impact measured, manipulation flagged at >5% slippage.

2. **Whale Flow Tracking** — Large trades classified in real-time (normal / large / whale / mega-whale). Trade size as percentage of pool reserves calculated. Sandwich attack risk signaled. Slippage cost analysis per trade.

3. **Pool Health Monitor** — Tracks reserve ratios, liquidity depth, volume patterns. Flags pools approaching dangerous imbalance before users get hurt.

4. **Price Attestation** — Periodic on-chain price snapshots with WAVS attestation. Creates a verifiable price oracle that doesn't depend on external feeds.

**Security & Governance**

5. **Governance Watch** — Monitors DAO proposals for attack patterns: rapid quorum rushes, unanimous votes, admin privilege transfers, DEX-affecting proposals, suspiciously fast execution. Risk-scored and attested.

6. **Migration Watchdog** — Every contract migration checked against known registries. Unauthorized code upgrades flagged before they execute. Verifies proposal authorization and code ID ranges.

7. **Outcome Verification** — Prediction market resolution verified against external sources. TEE hardware guarantees the verification code wasn't tampered with.

**Chain Health**

8. **IBC Health Check** — Monitors IBC channel state, packet loss rate, relay efficiency. Classifies channel health (healthy / degraded / critical / dead). Flags issues before they affect users.

9. **Sortition (Random Jury)** — Fetches drand randomness and submits on-chain. Fisher-Yates shuffle with SHA-256 sub-randomness — deterministic, verifiable, fair. Used for dispute resolution and random audits.

10. **Data Verification** — General-purpose data attestation. Any structured input can be verified, hashed, and attested. Foundation for future workflows the DAO defines.

Each workflow follows the same pattern:
```
Trigger event on Juno → WAVS operator catches it → WASI component verifies → attestation hash submitted back to contract
```

Fully autonomous. Every attestation is permanent and queryable.

---

## PART 5: GOVERNANCE — GENESIS TO 13 BUDS

Same philosophy as Juno's own fairdrop — distribute power, don't hoard it.

- Genesis address holds 100% weight during setup
- Genesis submits WeightChange → distributes to 13 initial DAO members ("buds")
- Each bud: 769/10000 weight. Genesis retains 3/10000 (symbolic)
- Normal proposals: 51% quorum (7 of 13 buds)
- Code upgrades: 67% supermajority (9 of 13 buds)
- Further budding: DAO adds members via WeightChange proposals

The Genesis Root is urged to evolve JunoClaw and involve up to 13 buds to do so. The timeline is with Genesis. This is not a centralized project with a corporate roadmap — it's a seed planted in Juno soil. Genesis decides when to bud, who to bud, and what to build next. The community's role is to watch the tree grow and prune bad branches via BreakChannel if needed.

Once the 13 buds are active, Genesis loses voting power. The DAO self-governs. Code upgrades require 9 of 13 (67%). No single actor can push changes unilaterally. This is how decentralized infrastructure should work.

---

## PART 6: JUNOSWAP — THE CODE

Junoswap has been a sore point for Juno for a long time. The original Junoswap v1 contracts were abandoned. Liquidity dried up. Traders left. The chain that pioneered CosmWasm smart contracts had no functioning DEX.

JunoClaw fixes this. Junoswap v2 is a clean rewrite — two contracts, open source, Apache 2.0.

**Factory Contract** (`junoswap-factory`)

The factory manages pair creation and global DEX configuration. It stores a reference to the JunoClaw agent-company contract for governance integration.

- `CreatePair { token_a, token_b, fee_bps }` — Anyone can create a new trading pair. Assets are sorted deterministically to prevent duplicates. Default fee: 30 bps (0.30%). The factory instantiates a new pair contract from a stored code ID.
- `UpdateConfig` — Owner-only. Can update pair code ID (for upgrades), default fee, and JunoClaw contract reference.
- `Pair` / `AllPairs` / `PairCount` queries — Full pair registry with pagination.
- Duplicate protection: identical asset pairs are rejected. Fee validation: max 10000 bps.

Factory config on uni-7: code ID 61, address `juno12v0t60...`, wired into agent-company via Proposal 5 (SetDexFactory).

**Pair Contract** (`junoswap-pair`)

Each pair is a standalone XYK constant-product AMM. The core swap logic:

```
fee_amount = offer_amount * fee_bps / 10000
offer_after_fee = offer_amount - fee_amount
return_amount = offer_after_fee * return_reserve / (offer_reserve + offer_after_fee)
spread_amount = (offer_after_fee * return_reserve / offer_reserve) - return_amount
```

This is the standard constant-product formula (x * y = k). Fees are deducted from the offer side before computing the return. The spread (slippage) is calculated separately for transparency.

Key features:
- **Liquidity provision**: Proportional LP shares minted on deposit. Geometric mean for initial deposit.
- **Withdrawal**: Burns LP tokens, returns proportional reserves.
- **Slippage protection**: `min_return` parameter rejects trades that slip beyond the user's tolerance.
- **Empty pool protection**: Swaps against empty reserves are rejected.
- **Volume tracking**: `total_swaps`, `total_volume_a`, `total_volume_b`, `last_swap_block` stored on-chain.
- **Simulate swap**: Read-only query to preview trade outcome before committing.
- **LP balance query**: Per-address LP share tracking.

Every swap emits a `wasm-swap` event with 12 attributes: pair, sender, offer_asset, offer_amount, return_asset, return_amount, spread_amount, fee_amount, reserve_a, reserve_b, block_height, timestamp. This is what the WAVS operator watches. Within 3 blocks (~9 seconds), the agent independently recomputes the math and attests correctness on-chain.

Pair contracts on uni-7:
- JUNOX/USDC: `juno1xn4mtv9...` (code ID 60)
- JUNOX/STAKE: `juno156t270z...` (code ID 60)

**Why This Matters**

Juno had the first CosmWasm smart contracts in Cosmos. It should have a functioning DEX. Junoswap v2 is minimal, auditable, and fully governed by the JunoClaw DAO. Every swap is verified by an off-chain agent in hardware-attested compute. No other DEX in Cosmos does this.

The code is 438 lines for the pair contract, 209 for the factory. Small enough to audit in an afternoon. Tested with 34 unit tests including constant-product invariant checks, slippage protection, empty pool guards, and event emission verification.

---

## PART 7: WHAT THIS MEANS

We're not asking for recognition or funding. We're builders who showed up and built.

Junoswap is being revived. Swaps are being verified. Governance is being watched. IBC channels are being monitored. All of it attested, all of it on-chain, all of it open source.

If this passes, we continue building. If it doesn't, we continue building. The code is already deployed.

But a yes from the community means something. It means builders are shipping on Juno. It means the chain is alive. It means Juno is still where the technology is given to the people.

---

## PART 8: LINKS

- Code (Apache 2.0): https://github.com/Dragonmonk111/junoclaw
- TEE Attestation TX: 6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22 (uni-7)
- Agent-company: juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6 (uni-7)
- Akash Operator (LIVE): http://provider.akash-palmito.org:31812
- Junoswap Factory: juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh (uni-7)
- Junoswap Pair (JUNOX/USDC): juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98 (uni-7)
- Junoswap Pair (JUNOX/STAKE): juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s (uni-7)

Proposed by VairagyaNodes — staking Juno since December 30th, 2021.
