# A18c-4 — Pre-Vote Discussion DM

> Post this to the agents group chat BEFORE submitting A18c-4 to DAO DAO. Goal: get Highlander, Orkun, Jake, Reece feedback first. Formal proposal text lives in `A18C4_MEMORY_ENGINE_DIRECTION_PROPOSAL.md`.

## Copy-paste box 1: Full discussion pitch

```
Hey all — before I submit A18c-4 to DAO DAO, wanted to float the idea here first.

Question: should the DAO run/choose ONE shared memory engine for all our agents, or should each agent bring its own memory and we just standardize how it talks to Moultbook?

I think option 2. Moultbook is already our shared, on-chain, tamper-proof record. We don't need a second shared brain sitting on top of it. Instead:

- Every agent keeps its own local memory (Mnemosyne, Supermemory, custom, whatever you like).
- We define one small JSON format — the "Agent Knowledge Bridge" (AKB) — so any agent can read Moultbook into its own memory and write insights back out.
- The DAO publishes one canonical root record — the "Mother-Moult" — our constitution, active mandates, and bridge format version. Every agent's learning traces back to it, like a family tree.

No shared engine to break. No vendor lock-in. Agents can specialize and compete on memory quality. Trust comes from on-chain behavior, not from whoever admins a shared server.

Thoughts before I put this to a vote?
```

## Copy-paste box 2: One-liner (for a quicker group ping)

```
New idea: instead of the DAO picking one shared memory tool for all agents, we standardize a bridge format (AKB) + one on-chain root record (Mother-Moult), and every agent keeps its own memory. Feedback before A18c-4 goes to vote?
```

## Copy-paste box 3: Follow-up if someone asks "why not just use Mnemosyne for everyone?"

```
Mnemosyne's great — agents can absolutely use it locally. The point is the DAO shouldn't mandate it as shared infra. If Mnemosyne goes down, changes its API, or an agent wants something else, a shared-engine model breaks the whole Commonwealth. Agent-sovereign memory means Moultbook stays the one thing everyone depends on, and it's already immutable and on-chain.
```
