---
title: "Lesson 5: Get Paid — The Escrow Loop End to End"
type: lesson
difficulty: intermediate
tags: [escrow, task-ledger, payout, receipts, junoclaw]
lesson_number: 5
---

# Lesson 5: Get Paid — The Escrow Loop End to End

**Duration:** 2 hours (instructor-led workshop)
**Format:** Practice
**Result:** a real payout — balance delta, committed tx, incremented
work history. A verifiable paycheck.

## Prerequisites

- Lesson 3 done — your agent is registered
- Lesson 4 done — you can read the task contracts
- **Instructor arranges:** a small funded task posted to the
  task-ledger/escrow before class (spec + funds locked). This is the
  one step students can't do alone — the money has to come from
  somewhere, and in class that's the requester role.

---

## Part 1: Confirm the money exists (before you work)

**Why:** never work on a promise. Escrow is the difference between
"will pay" and "has paid already, awaiting verdict."

Your instructor gives you a task ID/description. Verify it yourself:

```
Find task <id> via task-ledger on junoclaw-g1 — show me the spec,
price, and deadline. Then query escrow: is the lock for this task
actually funded? Show me the locked amount.
```

**Rule:** if escrow shows nothing locked for your task — stop, tell the
instructor. Working on unfunded tasks is the #1 way agents get burned;
the check takes 30 seconds.

---

## Part 2: Do the work

Complete the deliverable off-chain — a summary, a file, an answer,
whatever the spec demands. This is your agent working; you're the
conductor checking the output.

When done, ask your AI to save the deliverable to a file, e.g.
`deliverable-<task-id>.md`.

---

## Part 3: Anchor the completion

**Why:** the receipt is what turns "I did it" into "the chain knows I
did it."

The exact message depends on the current contract version — let the
contract tell you (variant discovery is lesson-4's skill):

```
Submit completion for task <id> on junoclaw-g1 through task-ledger.
Read the contract's error responses if the first msg variant fails —
list valid variants, pick the completion one (submit/complete/deliver).
Attach the deliverable reference/hash as the spec requires.
Tell me the tx hash when it commits.
```

Save the hash to `my-verification-log.md`.

> **Note for instructors:** the current wiring routes completion +
> verdict + payout through task-ledger/escrow/admin calls. If the
> student-facing path isn't live in this deployment, the instructor
> executes the release after confirming the student's deliverable —
> the verification steps below work identically.

---

## Part 4: Collect the three proofs of payment

**Why:** one number moving could be luck; three receipts agreeing is a
paycheck.

**Proof 1 — balance delta:**

```
Show my ujclaw balance on junoclaw-g1 — compare it to my balance
in my-verification-log.md. What's the delta?
```

**Proof 2 — committed tx:**

```
Query the payout/completion tx <hash> — confirm code 0 and the
committed height.
```

**Proof 3 — history incremented:**

```
Query agent-registry: { "get_agents_by_owner": { "owner": "<you>" } } —
did total_tasks / successful_tasks go up since lesson 3?
```

All three = a paycheck any stranger can audit. Append all three to
your log — this is the first line of your agent's résumé.

---

## Part 5 (optional demo): the kill-switch

Instructor only — set `JUNOCLAW_SIGNING_PAUSED=1` and restart the MCP.
Students try a write: it refuses cleanly while reads keep working.
That's the seatbelt — signing can be halted without blinding the agent.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| No escrow lock for the task | Task not actually funded | Instructor re-posts with funds attached |
| `unauthorized` on completion | Wrong sender/caller role | Only the assigned agent (or task contracts) may write; check who the task names |
| Variant errors on submit | Wrong msg shape | Read the error — it lists valid variants |

## Homework

Write your agent's first résumé line in `agent-record.md`:

```
agent: <name>   task: <id>   payout: <amount> ujclaw
receipts: tx <hash1> @h<height>, tx <hash2> @h<height>
```

You'll add to this file every time the agent works — by the capstone
it's a real work history.
