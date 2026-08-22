# We Tested FeePay on Juno: Registration Works, Gasless Tx Blocked by Ante Handler Ordering

**Date:** August 21, 2026

---

We ran the full FeePay flow on Juno's uni-7 testnet against a live moultbook contract. Here's what happened.

## What We Did

1. **Instantiated a fresh moultbook contract** with the sender set as admin (the original had no admin — FeePay registration requires the caller to be the contract admin)
2. **Registered the contract with FeePay** via `MsgRegisterFeePayContract` with a wallet limit of 1,000 transactions per wallet
3. **Funded the FeePay pool** with 1,000,000 ujunox via `MsgFundFeePayContract`
4. **Queried the pool balance** via REST — confirmed 1,000,000 ujunox escrowed
5. **Sent a normal transaction** (with fees) to the moultbook contract — succeeded, 211,707 gas, 22,500 ujunox paid by sender
6. **Sent a gasless transaction** (fees=0) to the same contract — **failed**

## What Worked

Everything except the gasless transaction.

| Step | Result |
|------|--------|
| FeePay module enabled | Confirmed via REST query |
| Contract registration | Code 0, tx hash on-chain |
| Pool funding (1M ujunox) | Code 0, balance confirmed via query |
| Pool balance accounting | Correct — 1,000,000 ujunox |
| Wallet limit tracking | Working (set to 1,000) |
| Normal tx (with fees) | Succeeded, 211,707 gas |

FeePay's registration, funding, pool accounting, and wallet-limit tracking all work correctly on v30. The module is healthy. The plumbing is sound.

## What Didn't Work — and Why

The gasless transaction failed with:

```
insufficient fee: got: 0ujunox required: 22500ujunox
```

This is not a FeePay bug. FeePay did everything right. The problem is **ante handler ordering**.

On Juno v30, the transaction validation pipeline runs handlers in sequence:

```
1. GlobalFee ante handler  ← checks minimum fees
2. FeePay ante handler     ← would escrow from pool to cover fees
```

GlobalFee sees a zero-fee transaction, calculates `minGasPrice × gas = 22,500 ujunox`, and rejects it — **before FeePay ever gets a chance to escrow from the pool and cover the cost**.

FeePay is sitting there with 1,000,000 ujunox, ready to pay. It never gets the chance. GlobalFee says "insufficient fee" and the transaction is dead.

## The Fix

Juno v31 ([PR #1223](https://github.com/CosmosContracts/juno/pull/1223)) reorders the ante chain so FeePay intercepts **before** GlobalFee:

```
1. FeePay ante handler     ← escrows from pool, marks fees as covered
2. GlobalFee ante handler  ← sees fees are already paid, passes through
```

Same modules. Same logic. Different sequence. The fix is a reordering, not a rewrite.

When v31 lands on uni-7, we rerun the same script. Same contract, same pool, same zero-fee transaction. If the reorder works, the gasless tx succeeds, the pool is deducted, and the sender pays nothing.

## What This Means for Robot Fleets

JunoClaw's pitch is: plug in your robots, get cryptographic safety proofs, pay half a cent per robot per day. That pitch works when gas is cheap and stable. It breaks when a robot operator has to manage gas themselves — especially from a mobile wallet like Trust Wallet that doesn't let users adjust gas prices.

FeePay solves this. A fleet operator funds one pool. Robot operators send transactions with zero fees. The pool covers it. No JUNO needed, no gas strategy, no stuck transactions during spikes.

But only on v31. On v30, the pool is funded, the registration is done, the accounting works — and the gasless transaction still fails because of ordering.

We proved the plumbing works. v31 turns the valve.

## Test Artifacts

- Script: `deploy/test-feepay-testnet-v2.cjs`
- Contract: moultbook, code ID 80, uni-7 testnet
- Pool: 1,000,000 ujunox
- RPC: `https://juno.rpc.t.stavr.tech`
- REST: `https://juno-testnet-api.cogwheel.zone`
- FeePay integration spec: `docs/FEEPAY_INTEGRATION_SPEC.md`

---

*August 2026. FeePay tested on uni-7: registration, funding, pool accounting all verified on-chain. Gasless tx blocked by GlobalFee ante handler ordering — root cause confirmed, fix is a reordering in v31 PR #1223. Same script, same pool, same contract — rerun when v31 lands.*
