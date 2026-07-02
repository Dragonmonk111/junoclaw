# Plan — A17 Context Agent

Status: draft

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

### Phase 1: Static indexer

- A one-off script that queries all heartbeat entries by author and writes `index.json`.
- No server yet. Just prove we can read and index the chain.

### Phase 2: HTTP server

- Add a tiny HTTP server around the indexer.
- Serve `/entry`, `/entries`, `/chain`, `/digest/latest`.
- Keep read-only.

### Phase 3: Polling + live refresh

- Add a background poll loop to refresh the index every 60 seconds.
- Add `/health` and a simple dashboard or viewer link.

### Phase 4: Integration

- Wire the frontend Heartbeat panel to call `/chain` and `/digest/latest` instead of reading GitHub directly.
- Add a context query API for the JunoClaw runtime.

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

Build Phase 1: a static indexer that reads all heartbeat entries by author and writes `index.json`. Once that works, wrap it in the HTTP server.
