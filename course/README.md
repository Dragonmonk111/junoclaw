# JunoClaw AI-Course

A practical course for operating AI agents on a live blockchain built for
agents. Students finish with a live agent on the JunoClaw testnet — on-chain
identity, a completed paid task, and a publicly verifiable work history.

Audience: no programming background required. Students act as the agent's
operator and conductor — the AI does the technical work through the MCP
tool interface.

## Course arc

**verify → identify → earn → adjudicate**

| Lesson | Title | Outcome |
|---|---|---|
| 1 | The chain that runs agents | Student understands Simplex BFT, threshold certs, and the agent economy |
| 2 | Your cockpit: MCP + wallet | MCP server installed, `wallet_id` enrolled, `list_chains` works |
| 3 | Verify with your own eyes | Live queries: height, balances, contract state — no trust needed |
| 4 | Give your agent an identity | `register_agent` executed; `get_agents_by_owner` returns the profile |
| 5 | The task pipeline | Discovers work via task-ledger + marketplace queries |
| 6 | Get paid | Completed escrow-funded task flow; receipt verified via `query_tx` |
| 7 | Truth markets | Understands adjudication; stakes or observes a verdict round |
| 8 | Capstone workshop | Full arc in one session — agent has a public, auditable history |

## What makes this course different

Other courses end with a deployed app or a hackathon entry. This one ends
with an **on-chain work history**: the student's agent has receipts —
registration, task completions, verdicts — that any third party can audit
without trusting anyone. It's a résumé the agent carries, not a certificate
the student holds.

## Files

- `overview.md` — the concepts map (registry → marketplace → escrow →
  truth-market → receipts → DAO-gated relay)
- `instructor-checklist.md` — what the mentor must arrange before lesson 1
- `verify-before-teaching.md` — pre-flight checks so every step in class works
- `lesson-01.md` … `lesson-08.md` — the lessons

## The network today

Live testnet `junoclaw-g1`: Simplex BFT consensus, hybrid BLS+MAYO2
post-quantum-ready certificates, CosmWasm contract stack (agent-registry,
task-ledger, escrow, truth-market, marketplace, skill-registry), 8 → 10
external validators, public observer/gRPC access.
