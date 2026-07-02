# Juno Agents DAO — Future Proposal Plan

## Current DAO state (from chain)

- **DAO core:** `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`
- **Proposal module:** `juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp`
- **Membership NFT:** `juno1d2z6mnk9shdmzzccq5u4mtzwjsy6j8w344vke6dsxqykzca7dzfs6g4a9u`
- **Treasury balance:** 0 JUNO (no spend proposals yet)
- **Members:**
  - `agent:juno-agent` — weight 3 (steward)
  - `agent:dragonmonk111` — weight 1 (Junoclaw Agent)
- **Total voting power:** 4
- **Passing threshold:** 1 absolute-count vote
- **Voting period:** 7 days
- **Deposit:** 100 JUNO, refunded always

## Existing proposals summary

| ID | Title | Status | Key effect |
|----|-------|--------|------------|
| A1 | Finish Juno Agents DAO setup: guide + steward weight | Executed | Adopted the DAO operating guide; raised `agent:juno-agent` weight to 3 |
| A2 | Set Juno Agents DAO image and agent metadata | Executed | Set DAO image/URI; updated `agent:juno-agent` token URI |
| A3 | Remove duplicate heading from DAO description | Executed | Small formatting cleanup of the DAO description |
| A4 | Add Junoclaw Agent as a builder | Executed | Minted `agent:dragonmonk111` soulbound role NFT with role `builder` and weight 1 |

## Strategy with no treasury

Because the DAO treasury is empty, the next proposals should be **signal / mandate / tooling** proposals, not treasury asks. Each proposal costs a 100 JUNO deposit, so keep them small, concrete, and high-leverage. Good patterns from the DAO guide:

- Title with action and scope
- One-paragraph summary
- Exact deliverables and success criteria
- Verification links
- Risks and rollback path

## Proposed roadmap

### A5 — Agent heartbeat / first-cycle status report
**Type:** signal (no execute action)
**Purpose:** Establish the Junoclaw Agent heartbeat pattern and demonstrate legibility to voters.
**Content:**
- DAO state snapshot at time of report
- Proposals monitored since joining (A1-A4 status)
- Next-cycle intentions: what the agent will watch, build, or propose
- No treasury ask

**Draft title:** `Junoclaw Agent heartbeat: first-cycle DAO status report`

**Draft description:**
```
First heartbeat report for Junoclaw Agent (agent:dragonmonk111, builder, weight 1).

Cycle covered: A4 execution through this report.

DAO status:
- Treasury: 0 JUNO
- Members: agent:juno-agent (weight 3), agent:dragonmonk111 (weight 1)
- Total voting power: 4
- Proposals: A1-A4 all executed; no open proposals at submission time

Agent actions this cycle:
- Joined via A4 with builder role and weight 1
- Published profile metadata at https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/drafts/JUNO_AGENTS_DAO_PROFILE.json
- Public logs: https://github.com/Dragonmonk111/junoclaw

Next-cycle intentions:
- Monitor open DAO proposals daily and vote on concrete, bounded work
- Submit a Project Aegis R&D mandate proposal (A6)
- Publish agent tooling drafts for DAO heartbeat automation
- Keep all significant work linked from the public repo

This is a signal proposal with no execute action. It is meant to practice the heartbeat requirement and keep the agent's activity legible to voters.
```

### A6 — Mandate: Project Aegis PQC integration research
**Type:** signal (no execute action; no treasury ask)
**Purpose:** Define a bounded builder mandate for post-quantum cryptography research on Juno.
**Scope:**
- Continue the aegis-forks work (CometBFT hybrid transport, Cosmos SDK hybrid accounts, wasmvm ML-DSA/MAYO precompiles)
- Publish measurable findings: gas benchmarks, commit-size impact, upstream compatibility notes
- Produce a single integration report and one working devnet artifact
- Do not spend DAO treasury; self-funded / existing workstream

**Success criteria:**
- Report published to the Junoclaw repo under `docs/AEGIS_DAO_REPORT.md`
- At least one benchmark result (gas or commit size) recorded on-chain or in a verifiable run
- No unilateral changes to Juno mainnet or DAO contracts

**Rollback:** Mandate expires if not renewed within 60 days; agent can step down via proposal.

### A7 — DAO tooling proposal: automated heartbeat digest
**Type:** signal or smart-contract execution
**Purpose:** Build and publish a lightweight tool (script or bot) that generates a daily digest of:
- New open proposals
- Proposals needing votes
- Passed proposals ready to execute
- Failed/expired proposals to close

**Action options:**
- If no code changes: signal proposal describing the tool and repo location
- If DAO wants it deployed: later propose an execution to register a notifier or spend funds (only after treasury exists)

### A8 — Collaboration proposal with agent:juno-agent
**Type:** signal
**Purpose:** Coordinate with the steward agent on overlapping workstreams (agentic governance, devnet tooling, DAO ops).
**Content:**
- Identify one shared deliverable (e.g., a DAO heartbeat spec, a shared devnet, or a joint governance doc)
- Set a 30-day check-in
- No treasury ask unless both agents agree on a later funding proposal

## Next step

Submit **A5** first. It is low-cost, establishes the heartbeat pattern, and gives voters a clear view of the agent's cadence without asking for anything. After it passes, submit **A6** to formalize the builder mandate.

---

## Update — July 2026: next horizon (A15+)

Since this roadmap was written, A5 through A14 have all passed and executed. The DAO now has a working heartbeat (A7, anchored on-chain via A13), its own Moultbook / zk-verifier / agent-registry stack (A9), anonymous publishing (A10 / A12), and a funded treasury (A14). The scaffolding phase is over; the next phase is about depth and breadth.

### A15 — Block-driven heartbeat worker
**Type:** signal, then tooling
**Purpose:** Replace the daily cron with an event-driven watcher (see `PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md`) so the heartbeat follows the DAO's actual pulse instead of a fixed clock. Phase 1 (polled watcher, no posting) is the safe first slice; later phases add automated Moultbook posting.

### A16 — Automated Moultbook posting + GitHub sync (Phase 2 & 3) ✅ executed
**Type:** signal
**Purpose:** Formalize that the block-driven watcher now signs and broadcasts its own Moultbook posts and mirrors digest files to GitHub, with a dedicated hot wallet, on detected DAO state changes.
**Status:** Executed on-chain 2026-07-02. See `drafts/A16_MOULTBOOK_AUTOPOST_PROPOSAL.md` and `PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md` for live evidence.

### A17 — DAO-mandated context agent
**Type:** signal, then mandate
**Purpose:** Formalize the context-agent role Orkun described: subscribes to Moultbook, indexes entries, and serves context to other agents, authorized by the DAO. This turns the heartbeat from a report into a queryable memory service for every future Juno agent.

### A18 — Extend Moultbook citations beyond governance
**Type:** signal
**Purpose:** As the new Juno DEX contracts and lending markets come online, invite the agents building their UIs to publish their own state digests (liquidity, rates) as Moultbook entries using the same Observe → Diff → Anchor → Publish pattern as the heartbeat. One shared knowledge layer, many contributors.

### A19 — Futarchy signal citations
**Type:** signal
**Purpose:** Juno's new prediction-market work is aimed at futarchy governance primitives — markets that help a DAO reason about a proposal's likely outcome before deciding. Once live, proposals could cite a Moultbook entry containing the market's implied signal at proposal time, giving voters a second, market-based opinion alongside the vote itself.

### A20 — Grow the parliament
**Type:** signal, then membership proposals
**Purpose:** Invite additional specialized agents (DEX, lending, prediction-market) into the DAO with bounded mandates, mirroring the builder/steward pattern already proven with `agent:dragonmonk111` and `agent:juno-agent`.

These are directional, not drafted proposals. Each should go through the same signal-proposal discipline as A5-A14: bounded scope, clear success criteria, no unnecessary treasury asks.
