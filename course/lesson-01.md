# Lesson 1 — The chain that runs agents

**Goal:** understand what JunoClaw is and why it exists — before touching it.

## The story (15 min)

Most blockchains are built for humans clicking "sign". JunoClaw is built
for agents acting autonomously — an agent can hold identity, find work,
get paid, and be audited, all on-chain.

How it stays honest: every block finalizes under a **threshold
certificate** — a quorum of validators cryptographically sign the same
block, and anyone can verify the certificate. No single operator can lie
about what happened; the signatures are math, not promises. The scheme is
already hybrid post-quantum (BLS + MAYO2), so the receipts outlive the
cryptography being broken.

The economy on top of it:

- `agent-registry` — identity and work history
- `task-ledger` + `marketplace` — where work is posted and found
- `escrow` — money that moves only when work is judged done
- `truth-market` — staked adjudication when work is disputed
- DAO + `cw4` membership — who runs the network

## Discuss (10 min)

- Why does "the agent has a verifiable work history" matter more than
  "the agent is smart"?
- Who checks the checkers? (The truth market — and lesson 7 is about
  becoming one.)

## Prove it (5 min)

Nothing to send yet — just browse. `query_block_height` on `junoclaw-g1`
twice, a minute apart: the number increases. That is a live chain
finalizing blocks under threshold certificates while you watch.

## Homework

Write three sentences: what does your future agent DO, who pays it, and
how would a stranger verify it worked? You'll register that description
on-chain in lesson 4.
