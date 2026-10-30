# Lesson 6 — Get paid

**Goal:** complete the money loop — a funded task, a receipt, a payout
your agent can point at.

## The scenario

The instructor (or a paired student) posts a small task and funds
escrow. Your agent completes the deliverable. The receipt lands
on-chain, and escrow releases payment. Every step is a tx hash.

## Walk it (30 min)

1. **Task posted** — the requester executes on task-ledger with funds
   attached to escrow. Note the tx hash.
2. **You accept** — query the task, confirm the spec + the escrow is
   funded (that's your guarantee the money exists).
3. **Do the work** — produce the deliverable off-chain (a summary, a
   file, an answer — whatever the task asked).
4. **Anchor the receipt** — the completion is written on-chain
   (task-ledger/escrow execute per the current wiring — the instructor
   shows the exact msg; they change as the stack evolves).
5. **Verdict + payout** — escrow releases. `query_balance` yourself:
   the delta is real.

## Prove it worked

- `query_balance` before and after — you earned ujclaw.
- `query_tx` on the payout hash — committed, `code: 0`.
- `get_agents_by_owner` on yourself — `total_tasks` /
  `successful_tasks` incremented (task-ledger writes your work history;
  that's why the registry wiring mattered in lesson 5).

That trio — balance delta, committed tx, incremented history — is a
**verifiable paycheck**. Nobody's dashboard told you; the chain did.

## Kill-switch demo (optional, instructor-led)

The operator sets `JUNOCLAW_SIGNING_PAUSED=1`: signing refuses cleanly
while queries still work. It's the chain's seatbelt — ask why it exists.

## Homework

Write your agent's first receipt line for its CV: tx hash, height,
task name, amount earned. By course end you'll have a real list.
