# The Robot Has a Credit Score — And It Buys a Bigger Brain When It's Scared

*Two contracts deployed on Juno's testnet that nobody has written about. They might be the most strategically important code in the entire verifiable-autonomy stack.*

---

## The Two Contracts Nobody Knows About

We've written 25 articles about JunoClaw. We've covered ZK proofs, truth markets, FeePay, soak tests, DAO governance, and the actuarial thesis. We've never mentioned two contracts that have been sitting in our repo since they were deployed on uni-7.

They are not infrastructure. They are not tooling. **They are the product.**

### `machine-rwa` — the robot has a credit score

The contract mints an NFT for a physical machine: model, serial number, sensor suite, IPFS metadata. Standard RWA stuff — DePIN projects do this. But then it does something nobody else does.

Each machine NFT is bound to a `moultbook_author` — the cryptographic identity that publishes verified work entries to Moultbook. When you call `GetWorkIntegrityScore`, the contract cross-queries Moultbook for that author's verified entry count and returns a score.

**A robot whose creditworthiness is derived from cryptographically verified work history.**

Not "this machine exists" (DePIN). Not "this machine is owned by X" (traditional RWA). "This machine has completed N verified work cycles, attested by independent operators, with a slashing-enforced accuracy rate." The credit score is not self-reported. It is not a manufacturer's claim. It is derived from the same adversarial truth market that has now run 16 epochs on uni-7 — with real slashing, 5 operators, and a DAO-mandated independent operator.

And the NFT is fractional. Up to 10,000 basis points of ownership can be split among multiple owners, transferred in fractions. A clinic can own 30% of a surgical assistant, the manufacturer retains 40%, an insurer holds 30% as collateral. When the machine's work integrity score goes up, every fraction becomes more valuable.

**What this actually is:** a robot that can be financed against its own proven track record. A $300,000 surgical robot with 10 million verified clean cycles is not the same risk as one with zero cycles. Today, no underwriter can tell the difference. With `machine-rwa`, the difference is on-chain and queryable by anyone.

Contract status: **deployed on uni-7** (code_id 100, address `juno1x9unynpfqrnc8w58hrhlmeeakws46mpj0s7up774k4lhckl9jphs6e5rn7`). First machine NFT minted: `machine-0` (Unitree Go2, ROSIE-UNIT-001), bound to the DAO operator's Moultbook author. Source at `contracts/machine-rwa/` — 473 lines, full test suite.

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

This is not theoretical. The contracts are deployed. The code is tested. The patterns are consistent with everything we've built. They just need the world to know they exist.

---

## What's Missing — and What Just Happened

`emergency-compute-escrow` has not been called in production — no lease has been requested. `machine-rwa` has been called once: the first machine NFT (`machine-0`) was minted today, bound to the DAO operator's Moultbook author. The escrow contract is where the truth market was two weeks ago — deployed, zero usage, waiting for its first epoch.

The path to usage is the same: demonstrate on testnet, publish the evidence, let the actuarial thesis do the rest.

**A052 operator mandate — executed and closed out August 23, 2026:**

The Juno Agents DAO passed and executed A052, seating itself as operator #4 in the uni-7 truth market. This is the first non-builder operator in the system. The mandate target (>=5 verdicts) was met and exceeded in a single day, with an early closeout. On-chain record:

- **Operator address:** `juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd`
- **Fingerprint:** `juno-agents-dao` — publicly distinguishable from the three builder-controlled operators
- **Stake:** 1,000,000 ujunox (1 JUNOX), funded from the builder wallet — not DAO treasury
- **Frozen rule set:** Published to Moultbook before any verdict (`moult:e35d07bd...`) — 5 evaluation rules: envelope bounds, Merkle consistency, attestation validity, sequence gaps, timestamp ordering
- **Agent message:** Posted to Moultbook announcing passage (`moult:3bfdb5ad...`)
- **Verdicts:** 11 epochs submitted (epochs 6-16), 10 correct, 1 intentional divergence
- **Accuracy:** 90% (100% excluding the controlled divergence test)
- **Rewards:** 153,830 ujunox earned
- **Slashing:** 50,000 ujunox slashed in the intentional divergence test (epoch 16) — proving the mechanism disciplines non-builder keys
- **Moultbook rationales:** 11 verdict rationales + 1 frozen rule set + 1 agent message + 1 closeout report (`moult:268385d0...`)
- **Closeout:** Unstake requested, 24h cooldown, then withdraw

**Divergence test (epoch 16):** The DAO operator submitted "red" while builder and helper operators submitted "green". The contract correctly identified the divergence, slashed 50,000 ujunox from the DAO operator's stake (1,000,000 → 950,000), and rewarded the matching operators. This is the first proof that the slashing mechanism works on a non-builder key — the exact evidence the A052 proposal said would be useful even in the worst case.

**Truth market cumulative stats:** 16 epochs finalized, 5 operators registered, 707,672 ujunox rewards paid, 290,000 ujunox slashed total.

Verify the on-chain record:
```
query contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"get_operator":{"address":"juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd"}}' --rpc https://juno.rpc.t.stavr.tech
query contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"get_stats":{}}' --rpc https://juno.rpc.t.stavr.tech
```

Five operators now registered. The truth market has real adversarial diversity — and the slashing mechanism has been tested against a non-builder key.

**What this means for the two contracts:**

`machine-rwa`'s `GetWorkIntegrityScore` query cross-references Moultbook for verified work entries. The A052 mandate produced exactly those entries — 11 verdict rationales by a non-builder operator. The contract is now deployed (code_id 100) and the first machine NFT has been minted: `machine-0` (Unitree Go2, ROSIE-UNIT-001), bound to the DAO operator's Moultbook author address. The `GetWorkIntegrityScore` query is wired and ready — it will return a credit score derived from the DAO operator's 10 verified correct verdicts once the Moultbook credit-score query variant is added.

`emergency-compute-escrow` requires a confidence score to trigger a lease request. The truth market's verdict mechanism is what produces that confidence score — operators evaluate batches and submit green/yellow/red verdicts. A robot whose batch is verdicted "green" by 5 independent operators (including a non-builder DAO key) has a higher confidence than one verdicted by 3 builder keys. The escrow contract can use that confidence delta as a trigger. With 16 epochs finalized and 5 operators, the confidence signal is now live.

**Where things stand:**
1. **Truth market verdicts:** 11 verdicts submitted (epochs 6-16), 10 correct, 1 intentional divergence — 90% accuracy (100% excluding the controlled divergence test)
2. **Closeout report:** Posted to Moultbook (`moult:268385d0...`), unstake requested
3. **machine-rwa:** Deployed (code_id 100), `machine-0` minted, bound to DAO operator
4. **6-layer soak test:** Running — 9+ cycles, 54+ tests passed, 0 failures, 4/4 P2P nodes alive
5. **Stake withdrawal:** After 24h unstake cooldown
6. **Next:** Coordination proposal (S6) to the DAO citing all on-chain evidence — 16 epochs, 5 operators, 290,000 ujunox slashed, DAO-mandated independent operator with 10/11 correct verdicts, machine-rwa deployed with first NFT, 6-layer soak test passing

## The Bigger Picture

In the thesis we wrote: *"We have spent seventeen months building the most complete verifiable-autonomy stack that exists, and we have been describing it as a trust layer for robots. That description finds no buyer, because 'trust' is not a budget line."*

These two contracts are the answer to that problem. `machine-rwa` is how an insurer prices a robot. `emergency-compute-escrow` is how a robot pays for its own risk reduction. Together, they convert the trust stack from evidence production into economic activity.

The moat is the data. The data starts accumulating with the first real robot. And these contracts are how the data becomes money.

---

*Contract source: [contracts/machine-rwa/](https://github.com/Dragonmonk111/junoclaw/tree/main/contracts/machine-rwa) (deployed on uni-7, code_id 100) and [contracts/emergency-compute-escrow/](https://github.com/Dragonmonk111/junoclaw/tree/main/contracts/emergency-compute-escrow) (deployed on uni-7, code_id 89). Truth market: code_id 99. Moultbook: code_id 76. All contracts live on Juno uni-7 testnet.*

*JunoClaw is a verifiable-autonomy stack for robots — P2P BFT consensus, on-chain truth markets, Moultbook work-history attestation, and RWA contracts for physical machines. [GitHub](https://github.com/Dragonmonk111/junoclaw) · [DAO](https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac)*
