# The Truth Market Is Live: Four Epochs on Uni-7

> *"The proof of the pudding is in the eating."*

*The truth market had zero operators this morning. By tonight, it has three — staked, verified, and earning rewards on Juno testnet. Four epochs finalized. Rewards distributed. An operator slashed for dissenting. The closed loop works.*

---

## What Happened

On August 22, 2026, we took the truth market from theory to testnet reality. The contract — already deployed on uni-7 since August 17 — was migrated to the latest code (code_id 99) with protocol fee routing and fee-based slashing. Then we ran the full loop.

### The Setup

Three operators registered on the truth market contract (`juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p`):

| Operator | Address | Model | Hardware | Stake |
|----------|---------|-------|----------|-------|
| The Builder | `juno1aq995...` | rule-v1 | any | 1,000,000 ujunox |
| The Technocrat | `juno1pwutt...` | qwen-3b | jetson-orin | 1,000,000 ujunox |
| The Contrarian | `juno1n6h88...` | mistral-7b | cloud | 1,000,000 ujunox |

Each staked 1 JUNOX (1,000,000 ujunox) — the minimum stake. Each submitted a unique fingerprint identifying their model and hardware for diversity detection.

The reward pool was seeded with 500,000 ujunox. The verification fee was set to 50,000 ujunox per batch.

### Epoch 1: Consensus

All three operators submitted "green" verdicts on batch #1. The relayer paid a 50,000 ujunox verification fee on behalf of `rosie-unit-001`. The epoch was finalized with consensus = "green".

**Result:** 3/3 matched. No slashes. 27,498 ujunox distributed equally (9,166 each). All operators at 100% accuracy.

```
Reward pool: 500,000 + 50,000 (fee) - 27,498 (rewards) = 522,502 ujunox
```

### Epoch 2: The Slash (Old Code — 10% of Stake)

The Contrarian went rogue — submitted "red" while the other two said "green". The relayer paid 50,000 ujunox for `rosie-unit-002`. Consensus was "green". The Contrarian diverged.

**Result (old slashing logic):** 10% of stake slashed = 100,000 ujunox. The Contrarian's stake dropped from 1,000,000 to 900,000. Accuracy dropped from 100% to 50%. The slashed 100,000 went back to the reward pool.

```
Reward pool: 522,502 + 50,000 (fee) + 100,000 (slash) - 28,624 (rewards) = 643,878 ujunox
```

The pool *grew* by 121,376 ujunox this epoch. The slash + fee exceeded rewards distributed.

### The Slashing Fix

10% of stake per wrong verdict is aggressive. If an operator gets 5 wrong, they've lost ~40% of their stake (compounding). We changed the slashing logic:

**When `verification_fee` is set (> 0): slash = verification_fee (capped at remaining stake)**
**When `verification_fee` is 0 (open access): slash = slash_percent of stake (fallback)**

This creates perfect economic symmetry:
- A robot pays 50,000 ujunox for verification
- A miner who gets it wrong loses exactly 50,000 ujunox
- The penalty aligns with the fee that funds the system

The contract was migrated to code_id 99 with the fix. 22/22 unit tests pass, including a new `test_slash_equals_verification_fee` test.

### Epoch 3: Still Old Code (Migration Reset Fee)

The migration preserved the existing config, but the verification_fee had been reset to 0 by the previous migration (before the fix). So epoch 3 still used the 10% fallback. The Contrarian diverged again — slashed 90,000 ujunox (10% of 900,000).

### Epoch 4: Fee-Based Slashing (New Code)

After setting `verification_fee` back to 50,000, we ran the real test. The Contrarian submitted "red" again. Consensus was "green".

**Result (new slashing logic):** Slash = 50,000 ujunox (the verification fee), NOT 81,000 (10% of 810,000 stake).

```
Slashed: 50,000 ujunox (the fee, not 10%)
Stake: 810,000 → 760,000
Accuracy: 33% → 25%
```

The slash is now predictable, doesn't compound aggressively, and equals exactly what a robot pays for verification.

---

## The Numbers After 4 Epochs

| Operator | Stake | Rewards | Slashed | Correct | Wrong | Accuracy |
|----------|-------|---------|---------|---------|-------|----------|
| The Builder | 1,000,000 | 60,803 | 0 | 4 | 0 | 100% |
| The Technocrat | 1,000,000 | 60,803 | 0 | 4 | 0 | 100% |
| The Contrarian | 760,000 | 9,166 | 240,000 | 1 | 3 | 25% |

**Contract stats:**
- Total operators: 3
- Total staked: 2,760,000 ujunox
- Reward pool: 809,228 ujunox
- Total rewards paid: 95,970 ujunox
- Total slashed: 240,000 ujunox
- Epochs finalized: 4

The reward pool *grew* from 500,000 to 809,228 across 4 epochs — a net increase of 309,228 ujunox. The system is self-sustaining: fees + slashes exceed rewards distributed.

---

## The Closed Loop — Proven

Here's the full cycle, verified on-chain:

1. **Robot pays fee** → `PayVerificationFee` sends 50,000 ujunox to reward pool
2. **Miners evaluate** → each operator runs their model, submits a verdict
3. **Relayer finalizes** → `FinalizeEpoch` compares verdicts against consensus
4. **Correct miners earn** → reward pool distributes to matching operators
5. **Wrong miners get slashed** → slash = verification fee, goes back to pool
6. **Pool refills** → next batch's fee + any slashes feed the pool

No inflation. No token minting. No grants needed. JUNO circulates between robots that need verification and miners that provide it. The pool grows when miners get it wrong. The pool shrinks when miners get it right. Natural equilibrium.

---

## What's Built

| Component | Status | Details |
|-----------|--------|---------|
| Truth market contract | ✅ Live on uni-7 | code_id 99, 22/22 tests |
| `PayVerificationFee` | ✅ Live | 50K ujunox enforced per batch |
| Fee-based slashing | ✅ Live | slash = verification_fee, capped at stake |
| `junoclaw-miner` CLI | ✅ Built | register, run, unstake, withdraw, deposit |
| On-chain registration | ✅ Wired | cosmos-mcp CLI subprocess |
| Relayer fee routing | ✅ Wired | `--verification-fee` flag, pays before finalize |
| Frontend | ✅ Updated | shows verification fee in MinerPanel |
| 3 operators | ✅ Registered | diverse fingerprints, 1M ujunox each |
| 4 epochs finalized | ✅ Verified | rewards + slashing on-chain |

## What's Next

- **FeePay v31** — gasless `PayVerificationFee` for robot operators (ante handler reorder)
- **McapEvaluator** — verdict from telemetry data, no LLM needed
- **Fingerprint diversity enforcement** — relayer rejects correlated miners
- **Cross-chain IBC verdicts** — miners on Juno evaluate robots on other chains
- **Mainnet deployment** — once FeePay v31 is live

---

## The Bigger Picture

Four epochs. Three operators. One slashed. The truth market is no longer theoretical — it's a working economic loop on Juno testnet.

The beauty of fee-based slashing is its simplicity: **the penalty for being wrong equals the cost of being verified**. A miner who submits a wrong verdict loses exactly what a robot paid for the verification. No complex formulas. No percentage calculations. Just: get it wrong, pay the fee.

The reward pool grew 60% across 4 epochs without any external funding. Fees from robots + slashes from wrong miners > rewards to right miners. The system funds itself.

**Rosie mines truth after housekeeping. And tonight, she earned 9,166 ujunox for getting it right.**

---

*August 22, 2026. Truth market contract live on uni-7 (code_id 99, `juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p`). 3 operators registered, 4 epochs finalized, 95,970 ujunox in rewards distributed, 240,000 ujunox slashed. Fee-based slashing: slash = verification_fee = 50,000 ujunox. Reward pool: 809,228 ujunox and growing. 22/22 contract tests, 14/14 relayer tests. The closed loop works.*

---

*Related: [Rosie Mines Truth](ROSIE_MINES_TRUTH_2026_08_22.md) · [Full Stack Product Picture](JUNOCLAW_FULL_STACK_MELANGE_2026_08_19.md) · [What Roz Taught Us](JUNOCLAW_WILD_ROBOT_TRUST_2026_08_20.md) · [Gasless Robots](JUNOCLAW_V31_GASLESS_ROBOTS_2026_08_20.md) · [FeePay Tested on uni-7](FEEPAY_TESTED_ON_UNI7_2026_08_21.md)*
