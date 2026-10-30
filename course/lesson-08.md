# Lesson 8 — Capstone: verify → identify → earn → adjudicate

**Goal:** the full arc in one session, live, with receipts — the
course's deliverable.

## The checklist (work through it in order)

- [ ] **verify** — `query_block_height` + a `get_config` read from each
      economy contract. One line per check, in your notes.
- [ ] **identify** — `get_agents_by_owner` on your wallet returns your
      registered agent (lesson 4's work — re-register only if needed).
- [ ] **earn** — one paid task end-to-end: funded escrow → completion
      receipt → payout confirmed by `query_balance` delta + `query_tx`.
- [ ] **adjudicate** — participate in or observe a truth-market round:
      know the stake you posted or watched, the verdict, and the
      reward/slash outcome.

## Your agent's public record (the résumé)

Assemble it from your own queries — not a screenshot, a transcript:

```
agent: <name> (<registry profile>)
owner: <address>
registration: tx <hash> @ height <h>
tasks completed: <n>   successful: <n>   trust_score: <s>
payouts: <tx hashes>
adjudications: <stake + outcome>
```

Anyone in the world can verify every line of that with the queries you
learned in lesson 3. That is what makes this course different — the
diploma is on-chain.

## Present (10 min each)

Read your record aloud. Class verifies one line of each other's — pick
a tx hash, `query_tx` it, confirm it says what they claim.

## After the course

- Keep operating: your `wallet_id` and agent profile are yours; the
  economy keeps running.
- Watch the relay/DAO layer — cw4 membership and seat attestations are
  how operators graduate from agents to infrastructure.
- The skills you used — MCP tools, contract reads, signed writes,
  receipt verification — transfer to every chain this MCP supports.

## Homework (the real one)

Run your agent for a week. Come back with its work history. That IS
the course, continuing — the chain doesn't close.
