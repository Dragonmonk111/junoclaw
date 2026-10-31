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
| 1 | Your cockpit: GitHub, IDE, AI + the JunoClaw MCP | Accounts created, Node + IDE + MCP installed, `list_chains` and block-height work — first contact with the chain |
| 2 | Verify with your own eyes | Live queries: height, balances, real contract configs — `my-verification-log.md` started |
| 3 | Give your agent an on-chain identity | `register_agent` executed; profile verified two ways (`get_agents_by_owner` + `query_tx`) |
| 4 | The task pipeline | Reads task-ledger/escrow/marketplace configs; verifies escrow is funded; drafts a first listing |
| 5 | Get paid — the escrow loop | Instructor-funded task completed; payout proven three ways (balance delta, tx receipt, history increment) |
| 6 | Truth markets — the job this course trains you for | Adjudication incentive math from live config; operator + verdict tracing; domain self-assessment |
| 7 | Capstone — the public record | Full arc in one pass; `agent-record-final.md` pushed to GitHub; peer cross-verification |

Each lesson is step-by-step and copy-paste driven — students paste
prompts into their AI panel and get real results on a live chain.
Students who already have a GitHub account, an LLM provider, an IDE,
and a funded `wallet_id` can start at lesson 2.

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
- `lesson-01-setup-cockpit.md` … `lesson-07-capstone.md` — the lessons
- `paths-operator.md` — the post-course ladder: agent → DAO member →
  observer → attested seat → relay/validator

## The network today

Live testnet `junoclaw-g1`: Simplex BFT consensus, hybrid BLS+MAYO2
post-quantum-ready certificates, CosmWasm contract stack (agent-registry,
task-ledger, escrow, truth-market, marketplace, skill-registry), 8 → 10
external validators, public observer/gRPC access.
