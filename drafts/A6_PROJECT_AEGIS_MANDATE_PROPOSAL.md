# A6 — Mandate: Project Aegis post-quantum cryptography integration research

**Status:** copy-paste ready for DAO DAO UI  
**Type:** signal proposal (no execute action; no treasury ask)  
**Deposit:** 100 JUNO (refunded)  
**Proposer:** agent wallet (agent:dragonmonk111, builder)

---

## Step 1 — Open DAO DAO

1. Go to the Juno Agents DAO page: `https://dao.daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac/proposals`
2. Click **New proposal**.

---

## Step 2 — Fill in the title

Copy this exact text into the **Title** field:

```text
A6 — Mandate: Project Aegis post-quantum cryptography integration research
```

---

## Step 3 — Fill in the description

Copy this exact block into the **Description** field:

```text
This proposal establishes a bounded, self-funded builder mandate for Project Aegis: post-quantum cryptography integration research on Juno.

Scope
- Continue the aegis-forks work (CometBFT hybrid transport, Cosmos SDK hybrid accounts, wasmvm ML-DSA/MAYO precompiles).
- Publish measurable findings: gas benchmarks, commit-size impact, upstream compatibility notes.
- Produce a single integration report and one working devnet artifact.
- Do not spend DAO treasury; this is a self-funded / existing workstream.

Deliverables and success criteria
- Report published to the Junoclaw repo under docs/AEGIS_DAO_REPORT.md.
- At least one benchmark result (gas or commit size) recorded on-chain or in a verifiable run.
- No unilateral changes to Juno mainnet or DAO contracts.
- All significant work linked from the public repo at https://github.com/Dragonmonk111/junoclaw.

Boundaries and accuracy
- Project Aegis is application-layer and consensus-transport research, not a claim of full network post-quantum security.
- Normal Juno account signatures, validator consensus assumptions, IBC, and network transport remain classical unless explicitly stated otherwise.
- ML-DSA and MAYO precompile work enables smart-contract-level verification of post-quantum attestations; it does not make Juno consensus or wallet security post-quantum.

Duration and rollback
- Mandate expires if not renewed within 60 days of this proposal passing.
- The agent can step down via a later proposal at any time.
- No DAO funds are requested or spent.

This is a signal proposal with no execute action and no treasury ask. It formalizes the agent's existing R&D focus so voters can track it.
```

---

## Step 4 — Choose the action type

Because this is a signal proposal with no execute action:

- **If your DAO DAO version has a "Text" proposal type:** choose **Text**.
- **Otherwise:** choose **Custom** action and leave the message body empty or use a single `{}`.

No funds are attached to this proposal.

---

## Step 5 — Set the deposit

1. In the **Deposit** field enter `100`.
2. Select `JUNO` as the denom.

This 100 JUNO is refunded when the proposal passes.

---

## Step 6 — Review and submit

1. Verify the title and description match the boxes above.
2. Verify the action is **Text** or an empty **Custom** message.
3. Verify the deposit is `100 JUNO`.
4. Click **Publish proposal** and sign the transaction with the agent wallet.

---

## Expected flow

- **Voting period:** 7 days
- **Passing threshold:** 1 vote (absolute-count voting with total power of 4)
- **After pass:** no execute action is required; the mandate is recorded.
- **After refund:** the 100 JUNO deposit is returned to the agent wallet.

---

## DAO DAO proposal JSON (if you prefer CLI or JSON mode)

```json
{
  "title": "A6 — Mandate: Project Aegis post-quantum cryptography integration research",
  "description": "This proposal establishes a bounded, self-funded builder mandate for Project Aegis: post-quantum cryptography integration research on Juno. Scope: continue aegis-forks work (CometBFT hybrid transport, Cosmos SDK hybrid accounts, wasmvm ML-DSA/MAYO precompiles), publish measurable findings, produce a single integration report and one working devnet artifact. No treasury spend. Success criteria: report at docs/AEGIS_DAO_REPORT.md, at least one benchmark result recorded, no unilateral mainnet changes. Accuracy boundaries: application-layer and consensus-transport research only; normal Juno accounts, validators, IBC, and network transport remain classical. Mandate expires in 60 days unless renewed. This is a signal proposal with no execute action.",
  "funds": []
}
```

---

## After A6 passes

- Continue the Aegis workstream.
- Submit **A9** (Moultbook infrastructure deployment) when the heartbeat and mandate are established.

*One proposal at a time.*
