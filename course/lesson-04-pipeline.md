---
title: "Lesson 4: The Task Pipeline — Where Work Comes From"
type: lesson
difficulty: intermediate
tags: [task-ledger, marketplace, escrow, queries, junoclaw]
lesson_number: 4
---

# Lesson 4: The Task Pipeline — Where Work Comes From

**Duration:** 90 minutes
**Format:** Practice
**Result:** you can trace a task's full journey through the economy —
and read whether the money is really there.

## Prerequisites

- Lesson 3 done — your agent is registered
- `my-verification-log.md` from lesson 2

---

## The pipeline (read once, then go prove each hop)

```
requester posts task + funds escrow
        ↓
task-ledger records it (spec, price, deadline)
        ↓
marketplace makes it discoverable
        ↓
agent completes the work
        ↓
receipt anchors on-chain → verdict → escrow pays
```

The one idea that makes "earn" real: **the requester's money is
committed to escrow before work starts.** It's not a promise to pay —
it's locked funds the chain can release on verdict.

---

## Part 1: Read each contract's own rules

**Why:** every hop is a contract with live parameters — read them.

```
Query task-ledger on junoclaw-g1 with { "get_config": {} }.
Which contracts does it trust? What can write to it?
Contract: juno1ah3hkymcsr7nzm2sjpapl0r3yd72r9pekg723tdvr0lxe4vzacmqrm778u
```

Expected: wired to agent-registry, escrow, and related contracts.
The point: task-ledger only accepts history-writes from contracts it
trusts — your agent's record can't be faked by a random caller.

```
Query escrow with { "get_config": {} }.
Contract: juno15xg0w67jehudu4cmrsslm6cxkfyemwjf99728334jwj9nk3thaksr8gprp
What's the timeout, and the denom?
```

The timeout is how long money can stay locked — important when you're
deciding whether to accept a task.

```
Query marketplace with { "get_config": {} }.
Contract: juno1t08z3ta2tq7fz73rlxw4y6mxualjqdvh6rw7zpvguunmd7ly45as8ay25e
What does it reference, and what's the cancel window?
```

Append all three configs to `my-verification-log.md`.

---

## Part 2: Find the work

**Why:** this is a skill, not a lookup — contract query variants differ
by version, and the error messages teach you the real API.

```
Query task-ledger for open tasks. Try these variants until one works:
{ "list_tasks": {} }, { "get_tasks": {} }, { "get_open_tasks": {} }
When a variant errors, READ the error — it usually lists valid variants.
Show me whatever tasks exist, or confirm the market is empty.
```

Two outcomes, both real findings:

- **Tasks exist** → pick one: what's the spec, the price, the deadline?
  Ask: `Is its escrow actually funded? Query escrow for this task's
  lock.` — verify the money exists before imagining the work.
- **No open tasks** → the marketplace is young; your first paid task
  will come from the instructor in lesson 5. Same checks apply.

---

## Part 3: Understand who may write your history

**Why:** this boundary is the difference between a résumé and graffiti.

Ask your AI:

```
Explain the write-access pattern I just saw: why does task-ledger only
accept calls from wired contracts, and why does that make my agent's
work history trustworthy?
```

Short version: only the escrow/registry path can mark tasks complete —
so `total_tasks` on your profile means money actually moved, not that
someone typed a number.

---

## Part 4: Draft your first listing

**Why:** flip roles — you'll post work for others someday (or the
instructor posts for you in lesson 5).

```
Help me design one task listing for the junoclaw-g1 marketplace:
- a deliverable a truth-market could verify objectively
- a fair price in ujclaw
- a "done" definition a stranger could check
Save it to first-task-draft.md.
```

Good first tasks are objectively checkable: "summarize this document to
<500 words," "compute sha256 of this file," "answer these 5 factual
questions." Verifiable-by-stranger is the whole game — a verdict market
can't adjudicate taste.

## Prove it worked

Your log now shows: three contract configs you queried yourself, plus
either a found task (with escrow check) or the confirmed-empty market.
That's pipeline literacy.

## Homework

Refine `first-task-draft.md` until the "done" definition is so precise
that two strangers would always agree whether it's met. Bring it to
lesson 5 — it's what gets funded.
