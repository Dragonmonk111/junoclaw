# JunoClaw FeePay Integration Spec

## Overview

Juno's `x/feepay` module lets a contract sponsor transaction fees for its users. The user sends a tx with zero fees; the FeePay pool covers it. This spec defines how JunoClaw registers its coordination contracts with FeePay so that robot operators can submit transactions without holding JUNO or managing gas.

**Dependency:** Juno v31 ([PR #1223](https://github.com/CosmosContracts/juno/pull/1223)) — fixes denom overflow, sender-accounting bugs, zero-fee rejection, and legacy contract-cap migration.

---

## Contracts to Register with FeePay

Not every JunoClaw contract needs FeePay. Only contracts that robot operators interact with directly (not admin/governance calls):

| Contract | ExecuteMsg | Who Calls | FeePay? | Rationale |
|----------|-----------|-----------|---------|-----------|
| merkle-verifier | `AnchorRoot` | Relayer / prover daemon | **Yes** | Robot-side submission, high frequency |
| merkle-verifier | `VerifyProof` | Anyone (read query via tx) | **Yes** | Robot operator spot-checks |
| circuit-breaker | `TripBreaker` | Coordination settler / governance | **No** | Governance-only, not robot operator |
| circuit-breaker | `ResetBreaker` | Admin / fleet operator | **Yes** | Fleet operator may need to reset from mobile |
| safety-envelope | `SetEnvelope` | Admin only | **No** | Governance-only |
| safety-envelope | `TightenEnvelope` | Admin only | **No** | Governance-only |
| coordination-settler | `SubmitBatch` | Registered relayer | **No** | Relayer is infra, not a robot operator |
| zk-verifier | `VerifyProof` | Anyone | **Yes** | Robot operator verification checks |
| moultbook | `Post` | Robot operators / agents | **Yes** | Provenance entries from the field |
| moultbook | `PublishAnon` | Robot operators / agents | **Yes** | Anonymous disclosure from the field |

**Summary:** 5 contracts / 6 message types registered for FeePay. Admin and relayer paths excluded.

---

## FeePay Registration Flow

### Step 1: Register Contracts (post-v31 mainnet upgrade)

Fleet operator registers each contract address with the FeePay module:

```bash
# Register merkle-verifier for FeePay
junod tx feepay register-feepay \
  --contract juno1...merkle-verifier \
  --from fleet-operator \
  --gas auto --fees 250ujuno

# Register zk-verifier
junod tx feepay register-feepay \
  --contract juno1...zk-verifier \
  --from fleet-operator \
  --gas auto --fees 250ujuno

# Register circuit-breaker (for ResetBreaker only)
junod tx feepay register-feepay \
  --contract juno1...circuit-breaker \
  --from fleet-operator \
  --gas auto --fees 250ujuno

# Register moultbook
junod tx feepay register-feepay \
  --contract juno1...moultbook \
  --from fleet-operator \
  --gas auto --fees 250ujuno
```

### Step 2: Fund the FeePay Pool

```bash
# Fund with 10,000 ujuno (covers ~2.7M txs at ~3.7K gas each)
junod tx feepay fund-contract \
  juno1...merkle-verifier \
  10000000000ujuno \
  --from fleet-operator \
  --gas auto --fees 250ujuno
```

**Recommended pool sizes per fleet scale:**

| Fleet Size | Txs/Day | Daily Gas Cost | Recommended Pool | Top-up Cadence |
|------------|---------|---------------|-----------------|----------------|
| 1 robot | ~10 | $0.004 | 100 ujuno | Monthly |
| 100 robots | ~1,000 | $0.36 | 10,000 ujuno | Monthly |
| 1,000 robots | ~10,000 | $3.63 | 100,000 ujuno | Monthly |
| 10,000 robots | ~100,000 | $36.26 | 1,000,000 ujuno | Bi-weekly |

### Step 3: Set Per-Wallet Usage Limits (optional but recommended)

Prevents a single compromised or malfunctioning robot from draining the pool:

```bash
# Limit each wallet to 100 txs per epoch (default epoch = 1 block on some configs)
junod tx feepay register-feepay-wallet-limit \
  --contract juno1...merkle-verifier \
  --wallet juno1...robot-operator-1 \
  --limit 100 \
  --from fleet-operator
```

For fleet-wide defaults, register each robot operator wallet after onboarding via the fleet coordinator's `RegisterRobot` endpoint.

---

## Robot Operator Experience (After FeePay)

### Before FeePay (today)

```
1. Robot operator opens Trust Wallet
2. Needs JUNO balance for gas
3. Sends AnchorRoot tx with --fees 250ujuno
4. If gas prices spike → tx stuck, can't adjust in Trust Wallet
5. Merkle root not anchored on-chain → safety record gap
```

### After FeePay (v31)

```
1. Robot operator opens Trust Wallet
2. No JUNO needed
3. Sends AnchorRoot tx with --fees 0
4. FeePay ante handler checks: contract registered? pool funded? wallet under limit?
5. Fee deducted from fleet operator's pool
6. Tx executes — Merkle root anchored
7. Robot operator never thinks about gas
```

---

## Pool Monitoring & Alerts

The fleet coordinator already tracks fleet status via REST API. FeePay pool monitoring extends this:

### New REST Endpoints (proposed for fleet coordinator)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/feepay/{contract}` | GET | Pool balance, registered wallets, usage this epoch |
| `/feepay/{contract}/history` | GET | Pool funding/withdrawal history |
| `/feepay/alerts` | GET | Low-balance alerts (pool < 20% of recommended) |

### Alert Thresholds

| Level | Trigger | Action |
|-------|---------|--------|
| Green | Pool > 50% of recommended | None |
| Yellow | Pool 20-50% | Notify fleet operator via dashboard |
| Red | Pool < 20% | Alert + auto-pause new robot onboarding until refilled |
| Critical | Pool exhausted | Robot operators must pay own gas until refill (graceful degradation) |

### Graceful Degradation

If the FeePay pool is exhausted, the v31 ante handler cleanly rejects zero-fee txs (no sequence number consumption — this is the v31 fix). Robot operators with JUNO can still submit normally. The trust layer degrades to "operators pay their own gas" — not "safety records go dark."

---

## Security Considerations

### Pool Drain Attack

A malicious robot operator could submit many trivial transactions to drain the FeePay pool. Mitigations:

1. **Per-wallet usage limits** — cap each wallet at N txs per epoch
2. **Fleet coordinator rate limiting** — already implemented (10 intents/sec/robot)
3. **Pool monitoring alerts** — Yellow/Red thresholds trigger before exhaustion
4. **Contract-level gas caps** — JunoClaw contracts have fixed gas costs (~80-120K per execute), so drain rate is bounded

### Stale Pool After Contract Upgrade

If a JunoClaw contract is migrated (e.g., zk-verifier precompile variant), the FeePay registration must be re-pointed to the new contract address. The fleet operator should:

1. Deploy new contract
2. Register new contract with FeePay
3. Fund new pool
4. Withdraw remaining funds from old pool
5. Update relayer/prover daemon config to new contract address

---

## Integration Checklist

- [ ] v31 mainnet upgrade passes governance
- [ ] FeePay module enabled on mainnet (verify via `junod query feepay params`)
- [x] FeePay module enabled on testnet (uni-7) — confirmed Aug 21, 2026
- [x] Register contract with FeePay on testnet — `MsgRegisterFeePayContract` succeeded, tx A3287D06...
- [x] Fund FeePay pool on testnet — `MsgFundFeePayContract` succeeded, 1M ujunox, tx 932518DC...
- [x] Query pool balance on testnet — confirmed via REST, `fee_pay_contract.balance = 1000000`
- [x] Normal tx (with fees) to registered contract — succeeded, 211,707 gas, tx DBD4974D...
- [x] Gasless tx (fees=0) on v30 — **failed**, GlobalFee blocks before FeePay (confirmed root cause)
- [ ] Test gasless tx flow on v31 testnet (after v31 lands on uni-7)
- [ ] Register merkle-verifier, zk-verifier, circuit-breaker, moultbook with FeePay on mainnet
- [ ] Fund each pool per fleet scale table
- [ ] Set per-wallet limits for each registered robot operator
- [ ] Add FeePay monitoring endpoints to fleet coordinator
- [ ] Update fleet dashboard to show pool balances
- [ ] Update SDK integration guide with FeePay section
- [ ] Document pool top-up procedure for fleet operators

---

## Open Questions

1. **FeePay + GlobalFee interaction** — **ANSWERED (Aug 21, 2026).** Tested on uni-7 v30: GlobalFee ante handler runs BEFORE FeePay ante handler, rejecting zero-fee txs (`insufficient fee: got 0ujunox required 22500ujunox`) before FeePay can escrow from the pool. FeePay registration, funding, pool accounting, and wallet-limit tracking all work correctly on v30 — only the gasless tx flow is blocked by ante handler ordering. v31 PR #1223 reorders the ante chain so FeePay intercepts first. Same modules, same logic, different sequence.

2. **FeePay + feemarket post-handler** — if feemarket adjusts gas prices post-ante, does FeePay cover the adjusted amount or the original? Need to confirm against v31 implementation.

3. **Multi-denom pools** — can a fleet operator fund a FeePay pool with multiple denoms (e.g., ujuno + stablecoin)? v31 fixes denom overflow bugs, but the supported denom set needs verification.

4. **IBC FeePay** — if a robot operator submits via ICA from another chain, does FeePay still cover the gas? Likely no (FeePay is local ante handler), but worth confirming for cross-chain robot fleets.

---

*Spec version: 2026-08-21. Blocked on v31 mainnet. FeePay registration, funding, and pool accounting verified on uni-7 v30 testnet. Gasless tx flow confirmed blocked by GlobalFee ante handler ordering — fixed in v31 PR #1223. All JunoClaw contracts are deployed and tested — only the FeePay gasless tx flow awaits v31.*
