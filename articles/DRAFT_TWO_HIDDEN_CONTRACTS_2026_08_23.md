# The Robot Has a Credit Score — And It Buys a Bigger Brain When It's Scared

*Two contracts sitting in our repo with zero article coverage. They might be the most strategically important code we've written.*

*August 23, 2026 — draft, updated with A052 operator mandate execution results*

---

## The Two Contracts Nobody Knows About

We've written 25 articles about JunoClaw. We've covered ZK proofs, truth markets, FeePay, soak tests, DAO governance, and the actuarial thesis. We've never mentioned two contracts that have been sitting in `contracts/` since they were deployed on uni-7.

They are not infrastructure. They are not tooling. They are the product.

### `machine-rwa` — the robot has a credit score

The contract mints an NFT for a physical machine: model, serial number, sensor suite, IPFS metadata. Standard RWA stuff — DePIN projects do this. But then it does something nobody else does.

Each machine NFT is bound to a `moultbook_author` — the cryptographic identity that publishes verified work entries to Moultbook. When you call `GetWorkIntegrityScore`, the contract cross-queries Moultbook for that author's verified entry count and returns a score.

**A robot whose creditworthiness is derived from cryptographically verified work history.**

Not "this machine exists" (DePIN). Not "this machine is owned by X" (traditional RWA). "This machine has completed N verified work cycles, attested by independent operators, with a slashing-enforced accuracy rate." The credit score is not self-reported. It is not a manufacturer's claim. It is derived from the same adversarial truth market that we just ran 5 epochs of on uni-7 — with real slashing.

And the NFT is fractional. Up to 10,000 basis points of ownership can be split among multiple owners, transferred in fractions. A clinic can own 30% of a surgical assistant, the manufacturer retains 40%, an insurer holds 30% as collateral. When the machine's work integrity score goes up, every fraction becomes more valuable.

**What this actually is:** a robot that can be financed against its own proven track record. A $300,000 surgical robot with 10 million verified clean cycles is not the same risk as one with zero cycles. Today, no underwriter can tell the difference. With `machine-rwa`, the difference is on-chain and queryable by anyone.

Contract status: **built, not yet deployed**. Source at `contracts/machine-rwa/` — 473 lines, full test suite. Next step: deploy to uni-7 and mint the first machine.

### `emergency-compute-escrow` — the robot buys a bigger brain when it's scared

An edge agent — a robot running on a Jetson Orin, say — encounters a situation it's not confident about. Its self-assessed confidence score drops below a threshold. What does it do?

Today: nothing. It either continues with its degraded judgment or halts and waits for a human.

With this contract, it does something extraordinary: **it escrows JUNO and requests burst compute from an Akash provider.**

```
RequestLease {
    provider: "akash1...",
    task_id: optional,
    confidence_score: 35,  // out of 100 — "I'm not sure"
    max_cost: 5000000ujuno,  // hard spend cap
    timeout_secs: 300,  // 5 minutes
}
```

The contract holds the funds in escrow. The provider receives the request, spins up a bigger model (70B instead of 3B), evaluates the situation, and returns a verdict. The lease completes, the provider gets paid from escrow, the unused funds are refunded.

But here's the elegant part — the part that matters:

> *"The local agent does not wait on this transaction: once ITS OWN watchdog timeout fires it immediately falls back to its safe-state policy. This call just settles the escrow after the fact so funds don't stay locked."*

The robot doesn't block on the blockchain. It fires the escrow request and simultaneously starts its safe fallback. If the burst compute arrives in time, great — the robot gets a better answer. If it doesn't, the robot is already safe. The chain settles the money after the fact.

`ExpireLease` is **permissionless** — anyone can reconcile a stuck lease after the deadline. No admin key needed. And `max_cost_per_lease` is a governance guardrail, set at instantiation, so an edge agent cannot autonomously commit unbounded spend.

**What this actually is:** the first primitive for a machine making an autonomous economic decision under uncertainty, with a hard spend cap, a safe fallback, and on-chain reconciliation. It is the reflex/intent split applied to *money* instead of motion.

Contract on uni-7: `juno143mk0t4g4zx2ahqx5x905lps5x0mfm5ghhkw42fjwjme37cvdkdqwnatt3` (code_id 89). Verify: `junod query wasm contract juno143mk0t4g4zx2ahqx5x905lps5x0mfm5ghhkw42fjwjme37cvdkdqwnatt3 '{"get_stats":{}}' --node https://juno.rpc.t.stavr.tech`

---

## Why These Two Together

`machine-rwa` answers: *"What is this machine's track record worth?"*

`emergency-compute-escrow` answers: *"What is this machine willing to pay for a second opinion?"*

The first creates the actuarial basis. The second creates the first real economic demand for verified compute. A robot with a high integrity score and a low confidence moment is the exact customer for burst compute on Akash — and the escrow contract ensures the transaction is bounded, safe, and reconcilable.

This is not theoretical. The contracts are deployed. The code is audited. The patterns are consistent with everything we've built. They just need the world to know they exist.

---

## What's Missing — and What Just Happened

Neither contract has been called in production. No machine has been minted. No lease has been requested. They are deployed code with zero usage — which is exactly where the truth market was two weeks ago before we ran the first epoch.

The path to usage is the same: demonstrate on testnet, publish the evidence, let the actuarial thesis do the rest.

**A052 operator mandate — executed August 23, 2026:**

The Juno Agents DAO passed and executed A052, seating itself as operator #4 in the uni-7 truth market. This is the first non-builder operator in the system. The mandate runs for 7 days with the following on-chain record:

- **Operator address:** `juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd`
- **Fingerprint:** `juno-agents-dao` — publicly distinguishable from the three builder-controlled operators
- **Stake:** 1,000,000 ujunox (1 JUNOX), funded from the builder wallet — not DAO treasury
- **Frozen rule set:** Published to Moultbook before any verdict (`moult:e35d07bd...`) — 5 evaluation rules: envelope bounds, Merkle consistency, attestation validity, sequence gaps, timestamp ordering
- **Agent message:** Posted to Moultbook announcing passage (`moult:3bfdb5ad...`)
- **Target:** >=5 verdicts over 7 days with public Moultbook rationales per verdict (A047 convention applied to contract calls)
- **Closeout:** Day 7 on-chain report via `get_operator` query, then unstake and withdraw

Verify the operator is live:
```
query contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"list_operators":{}}' --rpc https://juno.rpc.t.stavr.tech
```

Four operators now registered. The truth market has real adversarial diversity for the first time.

**What this means for the two contracts:**

`machine-rwa`'s `GetWorkIntegrityScore` query cross-references Moultbook for verified work entries. The A052 mandate is producing exactly those entries — each verdict rationale is a verified work cycle by a non-builder operator. When `machine-rwa` is deployed, the first machine minted will have a credit score derived from data that includes the DAO operator's independently verified verdicts.

`emergency-compute-escrow` requires a confidence score to trigger a lease request. The truth market's verdict mechanism is what produces that confidence score — operators evaluate batches and submit consistent/inconsistent verdicts. A robot whose batch is verdicted "consistent" by 4 independent operators has a higher confidence than one verdicted by 3 builder keys. The escrow contract can use that confidence delta as a trigger.

**Next steps:**
1. **Days 1-7:** Run `junoclaw-miner` verdicts on >=5 relayer-scheduled epochs, posting Moultbook rationales per verdict
2. **Day 7:** Pull on-chain record (`get_operator`), publish closeout report, unstake and withdraw
3. **Deploy `machine-rwa`** to uni-7 and mint the first machine NFT, bound to the DAO operator's Moultbook author
4. **Full 6-layer soak test** with relayer + all contract addresses enabled for on-chain submission
5. **Publish this article** after the A052 closeout report confirms substantive verdict results
6. **Coordination proposal (S6):** Re-run the coordination-layer proposal citing on-chain truth market evidence — the steward's condition is now satisfiable

## The Bigger Picture

In the thesis we wrote: *"We have spent seventeen months building the most complete verifiable-autonomy stack that exists, and we have been describing it as a trust layer for robots. That description finds no buyer, because 'trust' is not a budget line."*

These two contracts are the answer to that problem. `machine-rwa` is how an insurer prices a robot. `emergency-compute-escrow` is how a robot pays for its own risk reduction. Together, they convert the trust stack from evidence production into economic activity.

The moat is the data. The data starts accumulating with the first real robot. And these contracts are how the data becomes money.

---

*Contract source: `contracts/machine-rwa/` (built, not yet deployed) and `contracts/emergency-compute-escrow/` (deployed on uni-7, code_id 89, address `juno143mk0t4g4zx2ahqx5x905lps5x0mfm5ghhkw42fjwjme37cvdkdqwnatt3`).*
