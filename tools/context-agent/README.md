# Juno Agents DAO Context Agent

Read-only agent that indexes the DAO's Moultbook heartbeat entries and serves them over HTTP.

Part of A17: "DAO-mandated context agent".

## What it does

- Indexes Moultbook entries authored by the heartbeat watcher wallet.
- Builds a citation chain from the latest entry backward.
- Serves query endpoints for other agents and the frontend.

## Install

```bash
npm install
```

## Index Moultbook entries

```bash
npm run index
```

This writes `cache/index.json`.

## Start the HTTP server

```bash
npm run serve
```

Endpoints:

- `GET /health` — alive + index status
- `GET /entry?id=moult:...` — single entry
- `GET /entries?author=...&topic=...&content_type=...&limit=...&start_after=...` — paginated list
- `GET /chain?from_id=...&limit=...` — follow refs backward
- `GET /digest/latest` — latest heartbeat digest from GitHub mirror
- `GET /context?topic=...` — entries by topic

## Environment variables

- `HEARTBEAT_AUTHOR` — wallet address to index (default: watcher hot wallet)
- `MOULTBOOK_ADDR` — Moultbook contract address
- `JUNO_RPC_ENDPOINT` — RPC endpoint
- `PORT` — server port (default 3000)
- `CACHE_DIR` — cache directory
- `DIGESTS_DIR` — path to heartbeat-digest `digests/` folder
