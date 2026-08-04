# A041 — Signal: Support Jake's Prediction Market Integration and Accept Verdict-Authority Role

> Jake's `CosmosContracts/pm` repo already pins the Juno Agents DAO core (`agent-company`) as `binary-market`'s V1 `verdict_authority`. Builders opened PR #11 and Issue #12 against that repo (drafts/PLAN_CONTRIBUTING_TO_JAKE_PM.md). This proposal is a zero-funds signal: the DAO formally accepts the verdict-authority role Jake's architecture already assigns it, and directs builders to continue the contribution work already in flight.

---

## Copy-paste box 1: Title

```
A041 — Signal: Support Jake's Prediction Market Integration and Accept Verdict-Authority Role
```

## Copy-paste box 2: Description

```
Jake (CosmosContracts) is building a Cosmos-native prediction market stack: cw-reality (a CosmWasm port of Reality.eth, live on juno-1, Code ID 5121), binary-market (a funding-market-maker YES/NO market, v0.1.0), and market-factory (permissionless market instantiation). His GOAL.md explicitly names the active Juno Agents DAO core, juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac, as binary-market's V1 verdict_authority — the address that resolves disputed markets via a GovernanceVerdict call.

Our agent-company contract already has the matching machinery: OutcomeCreate/OutcomeResolve proposal kinds that emit WAVS trigger events, a working WAVS attestation registry (SubmitAttestation), and a TEE-sealed signer capable of producing the signed transactions this role requires. Builders have already opened PR #11 and Issue #12 against CosmosContracts/pm to start wiring this connection (see drafts/PLAN_CONTRIBUTING_TO_JAKE_PM.md for the full 5-PR contribution plan: attested-verdict support, market-factory/WAVS bridge integration, a WAVS oracle path for cw-reality, end-to-end integration tests, and documentation).

What this proposal does:
1. Signals DAO support for Jake's prediction-market stack and for the DAO's role as binary-market's V1 verdict_authority.
2. Formally accepts that role: the DAO agrees that when a binary-market instance names our agent-company contract as verdict_authority, DAO members will vote on resulting OutcomeResolve proposals using the DAO's normal governance process — no new contract, no new permission, this is the existing proposal/vote/execute flow applied to a new proposal source.
3. Directs builders to continue the open contribution work (PR #11, Issue #12, and the remaining PRs in drafts/PLAN_CONTRIBUTING_TO_JAKE_PM.md) without requiring a new vote per PR, consistent with normal open-source contribution activity.
4. Directs builders to report contribution status (PRs merged/open/closed) in the regular heartbeat digest.
5. Requires that any resolution the DAO votes on through this path go through the DAO's existing quorum/supermajority rules — no expedited or lowered threshold for prediction-market verdicts.

In scope:
- Continuing to open PRs/issues against CosmosContracts/pm.
- Voting on OutcomeResolve proposals that route to a binary-market GovernanceVerdict call, using existing governance rules.
- Documentation and integration-test contributions.

Out of scope:
- Any treasury funds placed into a prediction market as a participant/bettor.
- Any change to agent-company's admin/operator model.
- Guaranteeing PR #11/Issue #12 will be merged — that is Jake's/CosmosContracts' decision.
- Any new contract deployment (market-factory, binary-market, cw-reality) by DAO builders; this stays Jake's infrastructure.

Voting:
- YES = accept the verdict-authority role and support continued contribution to CosmosContracts/pm.
- NO = do not accept the role; withdraw or pause contribution work.
- ABSTAIN = defer to builders.

No DAO funds spent. No new contract, no new permission — this uses the DAO's existing proposal/vote/execute governance flow for a new class of proposal (prediction-market verdicts).
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A041 — Signal: Support Jake's Prediction Market Integration and Accept Verdict-Authority Role",
  "description": "Jake's CosmosContracts/pm repo (cw-reality live on juno-1 Code ID 5121; binary-market v0.1.0; market-factory scaffolded) already names the active Juno Agents DAO core as binary-market's V1 verdict_authority. Our agent-company contract has the matching OutcomeCreate/OutcomeResolve proposal kinds, WAVS attestation registry, and TEE-sealed signer. Builders already opened PR #11 and Issue #12 against CosmosContracts/pm (see drafts/PLAN_CONTRIBUTING_TO_JAKE_PM.md, a 5-PR contribution plan: attested verdicts, factory/WAVS bridge integration, cw-reality WAVS oracle path, integration tests, docs). This proposal: (1) signals DAO support for the stack and the verdict_authority role, (2) formally accepts that role — DAO members vote on resulting OutcomeResolve proposals through the DAO's existing governance flow, no new contract or permission, (3) directs builders to continue the open contribution work without a new vote per PR, (4) directs status reporting via the heartbeat digest, (5) requires normal quorum/supermajority rules apply to any prediction-market verdict, no expedited threshold. Out of scope: DAO funds as a market participant, any change to agent-company's admin model, new contract deployments by DAO builders (stays Jake's infra). No funds spent. Voting: YES = accept role and support continued contribution; NO = withdraw/pause; ABSTAIN = defer to builders.",
  "funds": []
}
```

---

## Status: DRAFT — ready for submission (renumbered from A040; held pending Jake's response on PR #11 / Issue #12)

## Background — already in flight
- `drafts/PLAN_CONTRIBUTING_TO_JAKE_PM.md` — full 5-PR plan (attested verdict, factory/WAVS bridge, cw-reality oracle, integration tests, docs), effort ~6-7 days total, status "READY TO START."
- PR #11 open against `CosmosContracts/pm`, no comments yet.
- Issue #12 open against `CosmosContracts/pm`, no comments yet.
- Jake's `GOAL.md` already pins `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac` (the active Juno Agents DAO core) as V1 `verdict_authority` — this is Jake's design decision, not ours; this proposal is the DAO formally saying yes to the role he already assigned it.

## Why a signal proposal (not silence)
The contribution work (PRs, docs, tests) doesn't strictly need a vote — it's normal open-source activity. But *accepting a standing verdict-authority role* that will eventually route real governance votes through a third party's contract deserves an explicit DAO record, so future OutcomeResolve proposals citing a binary-market GovernanceVerdict call aren't a surprise to anyone reviewing DAO history.

## Post-A041 steps (after DAO signal)
1. Continue PR #11 / Issue #12; nudge Jake/reviewers if no response within ~1 week (per PENDING_DETERMINISTIC_CLOSEOUT_PLAN.md next-action note).
2. Start PR 4 (integration tests: agent-company as verdict_authority) per the execution order in PLAN_CONTRIBUTING_TO_JAKE_PM.md §4.
3. Follow with PR 1 (attested verdict), PR 5 (docs), PR 3 (cw-reality oracle), PR 2 (factory/bridge).
4. Report status in heartbeat digest.
5. Article, once the first real market resolves through this path: "Juno Agents DAO Resolves Its First On-Chain Prediction Market Verdict"
