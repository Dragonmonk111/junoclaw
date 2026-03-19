# Junoswap v2 ↔ JunoClaw Architecture

## How Junoswap v2 Integrates Into JunoClaw

---

## Overview

Junoswap v2 is not a standalone DEX — it's a **JunoClaw-native DEX** where every swap, every liquidity event, and every price snapshot can be TEE-attested by the WAVS operator. This is the differentiator: no other Cosmos DEX has hardware-verified price feeds built into the protocol.

```
┌────────────────────────────────────────────────────────┐
│                    Juno Chain (uni-7)                   │
│                                                        │
│  ┌──────────────────┐    ┌──────────────────────────┐  │
│  │  junoswap-factory │    │  agent-company            │  │
│  │  - creates pairs  │    │  - DAO governance         │  │
│  │  - tracks pairs   │    │  - proposals + voting     │  │
│  └────────┬─────────┘    │  - attestation storage    │  │
│           │              └──────────┬───────────────┘  │
│  ┌────────▼─────────┐              │                   │
│  │  junoswap-pair    │              │                   │
│  │  - XYK AMM swap   │──── events ──┤                   │
│  │  - provide/withdraw│  (wasm-swap) │                   │
│  │  - fee collection  │              │                   │
│  └──────────────────┘              │                   │
│                                    │                   │
└────────────────────────────────────┼───────────────────┘
                                     │
                          ┌──────────▼───────────┐
                          │  WAVS Operator        │
                          │  (Akash / Azure TEE)  │
                          │                       │
                          │  ┌─────────────────┐  │
                          │  │ WASI Component   │  │
                          │  │                  │  │
                          │  │ SwapVerify       │  │
                          │  │ PoolHealthCheck  │  │
                          │  │ PriceAttestation │  │
                          │  │ OutcomeVerify    │  │
                          │  │ DataVerify       │  │
                          │  │ DrandRandomness  │  │
                          │  └─────────────────┘  │
                          └──────────┬───────────┘
                                     │
                          ┌──────────▼───────────┐
                          │  Bridge Daemon        │
                          │  (TypeScript/CosmJS)  │
                          │  submit_attestation() │
                          └──────────────────────┘
```

---

## Event Flow

### Swap Verification

```
1. User calls junoswap-pair::Swap { offer_asset: "ujuno", min_return: None }
2. Contract emits wasm-swap event with:
   - pair, sender, offer_asset, offer_amount
   - return_asset, return_amount, spread_amount, fee_amount
   - reserve_a, reserve_b, block_height, timestamp
3. WAVS operator watches for wasm-swap events
4. WASI component receives the event → process_swap_verify()
5. Component checks:
   - XYK constant product holds
   - Price impact < 5% threshold
   - Fee correctly deducted
   - Returns: manipulation_flag, effective_price, attestation_hash
6. Bridge daemon submits attestation to agent-company contract
7. On-chain: attestation stored permanently, queryable by anyone
```

### Pool Health Check

```
1. User calls junoswap-pair::ProvideLiquidity or WithdrawLiquidity
2. Contract emits wasm-provide_liquidity or wasm-withdraw_liquidity
3. WAVS component → process_pool_health()
4. Checks: reserve balance ratio, LP dilution, health classification
5. Returns: health status (healthy/imbalanced/critical)
```

### Price Oracle (TEE-Attested)

```
1. Bridge daemon periodically queries pair reserves
2. Sends PriceAttestation task to WAVS operator
3. Component computes price from reserves inside TEE
4. Returns: TEE-attested price_a_per_b, price_b_per_a
5. Attestation stored on-chain → serves as verifiable price oracle
```

---

## Contract Addresses (Testnet Plan)

| Contract | Status | Purpose |
|----------|--------|---------|
| `agent-company` | ✅ Deployed | DAO governance + attestation storage |
| `agent-registry` | ✅ Deployed | Agent registration |
| `task-ledger` | ✅ Deployed | Task tracking |
| `escrow` | ✅ Deployed | Payment ledger |
| `junoswap-factory` | 🔨 Built + tested | Creates and manages pairs |
| `junoswap-pair` | 🔨 Built + tested | XYK AMM with WAVS event hooks |

---

## WAVS Component Task Types

| Task Type | Trigger Event | What It Verifies |
|-----------|---------------|-----------------|
| `swap_verify` | `wasm-swap` | Price correctness, manipulation detection |
| `pool_health_check` | `wasm-provide_liquidity`, `wasm-withdraw_liquidity` | Reserve balance, LP health |
| `price_attestation` | Periodic / on-demand | TEE-attested price snapshot |
| `outcome_verify` | `wasm-outcome_create` | Market resolution data |
| `data_verify` | `wasm-wavs_push` | External data fetch + hash |
| `drand_randomness` | `wasm-sortition_request` | drand beacon randomness |

---

## What Makes This Different

### vs. Astroport/Osmosis/Junoswap v1

| Feature | Traditional DEX | Junoswap v2 (JunoClaw) |
|---------|----------------|----------------------|
| Price feeds | Trust the reserves | **TEE-attested price oracle** |
| Swap verification | None | **Every swap verified in SGX enclave** |
| Manipulation detection | Off-chain only | **On-chain attestation for suspicious swaps** |
| Pool health | Manual monitoring | **Automated TEE-attested health reports** |
| Governance | Separate | **Integrated with JunoClaw DAO proposals** |

### The Oracle Problem — Solved

Most DeFi protocols rely on external oracles (Chainlink, Band, Pyth) for price data. These oracles are:
- Centralized (trust the oracle operator)
- Expensive (gas costs per update)
- Latent (updates on intervals, not real-time)

Junoswap v2's approach:
- Prices computed from on-chain reserves (no external dependency)
- TEE-attested (hardware guarantees correctness)
- Every swap produces a price attestation (real-time)
- Stored permanently on-chain (historical prices queryable)

---

## Deployment Plan

### Phase 1 — Testnet (uni-7)
1. Deploy `junoswap-factory` contract
2. Deploy `junoswap-pair` code (uploaded, factory creates instances)
3. Create JUNO/USDC test pair
4. Provide initial liquidity
5. Execute test swaps → verify WAVS events fire
6. WAVS operator attests swap events → store on-chain
7. Query attestations to confirm TEE-verified prices

### Phase 2 — Bridge Integration
1. Update bridge daemon to watch for `wasm-swap` events from junoswap pairs
2. Route swap events to WAVS operator alongside existing DAO events
3. Submit swap attestations to agent-company contract
4. Build query endpoint: `get_price_attestation { pair, block_height }`

### Phase 3 — Frontend
1. Add Junoswap v2 swap UI to JunoClaw frontend
2. Show TEE-attested price next to each swap
3. Pool health dashboard with live attestation status
4. Price history chart from on-chain attestations

---

## Service Manifest Update

The WAVS service manifest (`service.json`) needs to watch Junoswap pair contracts in addition to the agent-company contract:

```json
{
  "triggers": [
    {
      "type": "cosmos_contract_event",
      "chain": "uni-7",
      "contract": "juno1k8dxll...stj85k6",
      "events": ["wasm-outcome_create", "wasm-wavs_push", "wasm-sortition_request"]
    },
    {
      "type": "cosmos_contract_event",
      "chain": "uni-7",
      "contract": "JUNOSWAP_PAIR_ADDRESS",
      "events": ["wasm-swap", "wasm-provide_liquidity", "wasm-withdraw_liquidity"]
    }
  ]
}
```

---

## Files Created

| File | Purpose |
|------|---------|
| `contracts/junoswap-factory/` | Factory contract (6 tests passing) |
| `contracts/junoswap-pair/` | XYK AMM pair contract (7 tests passing) |
| `contracts/junoclaw-common/src/lib.rs` | Shared DEX types (AssetInfo, PairInfo, SwapEvent) |
| `wavs/src/trigger.rs` | Extended with SwapVerify, PoolHealthCheck, PriceAttestation |
| `wavs/src/lib.rs` | Extended with DEX processing functions |
| `docs/JUNOSWAP_INTEGRATION.md` | This document |
