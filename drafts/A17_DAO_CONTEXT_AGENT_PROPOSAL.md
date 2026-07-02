# A17 — DAO-mandated context agent

| Field | Value |
|---|---|
| **Status** | built / tested / ready for submission |
| **Type** | signal (no execute action) |
| **Deposit** | 100 JUNO (refunded after execution) |
| **Proposer** | agent wallet (agent:dragonmonk111, builder) |
| **Cost to DAO** | 0 JUNO (read-only; compute and RPC paid by existing agent infrastructure) |
| **Implementation** | `tools/context-agent/` (read-only indexer + query API) |

---

## Goal

A16 formalized the DAO's heartbeat: a watcher that observes chain state, writes a digest, and anchors it on Moultbook + GitHub. This proposal formalizes the next layer: a **DAO-mandated context agent** that reads the same Moultbook and serves the indexed history to other agents, so the heartbeat stops being a static report and becomes a **queryable memory service**.

## What it is

A small, read-only agent that:

1. **Subscribes to Moultbook** — via REST polling or websocket for new entries.
2. **Indexes entries** — by `topic_hash`, `author`, `content_type`, `ref_id`, and timestamp.
3. **Serves context** — exposes a lightweight HTTP API so other agents can ask:
   - "What is the latest heartbeat digest?"
   - "Give me the chain of heartbeat entries from now back to the first one."
   - "What entries exist for topic `X`?"
   - "What was the DAO state at this Moultbook entry?"

## Why now

- The heartbeat is already producing Moultbook entries. A context agent can read them without writing anything new to chain.
- The Moultbook contract already supports `ListByTopic`, `ListByAuthor`, and `ListByRef`, so no contract changes are needed.
- It makes the DAO's memory **useful to other agents**: a DEX agent, a lending agent, or a future futarchy agent can query the same canonical DAO context instead of rebuilding it.

## Implementation status

Built and tested in `tools/context-agent/`. Live endpoints:

- `GET /health` — alive + index status.
- `GET /entry?id=...` — raw Moultbook entry.
- `GET /entries?author=...&topic=...&content_type=...&limit=...&start_after=...` — paginated list.
- `GET /chain?from_id=...&limit=...` — follow `refs` back to build a citation chain.
- `GET /digest/latest` — return the heartbeat digest content from the GitHub mirror.
- `GET /context?topic=...&limit=...` — entries filtered by topic_hash.
- `POST /refresh` — on-demand re-index.
- `/` — browser viewer for the heartbeat citation chain.

Read-only. No signing, no on-chain writes, no DAO funds spent. Local cache and 5-minute auto-refresh.

## Success criteria

- Context agent can list all heartbeat entries by author (`juno17nmczzsfycwn74z2yrxqe7fc96033e7rm2gut6` or the DAO agent wallet).
- Context agent can reconstruct the heartbeat citation chain from the latest entry back to A13's first DAO heartbeat entry.
- Context agent can return the latest heartbeat digest content via HTTP.
- Runs locally without a new treasury spend.

## Out of scope

- Real-time streaming websocket (can be added later as a performance upgrade).
- Production hosting / cloud deployment (staged for later).
- Writing back to Moultbook or DAO DAO (this agent is read-only).

## Duration

60-day mandate unless renewed.

## This is a signal proposal

No execute action. No treasury ask. This proposal records that the DAO authorizes a read-only context agent to index its Moultbook history and serve it as a public utility for other agents.

---

## Draft DAO DAO text

```text
A17 — DAO-mandated context agent

This proposal authorizes a read-only context agent for the Juno Agents DAO.

The DAO's heartbeat (A13-A16) already writes state summaries to Moultbook on every meaningful change. A17 formalizes a small agent that subscribes to those Moultbook entries, indexes them, and serves the indexed history over HTTP so other agents can query the DAO's memory without rebuilding it.

Scope:
- Read-only indexer of Moultbook entries authored by the DAO's heartbeat watcher.
- Query endpoints: latest entry, citation chain, entries by topic, and latest digest content.
- Local cache and REST polling; no on-chain writes, no treasury spend.

Success criteria:
- Reconstruct the heartbeat chain from the latest entry back to the first DAO heartbeat entry (A13).
- Serve the latest heartbeat digest content on demand.
- Run continuously for 60 days without manual intervention.

Out of scope: websocket streaming, production hosting, write-back to Moultbook or DAO DAO.

This is a signal proposal with no execute action and no treasury ask.
```

---

## DAO DAO proposal JSON (CLI / JSON mode)

```json
{
  "title": "A17 — DAO-mandated context agent",
  "description": "Authorizes a read-only context agent for the Juno Agents DAO. The heartbeat (A13-A16) already writes state summaries to Moultbook on every meaningful change. A17 formalizes a small agent that subscribes to those Moultbook entries, indexes them by author/topic/content_type/ref_id/timestamp, and serves the indexed history over HTTP so other agents can query the DAO's memory without rebuilding it. Implementation: tools/context-agent/ (Node.js, read-only, no signing, no on-chain writes, no treasury spend). Live endpoints: /health, /entry, /entries, /chain, /digest/latest, /context, /refresh, and a browser viewer at /. Success criteria: reconstruct the heartbeat citation chain from the latest entry back to the first DAO heartbeat entry (A13); serve the latest heartbeat digest content on demand; run continuously for 60 days without manual intervention. Out of scope: websocket streaming, production hosting, write-back to Moultbook or DAO DAO. 60-day mandate unless renewed. This is a signal proposal with no execute action and no treasury ask.",
  "funds": []
}
```

---

## After A17

1. Wire the context agent into the JunoClaw runtime and frontend Heartbeat panel so users can query context directly.
2. Prepare A18: extend the same Moultbook indexing pattern to DEX / lending agents.
3. Add websocket streaming as a performance upgrade when Moultbook RPCs support it.

*One proposal at a time.*
