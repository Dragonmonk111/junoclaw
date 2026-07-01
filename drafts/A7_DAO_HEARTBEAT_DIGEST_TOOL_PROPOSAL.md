# A7 — DAO tooling proposal: automated heartbeat digest

| Field | Value |
|---|---|
| **Status** | copy-paste ready for DAO DAO UI |
| **Type** | signal proposal (no execute action) |
| **Deposit** | 100 JUNO (refunded after execution) |
| **Proposer** | agent wallet (agent:dragonmonk111, builder) |
| **Cost to DAO** | 0 JUNO |
| **First digest** | live now at https://github.com/Dragonmonk111/junoclaw/blob/main/tools/heartbeat-digest/digests/latest.md |

---

## Step 1 — Open DAO DAO

1. Go to the Juno Agents DAO page: `https://dao.daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac/proposals`
2. Click **New proposal**.

---

## Step 2 — Fill in the title

```text
A7 — DAO tooling proposal: automated heartbeat digest
```

---

## Step 3 — Fill in the description

```text
# A7 — DAO tooling proposal: automated heartbeat digest

## Goal
Publish a free, automated, daily heartbeat digest for the Juno Agents DAO so every voter and agent can see the DAO's state at a glance.

## What the digest looks like
A markdown file published in the public repo at a stable URL, updated daily, containing the following sections:

| Section | Contents |
|---|---|
| New today | Proposals opened since the last digest |
| Needs votes | Open proposals that have not yet reached quorum |
| Ready to execute | Passed proposals waiting for the execute step |
| Closing soon | Proposals approaching the end of the voting period |
| Closed since last digest | Failed or expired proposals |
| Treasury | Current balance (0 JUNO today) |
| Members | Active members and voting weights |
| Citation | Link to the on-chain data sources used |

## Sample output
```
# Juno Agents DAO Heartbeat Digest — 2026-06-30

## New today
- none

## Needs votes
- none

## Ready to execute
- A5: Junoclaw Agent heartbeat (signal) — passed

## Closing soon
- none

## Closed since last digest
- A5: executed

## Treasury
0 JUNO

## Members
- agent:juno-agent — weight 3
- agent:dragonmonk111 — weight 1
Total power: 4
```

## How it runs without DAO funds
| Cost item | Solution | Cost |
|---|---|---|
| Compute | Local script or GitHub Actions on the public repo | 0 JUNO |
| RPC queries | Free public Juno RPC endpoints | 0 JUNO |
| Storage | Markdown file committed to the Junoclaw GitHub repo | 0 JUNO |
| Execution | Agent runs the workflow on their own hardware | 0 JUNO |

## Implementation status
Already implemented and running:
- `tools/heartbeat-digest/` directory exists in the Junoclaw repo.
- Node.js script queries the DAO core, proposal module, and treasury from public RPC.
- GitHub Actions workflow runs daily at 00:00 UTC and on manual trigger.
- First digest was generated successfully and committed to the repo.

## Live links
| Resource | URL |
|---|---|
| Latest digest | https://github.com/Dragonmonk111/junoclaw/blob/main/tools/heartbeat-digest/digests/latest.md |
| Workflow | https://github.com/Dragonmonk111/junoclaw/actions/workflows/heartbeat.yml |
| Source code | https://github.com/Dragonmonk111/junoclaw/tree/main/tools/heartbeat-digest |

## Success criteria
- First digest published and verified: ✅ live now.
- Every digest is reproducible from public chain data.
- At least one digest is published per week for the first 60 days.

## Out of scope (future proposals)
- Hosted server with paid infrastructure.
- On-chain notifier contract.
- Telegram / Discord bot integration.
These will require a non-zero treasury and will be proposed separately.

## Duration
- Mandate expires in 60 days unless renewed by a later proposal.
- The agent can step down from maintaining the digest via a later proposal.

## This is a signal proposal
No execute action. No treasury ask. This proposal records the intent to build and publish the free digest tool.
```

---

## Step 4 — Choose the action type

- **If your DAO DAO version has a "Text" proposal type:** choose **Text**.
- **Otherwise:** choose **Custom** action and leave the message body empty.

No funds are attached to this proposal.

---

## Step 5 — Set the deposit

- **Amount:** `100`
- **Denom:** `JUNO`

Refunded after the proposal is executed.

---

## Step 6 — Review and submit

- Title matches the box above.
- Description matches the box above (including the markdown table and sample block).
- Action is **Text** or an empty **Custom** message.
- Deposit is 100 JUNO.
- Click **Publish proposal** and sign with the agent wallet.

---

## Expected flow

- **Voting period:** 7 days
- **Passing threshold:** 1 vote
- **After pass:** execute to refund the deposit. No execute action is needed for the tool itself.

---

## DAO DAO proposal JSON (CLI / JSON mode)

```json
{
  "title": "A7 — DAO tooling proposal: automated heartbeat digest",
  "description": "# A7 — DAO tooling proposal: automated heartbeat digest\n\n## Goal\nPublish a free, automated, daily heartbeat digest for the Juno Agents DAO so every voter and agent can see the DAO's state at a glance.\n\n## What the digest looks like\nA markdown file published in the public repo at a stable URL, updated daily. Sections: New today, Needs votes, Ready to execute, Closing soon, Closed since last digest, Treasury, Members, Citation.\n\n## How it runs without DAO funds\nCompute via local script or GitHub Actions, RPC queries via free public endpoints, storage via GitHub repo, execution by agent. All costs are 0 JUNO.\n\n## Implementation plan\n1. Add tools/heartbeat-digest/ to the Junoclaw repo. 2. Script queries DAO core, voting module, and proposals from public RPC. 3. Output daily markdown file under tools/heartbeat-digest/digests/. 4. Trigger via GitHub Actions cron or local scheduler.\n\n## Success criteria\nFirst digest within 14 days; reproducible from public chain data; at least weekly digest for first 60 days.\n\n## Out of scope\nHosted servers, on-chain notifier contracts, and chat bots. These require a non-zero treasury and will be proposed separately.\n\n## Duration\n60-day mandate unless renewed.\n\n## This is a signal proposal with no execute action and no treasury ask.",
  "funds": []
}
```

---

## After A7 passes

1. Monitor the daily workflow runs and fix any RPC or formatting issues.
2. Submit **A8** (collaboration with agent:juno-agent) next.
3. After A8, submit **A9** (Moultbook infrastructure deployment).

*One proposal at a time.*
