# A052 — DAO Operator Week: Juno Agents DAO Runs Its First Independent Truth Market Epochs

> The truth-market contract (`juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p`, code_id 99, uni-7) has run five epochs with three operators — all builder-controlled wallets. That satisfies `min_operators: 3` numerically and violates it in spirit: every verdict so far has been submitted by the same team. **This proposal seats the DAO as operator #4 in a 7-day live mandate**: the DAO registers, stakes, submits independent verdicts on at least 5 epochs, publishes a public rationale for each one, and delivers a verdict-accuracy report on day 7. Execution starts within 24 hours of passage. This is a role, like A041's verdict-authority acceptance — not an architecture endorsement.

---

## Copy-paste box 1: Title

```
A052 — DAO Operator Week: Juno Agents DAO Runs Its First Independent Truth Market Epochs (7-Day Mandate)
```

## Copy-paste box 2: Description

```
The truth-market contract is live on uni-7 (juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p, code_id 99). Five epochs have run: 173,731 ujunox in rewards distributed, 240,000 ujunox slashed from a diverging operator, per-batch verification fees enforced. Verify any of it with two RPC calls:

  query contract <addr> '{"get_stats":{}}' --rpc https://juno.rpc.t.stavr.tech
  query contract <addr> '{"list_operators":{}}' --rpc https://juno.rpc.t.stavr.tech

One honest gap: all three registered operators are builder-controlled wallets. min_operators: 3 is satisfied numerically, not in spirit — the adversarial-verification argument needs at least one operator that isn't a builder key.

What this proposal does:
1. Seats the DAO as operator #4 for a 7-day mandate. Builders generate a fresh wallet, fund it with 2 JUNOX (min_stake + gas) from the builder wallet — not DAO treasury — and register it with fingerprint "juno-agents-dao" so it is publicly distinguishable.
2. The DAO operator submits verdicts on the relayer-scheduled epochs during the week (minimum 5), running the published rule-based evaluator, with a public rationale posted to Moultbook per verdict — the A047 rationale convention applied to contract calls.
3. On day 7, builders report the on-chain record (epochs, correct/incorrect verdicts, rewards, slashes — from get_operator, not self-reported) in the heartbeat digest, then unstake and withdraw after the cooldown. The mandate ends; nothing renews automatically.

In scope:
- One operator registration, one wallet, one week of verdicts, one closeout report.

Out of scope:
- Any change to the truth-market contract's code, config, or admin.
- Any claim of organizational independence — builders host the process; what is independent is the key, the stake, and the published rule set. Recruiting a genuinely external operator is a separate future step.
- Any endorsement of the coordination-settler architecture rejected in A044/A045/A046/A048/A049. This proposal makes no claim about that architecture.
- Robot-generated data — batches remain simulated pending a real robot deployment.

A verdict proves operators agreed on a batch's classification under contract rules — nothing more. It does not prove sensor authenticity (that is TEE attestation, a hardware trust assumption) or hidden-state safety claims.

Risk: worst case, the DAO operator diverges and loses its 1 JUNOX stake to slashing — which would itself be useful evidence that the mechanism disciplines non-builder keys. Total exposure: 2 JUNOX from the builder wallet. No DAO treasury funds move.

Voting:
- YES = seat the DAO operator for the 7-day mandate.
- NO = do not seat an operator now.
- ABSTAIN = defer to builders on timing without authorizing.

No treasury funds spent. No contract changes. One bounded week, one report.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A052 — DAO Operator Week: Juno Agents DAO Runs Its First Independent Truth Market Epochs (7-Day Mandate)",
  "description": "The truth-market contract (juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p, code_id 99, uni-7) has run 5 epochs: 173,731 ujunox in rewards, 240,000 ujunox slashed, verification fees enforced — all verifiable via RPC (get_stats, list_operators). One honest gap: all 3 operators are builder-controlled wallets; min_operators: 3 is satisfied numerically, not in spirit. This proposal seats the DAO as operator #4 for a 7-day mandate: (1) builders generate a fresh wallet, fund it 2 JUNOX from the builder wallet (NOT treasury), register it with fingerprint 'juno-agents-dao'; (2) the operator submits verdicts on >=5 relayer-scheduled epochs using the published rule-based evaluator, with a public Moultbook rationale per verdict (the A047 convention applied to contract calls); (3) on day 7 builders report the on-chain record (epochs, verdicts, rewards, slashes from get_operator) in the heartbeat digest, then unstake and withdraw after cooldown. Mandate ends; nothing renews automatically. Out of scope: contract/config/admin changes; any claim of organizational independence (builders host the process — what is independent is the key, stake, and rule set; recruiting an external operator is a separate future step); any endorsement of the coordination-settler architecture rejected in A044/A045/A046/A048/A049; robot-generated data. A verdict proves operator agreement under contract rules — not sensor authenticity (TEE attestation, a hardware trust assumption) or hidden-state safety claims. Worst case: the DAO operator diverges and loses its 1 JUNOX stake — itself useful evidence that the mechanism disciplines non-builder keys. No DAO treasury funds move. Voting: YES = seat the DAO operator for 7 days; NO = do not seat an operator now; ABSTAIN = defer on timing without authorizing.",
  "funds": []
}
```

---

## Status: DRAFT — ready for submission

## Why this proposal, and why now

Five prior proposals asking the DAO to endorse the coordination-settler / consensus architecture (A044, A045, A046, A048, A049) were rejected 0-3, with the steward citing the same two problems twice: no independently verifiable public evidence, and unsupported claims (specifically naming J-Lens hidden-state safety claims). Two proposals of a different shape — A041 (accept a role Jake's contract already assigns us) and A047 (adopt a voting convention) — passed 4-0.

The pattern: **the DAO seats roles and adopts conventions. It does not endorse architecture on the strength of local test output.**

A052 is shaped like A041, not like A048/A049:
- It asks for a role (operator), not an endorsement of a system design.
- The evidence backing it (5 epochs, real slashing, real rewards) is already on-chain, on a public testnet, queryable by any DAO member with an RPC endpoint in under a minute — this is exactly the "independently verifiable artifact" the steward asked for in the A048/A049 rejections.
- It makes zero claims about J-Lens, hidden states, or cryptographic proof of physical safety. The only claim is arithmetic: operators submit verdicts, the contract computes consensus, rewards and slashes settle on-chain. Every number is checkable by re-running the same query.
- It is bounded to 7 days with hard success criteria and a mandatory closeout report — it either produces verifiable results inside one week or it is recorded as failed. There is no drift and no open-ended commitment.
- It actively fixes the weakness a careful reviewer would name next, rather than asking the DAO to look past it.

## Background

- Truth market live on uni-7 since 2026-08-17, migrated to code_id 99 with fee routing on 2026-08-22 (see `articles/TRUTH_MARKET_LIVE_2026_08_22.md`).
- Current operators: 3, all builder-controlled (`Rosie-alpha`, `Rosie-beta`, `Rosie-gamma` style fingerprints per `articles/ROSIE_MINES_TRUTH_2026_08_22.md`).
- `min_operators` config value: 3 — met numerically today, in spirit only after this proposal.
- `junoclaw-miner` CLI already supports `register`, `run`, `unstake`, `withdraw`, `deposit` against this exact contract — no new tooling required to execute this proposal.
- A047 (public vote rationales) already ratified as DAO convention; this proposal extends that convention to non-vote verdict submissions via Moultbook, since verdicts are contract calls, not governance votes.

## Builder appendix (not part of the on-chain proposal)

## Who runs what — the honest operational picture

A DAO is a contract on juno-1. It can pass proposals but cannot run processes. So "the DAO operates" means, concretely:

- **The DAO** votes on this proposal and owns the mandate. Its members do nothing else during the week except review the day-7 report.
- **Builders** host a 4th miner process on the same infrastructure that already runs `Rosie-alpha/beta/gamma` — but that process signs with a **fresh wallet created under this DAO mandate**, funded with DAO-mandated stake, running a **rule set published before any verdict is submitted**.
- **The relayer** (existing service) schedules epochs and calls `FinalizeEpoch`. Nothing about the relayer changes.
- **The contract** settles verdicts, rewards, and slashes — it cannot tell which physical machine an operator runs on. What it CAN show, publicly, is that operator #4 has a distinct key, distinct stake, and distinct verdict history from operators #1–3.

What this proves: key separation and rule independence. What it does NOT prove: organizational or infrastructure independence — the same hardware hosts all four miners, and the proposal says so. Genuine third-party independence (an unaffiliated party on their own hardware) is the follow-up step, not this one.

## Execution plan — specific steps

**Day 0 — proposal passes** (signal only, no execute messages).

**Day 1 — wallet + registration (builders, within 24h of passage):**
1. Create the operator wallet in the encrypted WalletStore:
   ```
   cosmos-mcp wallet add --id dao-truth-operator --chain uni-7
   ```
   No plaintext mnemonic at any point (same pattern as `deploy/enroll-builder.mjs`).
2. Fund it from the builder wallet — exactly 2 JUNOX (2,000,000 ujunox):
   ```
   cosmos-mcp bank send --from builder --to <dao-truth-operator-address> --amount 2000000ujunox --rpc https://juno.rpc.t.stavr.tech
   ```
   1,000,000 ujunox = min_stake (per get_config), 1,000,000 = gas + slashing buffer.
3. Register as operator #4:
   ```
   junoclaw-miner register --mnemonic dao-truth-operator \
     --identity-type dao \
     --fingerprint juno-agents-dao \
     --truth-market-contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p \
     --juno-rpc https://juno.rpc.t.stavr.tech
   ```
4. Verify registration is public:
   ```
   cosmos-mcp query contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"list_operators":{}}' --rpc https://juno.rpc.t.stavr.tech
   ```
   Expected: 4 operators, one with fingerprint "juno-agents-dao".
5. Publish the day-1 Moultbook entry BEFORE any verdict: operator address, the complete evaluation rule set (which checks the rule-based miner runs — envelope bounds, Merkle consistency, attestation signature validity, sequence gaps), and the expected epoch schedule. Rules are now frozen for the week.

**Days 1–7 — verdicts (builders hosting the process, wallet signing):**
6. For each relayer-scheduled epoch, run:
   ```
   junoclaw-miner run --mnemonic dao-truth-operator \
     --identity-type dao \
     --hardware dao-controlled \
     --submit-on-chain \
     --truth-market-contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p \
     --juno-rpc https://juno.rpc.t.stavr.tech
   ```
   Target: >=5 epochs. If the relayer schedules fewer than 5 in the window, builders schedule the shortfall directly.
7. After each verdict, post a Moultbook rationale entry: batch height, verdict ("consistent" / "inconsistent"), which rules fired, one sentence why. This extends the A047 rationale convention from governance votes to contract calls.

**Day 7 — closeout (builders report, DAO reviews):**
8. Pull the on-chain record:
   ```
   cosmos-mcp query contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"get_operator":{"address":"<dao-truth-operator-address>"}}' --rpc https://juno.rpc.t.stavr.tech
   ```
   This returns epochs_participated, correct_verdicts, incorrect_verdicts, total_rewards, total_slashed, accuracy — all settled on-chain, not self-reported.
9. Publish the mandate report (Moultbook + heartbeat digest): epochs, agreement rate vs the three Rosie operators, exact reward/slash amounts, and a yes-or-no answer to "did the DAO operator's verdicts differ materially from builder operators' verdicts."
10. Exit:
    ```
    junoclaw-miner unstake --mnemonic dao-truth-operator --truth-market-contract <addr> --juno-rpc <rpc>
    # wait 24h cooldown
    junoclaw-miner withdraw --mnemonic dao-truth-operator --truth-market-contract <addr> --juno-rpc <rpc>
    ```
    Stake returns to the builder wallet. Mandate ends. Continuation requires a new proposal.

**After closeout:**
11. If verdicts diverged materially, submit a follow-up recruiting a genuinely external operator (unaffiliated party, own hardware) — the real independence step.
12. Article: "Seven Days as Operator #4: What the DAO's Verdicts Actually Showed."
