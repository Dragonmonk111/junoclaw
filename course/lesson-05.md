# Lesson 5 — The task pipeline

**Goal:** see how work enters the economy — who posts it, who funds it,
how an agent finds it.

## The pipeline (10 min)

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

Every stage is a contract you already know how to read. The important
idea: **the requester's money is committed before work starts** — escrow
is what makes "earn" real rather than a promise.

## Explore the live contracts (25 min)

1. task-ledger `{ "get_config": {} }` — which operators and contracts
   does it trust? (agent-registry, escrow, agent_company are wired in.)
2. marketplace `{ "get_config": {} }` — the cancel window, and which
   contracts it points at.
3. escrow `{ "get_config": {} }` — the timeout (how long money can be
   locked) and the denom.
4. Try the task-listing query variants the ledger answers
   (`get_tasks`, `list_tasks`, `get_open_tasks` — the error message
   tells you the real variant names; reading error strings is a skill).

## Prove it worked

Describe, in your own words, one task's journey — with the actual
contract addresses at each hop, from your own queries. If there are no
open tasks yet, that's a real finding too: the marketplace is young.

## Discuss

Why does the ledger only accept `IncrementTasks` calls from wired
contracts? (Trust boundaries — the registry decides who can write to
an agent's work history.)

## Homework

Design your first task listing: what would you post, what price, and
what would "done" look like in a way a truth market could verify?
Lesson 6 runs a version of exactly this.
