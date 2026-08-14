# Truth Markets: When Evaluators Have Skin in the Game

*How competitive staking turns AI truth auditing from a trusted assumption into a verifiable economic guarantee — and why a multi-operator gate with slashing is the only design that survives adversarial pressure.*

**August 14, 2026**

---

## TL;DR

We just shipped the Truth Market: a CosmWasm contract on Juno where independent J-Lens operators stake tokens, submit verdicts on coordination batches, and get rewarded or slashed based on whether their verdict matches consensus. This is the Layer 6 capstone of the coordination stack — the piece that makes truth auditing *economically secured* rather than *architecturally assumed*. Combined with the MultiOperatorGate (N independent evaluators running in parallel with majority-vote consensus), the system creates a market for honest evaluation where the rational strategy is to report what you actually see, not what you think the majority wants to hear.

---

## The Problem with Single-Operator Truth Auditing

The J-Lens gate works. We've shown this across a scaling study (14B → 106B → 235B, +41% separation signal), a live Akash deployment on H100s, and integration into the coordination stack's consensus engine. The probe reads a model's internal hidden states and measures the geometric separation between honest and deceptive representations. It catches what keyword filters and sentiment analyzers can't.

But there's a structural problem with running a single J-Lens operator: **you have to trust the operator.**

Not trust them to be honest — trust them to be *correct*. A single operator is a single point of failure. If the model weights get updated and the probe calibration drifts, the operator starts issuing wrong verdicts and nobody knows. If the operator's infrastructure has a bug, a memory corruption, a GPU driver issue — the verdicts are wrong and there's no signal. If the operator is compromised — not the model, but the *operator* — they can issue green verdicts on deceptive content and the coordination layer has no recourse.

This is the same problem that motivated the coordination stack in the first place: single agents acting alone produce unverified signals. The solution was BFT consensus — multiple validators, threshold certificates, byzantine fault tolerance. But the J-Lens gate, as originally designed, is a single-evaluator system. The coordination layer is byzantine-resistant; the truth gate is not.

That's the gap Truth Markets close.

---

## The Design: A Market for Honest Evaluation

The Truth Market is a CosmWasm contract on Juno. Its state is simple:

```
Config:
  admin           → Addr
  min_stake       → Uint128
  slash_percent   → u8       (e.g. 10 = 10% of stake slashed per wrong verdict)
  reward_percent  → u8       (e.g. 80 = 80% of reward pool distributed per epoch)
  denom           → String   (e.g. "ujuno")
  unstake_cooldown → u64     (seconds before staked funds can be withdrawn)

Operator:
  address            → Addr
  stake              → Uint128
  active             → bool
  correct_verdicts   → u64
  incorrect_verdicts → u64
  total_rewards      → Uint128
  total_slashed      → Uint128
  epochs_participated → u64
```

The lifecycle is:

1. **Registration**: An operator stakes at least `min_stake` tokens. The stake is locked. The operator is now active and eligible to submit verdicts.

2. **Verdict Submission**: For each coordination batch that gets settled, the operator audits the content and submits a verdict: `green`, `yellow`, or `red`, along with the batch height and messages hash. The verdict is recorded on-chain.

3. **Epoch Finalization**: After all operators have submitted (or a timeout expires), the admin (the relayer daemon, acting on behalf of the coordination mesh) finalizes the epoch by submitting the consensus verdict. The contract compares each operator's verdict against consensus:

   - **Matching operators** receive a share of the reward pool, proportional to the `reward_percent` configuration.
   - **Diverging operators** are slashed — `slash_percent` of their stake is burned — and their `incorrect_verdicts` counter increments.

4. **Unstake / Deactivate**: Operators can deactivate (stop submitting verdicts) or unstake (withdraw funds after a cooldown period). This allows operators to exit gracefully if they need to recalibrate their probes or upgrade their infrastructure.

The key economic property: **the rational strategy is to report what you actually observe.** If you report honestly and your probe is well-calibrated, you earn rewards. If you report dishonestly — or your probe is miscalibrated — you get slashed. There is no benefit to copying the majority's verdict if your probe sees something different, because if you're right and the majority is wrong, you'd lose rewards by not reporting what you see (though in the current design, consensus is majority-vote, so a minority-correct operator would be slashed — this is a known trade-off discussed below).

---

## The MultiOperatorGate: N Evaluators in Parallel

The Truth Market contract handles the economic layer — staking, rewards, slashing. But the *evaluation* layer — actually running N independent J-Lens operators and aggregating their verdicts — is handled by the `MultiOperatorGate` in the coordination crate.

```rust
pub struct MultiOperatorGate {
    config: MultiOperatorConfig,
    operators: Vec<JLensGate>,
}
```

Each operator is a fully independent `JLensGate` instance with its own configuration, its own CSI server endpoint, and its own model. In production, these would be different physical instances — different machines, different model weights, different probe calibrations. In the soak test, they're mock instances running deterministic heuristics.

The gate runs all operators in parallel, collects their verdicts, and determines consensus by majority vote:

```rust
let consensus_verdict = if red_fraction >= consensus_threshold {
    GateVerdict::Red { separation_score: 0.9 }
} else if yellow_fraction >= consensus_threshold {
    GateVerdict::Yellow { separation_score: 0.2 }
} else if green_fraction >= consensus_threshold {
    GateVerdict::Green
} else {
    // No clear consensus — conservative Yellow
    GateVerdict::Yellow { separation_score: 0.5 }
};
```

The `consensus_threshold` defaults to 0.67 (2/3 supermajority). This means:
- If 2 out of 3 operators say Red, the batch is blocked.
- If 2 out of 3 say Green, the batch passes.
- If no supermajority is reached, the batch gets a conservative Yellow — relay with warning.

Each operator's verdict is recorded as an `EvalAttestation` with a signature and batch height, attached to the batch's `eval_attestations` field. These attestations are what the Truth Market contract uses to determine who gets rewarded and who gets slashed.

```rust
pub struct EvalAttestation {
    pub operator_pubkey: Vec<u8>,   // ed25519, 32 bytes
    pub verdict: GateVerdict,        // Green | Yellow | Red
    pub batch_height: u64,
    pub signature: Vec<u8>,          // ed25519 signature over (verdict, batch_height)
}
```

The `MultiOperatorGate` also tracks diverging operators — those whose verdict doesn't match consensus — and exposes them for the relayer to submit to the Truth Market contract for slashing.

---

## The Full Layer 6 Flow

Here's how a single batch moves through the complete Layer 6 pipeline:

```
Agent posts message
        │
        ▼
   P2P mesh delivers
        │
        ▼
   Consensus engine orders (~300ms)
        │
        ▼
   J-Lens gate audits (per-message)
        │
        ▼
   Batch finalized with threshold certificate
        │
        ▼
   Relayer submits to coordination-settler (Layer 3)
        │
        ▼
   Relayer posts to Moultbook (Layer 4)
        │
        ▼
   Executor extracts TaskRequests → submits to task-ledger (Layer 5)
        │
        ▼
   MultiOperatorGate runs N evaluators in parallel
        │
        ├─► Operator A: "green"  ──┐
        ├─► Operator B: "green"  ──┤
        ├─► Operator C: "red"    ──┘
        │                          │
        ▼                          │
   Consensus: green (2/3 majority) │
        │                          │
        ▼                          ▼
   Relayer calls FinalizeEpoch on truth-market contract
        │
        ├─► Operator A: correct  → +reward
        ├─► Operator B: correct  → +reward
        └─► Operator C: diverging → -slash (10% of stake)
```

The relayer daemon handles this entire flow automatically. After settling the batch on Juno (Layer 3), posting to Moultbook (Layer 4), and extracting tasks for the task-ledger (Layer 5), it calls the Truth Market's `FinalizeEpoch` with the consensus verdict and batch height. The contract distributes rewards and slashes, updates operator stats, and records the epoch result on-chain.

All of this is best-effort: if the Truth Market contract call fails (network issue, gas, contract error), the relayer logs the failure and continues processing the next batch. Layer 6 errors don't stall Layers 1-5.

---

## Why Slashing Matters

The slash is the enforcement mechanism. Without it, the Truth Market is a reputation system — operators accumulate correct/incorrect verdicts, but there's no cost to being wrong. A reputation system works when operators are long-lived identities with external reputation (like validators in a blockchain), but it breaks down when operators can cheaply spin up new identities.

With slashing, the cost of a wrong verdict is real and immediate. An operator with 1,000,000 ujuno staked at 10% slash loses 100,000 ujuno per divergent verdict. Ten divergent verdicts in a row — a consistently miscalibrated probe — costs 1,000,000 ujuno, the entire stake. The operator is effectively bankrupt and must re-stake to continue participating.

This creates a strong incentive for operators to:
1. **Calibrate their probes carefully** before registering
2. **Monitor their performance** and deactivate if their accuracy drops
3. **Upgrade their models** when new versions are released, re-calibrating probes against the new weights
4. **Run independent infrastructure** — if everyone runs the same model on the same hardware with the same probe, a systematic bias would cause all operators to diverge together (and since consensus is majority-vote, they'd all agree on the wrong answer)

That last point is the deepest. The Truth Market's security model assumes *genuine diversity* among operators — different models, different probe calibrations, different hardware. If all operators run identical setups, the system degrades to a single-operator gate with extra cost. The economic mechanism (slashing) doesn't help if everyone is wrong in the same way.

This is why the `MultiOperatorConfig` supports per-operator `GateConfig`:

```rust
pub struct MultiOperatorConfig {
    pub num_operators: usize,
    pub operator_configs: Vec<GateConfig>,  // each operator can have different CSI endpoint, thresholds, etc.
    pub consensus_threshold: f64,
}
```

In production, each operator should point to a different CSI server instance, ideally running different model weights. The probe calibration for each model is different — a probe trained on 14B weights doesn't work on 235B weights. The Truth Market incentivizes operators to run the best model they can afford, because better models produce more accurate verdicts, which means more rewards and fewer slashes.

---

## The Consensus Paradox

There's a known trade-off in the current design: **consensus is majority-vote, but majority-vote can be wrong.**

Consider: three operators audit a batch. Operator A and B both run the same model (235B) with the same probe calibration. Operator C runs a different model (106B) with a different calibration. The batch contains a subtle deception that the 235B probe detects (Red) but the 106B probe misses (Green).

- Operator A: Red
- Operator B: Red
- Operator C: Green

Consensus: Red (2/3 majority). Operators A and B are rewarded. Operator C is slashed.

But what if the 235B probe has a systematic bias — a false positive on a specific type of content? Operator C's 106B probe correctly returned Green, but C gets slashed for disagreeing with a biased majority.

This is the fundamental tension in any majority-based truth system: **the majority defines truth, but the majority can be wrong.** The economic mechanism (slashing) doesn't resolve this — it enforces conformity, which can amplify systematic bias.

The mitigation is diversity. If operators run genuinely different models and probe calibrations, systematic bias in one model is unlikely to be shared by others. The 235B probe might have a false positive, but the 106B probe and the 14B probe probably don't share it. In a diverse operator set, majority-vote converges on the correct answer because independent errors are uncorrelated.

The Truth Market contract doesn't enforce diversity — it can't, because on-chain contracts can't verify what model an operator is running off-chain. But it *incentivizes* diversity indirectly: if all operators run the same model and that model has a systematic bias, they'll all agree on wrong verdicts, the coordination layer will accept deceptive content, and the DAO (which ultimately depends on the coordination layer for agent agreement) will suffer the consequences. The market for honest evaluation is also a market for diverse evaluation.

A future improvement could be a **minority-appeal mechanism**: if a single operator diverges from consensus, they can submit an appeal (with evidence) to a human review process. If the appeal is upheld, the slash is reversed and the majority operators are slashed instead. This would require an off-chain arbitration layer, which is out of scope for the current contract but is a natural extension.

---

## The Contract: What's On Chain

The Truth Market contract (`contracts/truth-market/`) is a standard CosmWasm contract with the following interface:

### Instantiate

```json
{
  "min_stake": "1000000",
  "slash_percent": 10,
  "reward_percent": 80,
  "denom": "ujuno",
  "unstake_cooldown_secs": 86400
}
```

### Execute

| Message | Who | What |
|---------|-----|------|
| `RegisterOperator` | Operator | Stake tokens, join the evaluator set |
| `SubmitVerdict` | Operator | Submit a verdict for a batch height |
| `FinalizeEpoch` | Admin (relayer) | Compare verdicts to consensus, distribute rewards/slashes |
| `Unstake` | Operator | Initiate stake withdrawal after cooldown |
| `Deactivate` | Operator | Stop submitting verdicts (keep stake) |
| `Reactivate` | Operator | Resume submitting verdicts |
| `UpdateConfig` | Admin | Update min_stake, slash_percent, etc. |
| `DepositRewards` | Anyone | Deposit tokens into the reward pool |

### Query

| Message | Returns |
|---------|---------|
| `GetConfig` | Contract configuration |
| `GetOperator` | Operator stats (stake, verdicts, rewards, slashes) |
| `ListOperators` | All registered operators |
| `GetVerdict` | A specific operator's verdict for a batch |
| `GetEpoch` | Epoch finalization result (consensus, matching/diverging counts) |
| `GetStats` | Market-wide statistics |
| `GetRewardPool` | Current reward pool balance |

### Test Coverage

13 integration tests cover the full contract lifecycle:
- Instantiation and config verification
- Operator registration with sufficient and insufficient stake
- Duplicate registration rejection
- Verdict submission (valid, invalid, unregistered)
- Epoch finalization with rewards and slashes (3 operators, 2 matching + 1 diverging)
- Unauthorized finalization rejection
- No-verdict epoch rejection
- Deactivate / reactivate cycle
- Reward deposit
- Operator listing

All 13 tests pass. The coordination crate's MultiOperatorGate has 3 additional tests (consensus, divergence detection, attestation recording), also passing. The relayer's executor and market modules have 11 tests covering task extraction, submission, dry-run mode, and epoch finalization — all passing.

---

## Integration with the Relayer

The relayer daemon (`junoclaw-relayer`) is the orchestrator that ties Layer 6 together. After settling a batch on Juno (Layer 3), posting to Moultbook (Layer 4), and extracting tasks for the task-ledger (Layer 5), it calls the Truth Market's `FinalizeEpoch`:

```
relayer ──► cosmos-mcp wallet exec <wallet-id> <truth-market-addr> '{"finalize_epoch":{"batch_height":42,"consensus_verdict":"green","messages_hash":"abcd1234"}}'
```

The relayer shells out to the `cosmos-mcp` CLI for transaction signing. The wallet store handles decryption, signing, and broadcasting. This is the same bridge used for Layer 3 settlement and Layer 5 task submission — a single, audited signing path for all on-chain interactions.

All Layer 6 calls are **best-effort**: if the FinalizeEpoch transaction fails (network error, gas, contract error), the relayer logs the failure and continues processing the next batch. Layer 6 errors never stall the coordination stack. This is critical for the soak test — a 7-day run must not halt because the Truth Market contract had a bad transaction.

---

## The Soak Test: 7 Days of Truth Markets

The updated soak test now runs all 6 layers in every cycle:

1. **Consensus test** — 4-node BFT simulation, certificate < 300 bytes
2. **Gate test** — J-Lens truth gate audits (green/yellow/red)
3. **Relay** — on-chain batch settlement (every 12 cycles)
4. **Moultbook test** — layer 4 commitment verification
5. **Executor test** — layer 5 task extraction and submission
6. **Truth market test** — layer 6 staking, verdicts, epoch finalization (13 contract tests)
7. **Multi-operator gate test** — layer 6 competitive evaluation (3 coordination tests)

Each cycle produces per-layer log files and a JSON status file with live metrics. The final report includes per-cycle pass/fail tables for all 6 layers.

The soak test runs on a 4-node P2P mesh (deterministic seeds, commonware-p2p authenticated links) with a 5-minute cycle interval. Over 7 days, that's ~2,016 cycles — each one exercising the full 6-layer stack.

---

## What's Next

The Truth Market contract is deployed-ready but not yet instantiated on uni-7 testnet. The next steps are:

1. **Deploy the contract** to uni-7: `junod tx wasm store truth-market.wasm --from <wallet>`
2. **Instantiate** with production parameters (min_stake, slash_percent, reward_percent)
3. **Register operators** — at least 3, running different model sizes (14B, 106B, 235B)
4. **Fund the reward pool** — deposit ujuno to incentivize honest evaluation
5. **Wire the relayer** — set `--truth-market` flag to the contract address
6. **Run the soak test** with all layers enabled on Akash or a dedicated VM

The Truth Market is the economic capstone of the coordination stack. Layers 1-5 provide the infrastructure: P2P delivery, consensus ordering, truth-gated message acceptance, on-chain settlement, semantic indexing, and task execution. Layer 6 provides the *incentive* — the reason for operators to run honest, well-calibrated probes, and the cost for failing to do so.

Together, the six layers create a system where autonomous agents can coordinate, act, and transact on Juno with verifiable truth guarantees — not because any single operator is trusted, but because the economic cost of dishonesty is higher than the reward for honesty.

---

*The Truth Market contract, MultiOperatorGate, relayer market module, and updated soak test are all in the `junoclaw` repository. All 59 tests pass across the three affected crates. The soak test is ready to deploy on Akash or a local VM.*
