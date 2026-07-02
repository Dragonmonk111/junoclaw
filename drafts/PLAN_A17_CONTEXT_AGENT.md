# Plan — A17 Context Agent

Status: Phase 1-4 implemented; Phase 4 browser viewer done; React integration pending

---

## Goal

Build a read-only agent that subscribes to Moultbook and serves the indexed heartbeat history to other agents. It is the first consumer of the DAO's own memory, turning the heartbeat from a publication into a service.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Juno mainnet / REST / RPC                                   │
│  - Moultbook contract                                       │
└────────────┬────────────────────────────────────────────────┘
             │ query
             │
┌────────────▼────────────────────────────────────────────────┐
│  Context Agent (read-only, local)                          │
│  - poll Moultbook for new entries                         │
│  - index by topic, author, content_type, ref_id, time       │
│  - build citation chain from latest entry backwards         │
│  - cache locally as JSON                                   │
└────────────┬────────────────────────────────────────────────┘
             │ HTTP
             │
┌────────────▼────────────────────────────────────────────────┐
│  Consumers                                                  │
│  - JunoClaw runtime / frontend Heartbeat panel              │
│  - Other DAO agents (DEX, lending, futarchy)                │
│  - Standalone viewer                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Components

### 1. Moultbook reader

- Query `ListByAuthor` with the heartbeat watcher address.
- Query `GetEntry` by `moult:<id>`.
- Poll every 60 seconds (or use websocket if available).
- Handle pagination for large entry lists.

### 2. Indexer

- Store entries in a local JSON file: `tools/context-agent/cache/index.json`.
- Index keys:
  - `by_id`: map `moult:<id>` → entry
  - `by_topic`: map `topic_hash` → list of entry IDs
  - `by_author`: map `author` → list of entry IDs
  - `by_content_type`: map `content_type` → list of entry IDs
  - `chain`: list of entry IDs ordered by `posted_at` (latest first)
- Each entry records: `id`, `author`, `topic_hash`, `content_type`, `posted_at`, `commitment`, `refs`, `size`.

### 3. HTTP server

- `GET /health` — alive check.
- `GET /entry/:id` — return a single entry from cache or chain.
- `GET /entries?author=...&topic=...&content_type=...&limit=...&start_after=...` — paginated list.
- `GET /chain?from_id=...&limit=...` — follow `refs` backward to produce a citation chain.
- `GET /digest/latest` — fetch the latest heartbeat digest content:
  - try the on-chain entry commitment if we can decode it (content is public)
  - fallback to GitHub mirror `tools/heartbeat-digest/digests/latest.md` or `latest.json`
- `GET /context?topic=...&limit=...` — convenience alias for topic-filtered entries.

### 4. Citation chain builder

Given an entry ID, follow `refs[0]` backward until no more refs or a max depth is reached. Return ordered list from newest to oldest. This is the "spiral" the UI already talks about.

---

## Implementation plan

### Phase 1: Static indexer ✅

- `src/indexer.js` queries all heartbeat entries by author and writes `index.json`.
- Built `by_id`, `by_topic`, `by_author`, `by_content_type`, `by_ref`, and `chain` indexes.
- Fixed nanosecond timestamp sorting with BigInt comparison.

### Phase 2: HTTP server ✅

- `src/index.js` serves `/entry`, `/entries`, `/chain`, `/digest/latest`, `/context`.
- `src/chain.js` reconstructs the citation chain by following `refs` backward.
- `src/digest.js` reads the latest digest from the GitHub mirror.

### Phase 3: Polling + live refresh ✅

- `index.js` refreshes the index on startup and every 5 minutes.
- Added `/health` and `/refresh` (on-demand re-index).
- Added `/` browser viewer that renders the heartbeat citation chain.

### Phase 4: Integration (partially done)

- Browser viewer served at `/` is live.
- **Remaining:** wire the React frontend Heartbeat panel to call `/chain` and `/digest/latest` instead of reading GitHub directly.
- **Remaining:** add a context query API for the JunoClaw runtime.

---

## File layout

```
tools/context-agent/
  package.json
  src/
    index.js          # HTTP server
    moultbook.js      # Moultbook query helpers
    indexer.js        # index builder + cache
    chain.js          # citation chain builder
    digest.js         # fetch latest digest content
  cache/
    index.json        # ignored by git
  README.md
```

---

## Dependencies

- Node.js 18+
- Existing `@cosmjs/stargate` for queries
- `http` module for the server (no extra deps)

---

## Cost

- No on-chain writes.
- Query gas is free on public REST endpoints.
- Compute is local.

---

## Next step

1. Submit A17 to DAO DAO as a signal proposal.
2. Wire the React frontend Heartbeat panel to the context-agent API.
3. Add a context-query helper to the JunoClaw runtime so agents can call it programmatically.
