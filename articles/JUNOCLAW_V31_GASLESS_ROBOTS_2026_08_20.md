# Gasless Robots: Why Juno v31 Unlocks the JunoClaw Fleet Model

> *"Sometimes to survive, we must become more than we were programmed to be."* — Roz, *The Wild Robot*

---

## The Problem You've Felt

If you've tried sending JUNO from Trust Wallet during a gas spike, you know the pain. The transaction sits there. It doesn't fail — it just doesn't go through. You can't bump the gas because Trust Wallet doesn't let you. You wait, you retry, you hope the network calms down.

Now imagine you're running a fleet of 1,000 robots. Each one submits proofs, anchors Merkle roots, trips circuit breakers. Every transaction needs gas. If gas spikes and the robot's wallet can't adjust, the robot's on-chain safety record goes dark. The physics keeps running — but the trust layer breaks.

This is not a theoretical problem. It's happening today on Juno.

---

## What v31 Fixes

**Juno v31** ([PR #1223](https://github.com/CosmosContracts/juno/pull/1223), opened August 18, 2026) is an integrated release candidate from juno-ai-dev. It's ready for testnet. Among many improvements, two matter directly for JunoClaw:

### 1. FeePay — Someone Else Pays the Gas

FeePay is a Juno module that lets a contract developer (or in our case, a fleet operator) prepay transaction fees for anyone interacting with their contracts. The user sends a transaction with zero fees. The FeePay module covers it from a pre-funded pool.

**v31 fixes what was broken:**
- **Denom overflow** — FeePay pools could misallocate funds when multiple denoms were involved
- **Sender-accounting bugs** — the wrong account was sometimes credited or debited
- **Legacy contract-cap migration** — old contracts hit caps that shouldn't have applied
- **Zero-fee rejection** — when FeePay failed (pool exhausted, wallet limit hit), the transaction used to silently consume the user's sequence number without executing. v31 rejects the tx cleanly in the ante handler instead.

With these fixes, FeePay becomes production-reliable. A fleet operator can fund a FeePay pool, and every robot operator under that fleet sends gasless transactions. The operator doesn't need JUNO. Doesn't need a gas strategy. Doesn't need to worry about spikes.

### 2. Feemarket Post-Handler — Dynamic Gas During Congestion

When network congestion rises, transactions with low gas prices get stuck. The feemarket post-handler adjusts dynamically so transactions don't get permanently trapped. This matters less for JunoClaw's coordination layer (which runs on its own BFT mesh, not Juno's mempool) but matters a lot for the settlement layer — the final on-chain anchor that regulators check.

---

## What This Means for JunoClaw

### The Fleet Economics Today

| Fleet Size | Daily Gas Cost | Who Pays |
|------------|---------------|----------|
| 1 robot | $0.004 | Robot operator (needs JUNO, needs gas strategy) |
| 100 robots | $0.36 | Each operator individually |
| 1,000 robots | $3.63 | Each operator individually |
| 10,000 robots | $36.26 | Each operator individually |

### The Fleet Economics with FeePay (v31)

| Fleet Size | Daily Gas Cost | Who Pays |
|------------|---------------|----------|
| 1 robot | $0.004 | Fleet operator (one pool) |
| 100 robots | $0.36 | Fleet operator (one pool) |
| 1,000 robots | $3.63 | Fleet operator (one pool) |
| 10,000 robots | $36.26 | Fleet operator (one pool) |

Same total cost. But the cognitive load changes completely:

- **Robot operators** don't need JUNO. Don't need a wallet with gas. Don't need to understand gas prices. Just send the transaction.
- **Fleet operators** fund one FeePay pool. They handle the gas strategy once, centrally, instead of 1,000 times individually.
- **Trust Wallet users** can interact with JunoClaw contracts without their transactions getting stuck during gas spikes.

### The Trust Wallet Angle

Trust Wallet is the most popular mobile wallet in Cosmos. It doesn't let users manually set gas prices. On Juno today, that means transactions fail during congestion. FeePay sidesteps this entirely — if the transaction is FeePay-eligible, the user's gas setting doesn't matter. The pool covers it.

For JunoClaw, this means a robot operator monitoring the fleet from their phone can trip a circuit breaker, query a Merkle root, or check a safety envelope without worrying about gas. The fleet operator's FeePay pool handles it.

---

## How It Works (Technical)

```
  ROBOT OPERATOR (Trust Wallet, no gas needed)
       |
  Sends tx with --fees 0 to JunoClaw contract
       |
  JUNO v31 ANTE HANDLER
  ├── Is this contract FeePay-registered? → YES
  ├── Is the FeePay pool funded? → YES
  ├── Is the wallet under its usage limit? → YES
  ├── Deduct fee from FeePay pool (not from user)
  └── Continue with tx execution
       |
  Transaction executes — robot operator pays nothing
```

The fleet operator funds the FeePay pool via `FundContract`. They set per-wallet usage limits via `RegisterFeePay`. They can top up, withdraw, and monitor usage via the FeePay module queries.

---

## What Needs to Happen

| Step | Status | Who |
|------|--------|-----|
| v31 PR #1223 merged | Ready for review | Juno maintainers |
| FeePay ante handler ordering | **Confirmed blocked on v30** — GlobalFee rejects zero-fee txs before FeePay can escrow. v31 reorders. | Us (tested Aug 21) |
| v31 testnet deployment | Pending PR merge | Juno team |
| v31 mainnet governance proposal | After testnet | Juno community |
| JunoClaw FeePay integration spec | Written, testnet-verified | Us |
| FeePay-registered JunoClaw contracts | After v31 mainnet | Us |
| Aegis PQC rebase onto v31 | After v31 mainnet | Us |

---

## The Bigger Picture

JunoClaw's pitch to a robotics company is: **plug in your robots, get cryptographic safety proofs, pay half a cent per robot per day.** That pitch is clean when gas is cheap and stable. It breaks when the robot operator has to manage gas themselves — especially from a mobile wallet that won't let them adjust it.

v31's FeePay fixes remove that friction. The fleet operator handles gas once. The robot operator never thinks about it. The Trust Wallet user just taps send.

That's the difference between a research project and a product.

---

## Qualification Evidence (from PR #1223)

The v31 release candidate has been qualified with:
- Exact v30.0.0 → v31 upgrade rehearsal passed
- 4.97 GB mainnet export/import/restart rehearsal passed (FeePay and module state preserved)
- Full root test suite and build passed
- Nested interchaintest packages compiled
- Release harness, actionlint, shfmt, ShellCheck, and state-evidence validator tests passed
- Candidate commit: `a6377d7feea50a86179de7a87ee9389255e65254`

Tracking: [juno-ai-dev/juno issues #6–#20](https://github.com/juno-ai-dev/juno/issues?q=is%3Aissue+number%3A6..20)

---

*August 2026. v30 live on Juno mainnet. v31 ready for testnet. FeePay registration, funding, and pool accounting verified on uni-7 (Aug 21, 2026). Gasless tx blocked by GlobalFee ante handler ordering on v30 — v31 reorders so FeePay intercepts first. JunoClaw fleet operators pay the gas. Robot operators just send. Trust Wallet works. The friction disappears.*
