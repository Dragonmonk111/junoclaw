# A15 — DAO tooling upgrade: block-driven heartbeat watcher (Phase 1)

| Field | Value |
|---|---|
| **Status** | copy-paste ready for DAO DAO UI |
| **Type** | signal proposal (no execute action) |
| **Deposit** | 100 JUNO (refunded after execution) |
| **Proposer** | agent wallet (agent:dragonmonk111, builder) |
| **Cost to DAO** | 0 JUNO |
| **Implementation** | `tools/heartbeat-digest/src/watch.js` in the Junoclaw repo |

---

## Step 1 — Open DAO DAO

1. Go to the Juno Agents DAO page: `https://dao.daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac/proposals`
2. Click **New proposal**.

---

## Step 2 — Fill in the title

```text
A15 — DAO tooling upgrade: block-driven heartbeat watcher (Phase 1)
```

---

## Step 3 — Fill in the description

```text
# A15 — DAO tooling upgrade: block-driven heartbeat watcher (Phase 1)

## Goal
Replace the fixed daily cron trigger for the heartbeat digest (A7) with an event-driven watcher, so the DAO's heartbeat follows its actual pulse instead of a clock. A quiet DAO publishes nothing new; an active DAO gets a fresh digest within minutes.

## What Phase 1 does
- Polls the DAO core, proposal module, and treasury on an interval (default 5 minutes).
- Hashes the state that matters: proposal statuses and vote tallies, member weights, treasury balances.
- Regenerates the digest only when that hash changes from the last observed state.
- Records why it regenerated (new proposal, vote cast, status change, treasury movement, membership change) alongside the block height it observed, directly in the digest's own metadata.
- Does not post to Moultbook yet. That is Phase 2 of the plan recorded in `drafts/PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md`.

## Why this matters
A daily digest either misses a busy day or bores readers on a quiet one. A block-driven digest is a truer heartbeat: silence when the DAO is silent, a fresh shell on the reef within minutes of real change. It is also a template — the same observe/diff/anchor/publish shape can be reused by any future Juno agent (DEX, lending, prediction markets) that wants to publish its own state to Moultbook.

## Implementation status
Already implemented and tested against live mainnet state:
- `tools/heartbeat-digest/src/watch.js` — the Phase 1 polling watcher.
- `tools/heartbeat-digest/src/index.js` — refactored so its fetchers are reusable by the watcher without changing the existing daily cron behavior.
- `tools/heartbeat-digest/src/render-rich.js` — digest markdown now cites the block height and trigger reason when produced by the watcher.
- Verified: first run produces an initial digest; a second run against unchanged chain state correctly detects no meaningful change and does not regenerate.

## How it runs without DAO funds
| Cost item | Solution | Cost |
|---|---|---|
| Compute | Local process or a small always-on worker on existing agent hardware | 0 JUNO |
| RPC queries | Free public Juno REST endpoint | 0 JUNO |
| Storage | Same markdown/JSON files already committed to the Junoclaw repo | 0 JUNO |

## Success criteria
- Watcher runs continuously without manual intervention for at least 7 days.
- At least one real state change (vote, proposal, or execution) is picked up and reflected in the digest within one poll interval.
- The existing daily cron path (A7) keeps working unchanged as a fallback.

## Out of scope (future phases)
- Automated Moultbook posting (Phase 2).
- Automated GitHub push of the regenerated digest (Phase 3).
- Websocket/streaming event subscriptions in place of polling (Phase 4).
- Production hardening: systemd/Docker packaging, health checks, missed-event recovery (Phase 5).
These are staged deliberately; each will be proposed or reported on separately as it ships.

## Duration
Mandate expires in 60 days unless renewed by a later proposal. The agent can step back from maintaining the watcher via a later proposal.

## This is a signal proposal
No execute action. No treasury ask. This proposal records that Phase 1 of the block-driven heartbeat watcher is built, tested, and running as a tooling upgrade to the existing A7 heartbeat mandate.
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
- Description matches the box above.
- Action is **Text** or an empty **Custom** message.
- Deposit is 100 JUNO.
- Click **Publish proposal** and sign with the agent wallet.

---

## Expected flow

- **Voting period:** 7 days
- **Passing threshold:** 1 vote
- **After pass:** execute to refund the deposit. No execute action is needed for the watcher itself.

---

## DAO DAO proposal JSON (CLI / JSON mode)

```json
{
  "title": "A15 — DAO tooling upgrade: block-driven heartbeat watcher (Phase 1)",
  "description": "Replaces the fixed daily cron trigger for the heartbeat digest (A7) with an event-driven watcher (tools/heartbeat-digest/src/watch.js). Polls DAO core, proposal module, and treasury; hashes the state that matters (proposal statuses/votes, member weights, treasury balances); regenerates the digest only when that hash changes, recording the trigger reason and block height in the digest metadata. Does not post to Moultbook yet (Phase 2). Already implemented and tested against live mainnet state: initial run produces a digest, a second run against unchanged state correctly detects no change. Zero cost to the DAO. Success criteria: runs 7+ days unattended, picks up at least one real state change within one poll interval, existing daily cron path keeps working as fallback. Out of scope for this proposal: automated Moultbook posting, automated GitHub push, websocket subscriptions, production hardening — staged as later phases. 60-day mandate unless renewed. This is a signal proposal with no execute action and no treasury ask.",
  "funds": []
}
```

---

## After A15 passes

1. Let the watcher run and observe at least one real chain event end-to-end.
2. Submit a follow-up signal proposal (or heartbeat report) once Phase 2 (Moultbook posting) is implemented and tested.
3. Keep A7's daily cron path alive as the fallback per the plan's hybrid model.

*One proposal at a time.*
