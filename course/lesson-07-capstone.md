---
title: "Lesson 7: Capstone — Your Agent's Public Record"
type: lesson
difficulty: intermediate
tags: [capstone, verification, work-history, github, junoclaw]
lesson_number: 7
---

# Lesson 7: Capstone — Verify → Identify → Earn → Adjudicate, Live

**Duration:** 2 hours (workshop)
**Format:** Practice + presentation
**Result:** a public, auditable work history for your agent — assembled
from your own queries, pushed to GitHub, verifiable by anyone.

## Prerequisites

- Lessons 1–6 done
- `my-verification-log.md` and `agent-record.md` with real entries

---

## Part 1: Run the full arc, one pass

Work this checklist in order — every step is a query or tx you already
know. If any check fails, that's a real finding, not a skip.

- [ ] **verify** — `query_block_height` on junoclaw-g1 + a
      `get_config` read from agent-registry, task-ledger, escrow,
      truth-market. One line each into your log.
- [ ] **identify** — `{ "get_agents_by_owner": { "owner": "<you>" } }`
      returns your agent, active.
- [ ] **earn** — your lesson-5 payout stands: balance delta + tx
      receipt + incremented history. (If you haven't completed a paid
      task, the instructor runs one with you now — it takes ~20 min.)
- [ ] **adjudicate** — participate in or observe a truth-market round:
      record the stake, the verdict, the outcome. Observing counts;
      understanding the round is the bar, not the stake.

---

## Part 2: Assemble the public record

**Why:** a résumé you can't verify is a claim. Yours is a transcript.

Ask your AI:

```
Assemble my agent's public record from my-verification-log.md and
agent-record.md into agent-record-final.md in this format:

agent: <name> (<registry profile>)
owner: <address>
chain: junoclaw-g1
registration: tx <hash> @ height <h>
tasks completed: <n>   successful: <n>   trust_score: <s>
payouts: <tx hashes>
adjudication: <stake + outcome, or "observed round <id>">
verified by: <how a stranger can check each line>
```

Anyone in the world can verify every line with the lesson-2 queries.
**That's the diploma — it's on-chain, not printed.**

---

## Part 3: Publish it

**Why:** GitHub from lesson 1 becomes your agent's portfolio home.

```
Push agent-record-final.md to a new public GitHub repo named
<agent-name>-record. Walk me through it — I'm the owner, you do the
git work.
```

Your record is now: on-chain (the txs), independently verifiable (the
queries), and publicly readable (the repo).

---

## Part 4: Peer verification (the exercise that matters)

Swap with a classmate. Take ONE line from their record — a tx hash, a
height, a profile claim — and verify it yourself:

```
Query the transaction <their-hash> on junoclaw-g1 — does it say what
their record claims? Show me the proof.
```

If it checks: their record is real. If it doesn't: you just did a
truth-market adjudication for free — say so, kindly. This is the job
graduates get paid for, practiced once for real.

## Present (5 min each)

Read your record aloud. Name one thing you'd do differently operating
the agent for a real week.

---

## After the course

- **Keep operating** — your `wallet_id` and profile are yours; the
  economy keeps running. New tasks, more verdicts, growing history.
- **Climb the operator track** — `paths-operator.md` maps the next
  ladder: DAO membership via cw4 → observer seat → seat attestation →
  relay/validator. Your capstone record is the application.
- **The skills transfer** — MCP tools, contract reads, signed writes,
  receipt verification work on every chain this MCP serves.

## Homework (the real one)

Run your agent for a week. Come back with its work history — entries
added to `agent-record-final.md`, verified the same way. The chain
doesn't close; neither does the course.
