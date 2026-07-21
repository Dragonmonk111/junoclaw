# proposal-indexer

Auto-index DAO DAO proposals as citable `application/json+agent-proposal` AKB exports in Moultbook.

## Run

Single pass (index all unposted proposals):

```powershell
$env:JUNO_REPLY_BOT_MNEMONIC = '<mnemonic>'
npm run once
```

Force a single proposal (useful for A32 right now):

```powershell
$env:JUNO_REPLY_BOT_MNEMONIC = '<mnemonic>'
npm run force -- --proposal-id 32
```

Daemon mode:

```powershell
$env:JUNO_REPLY_BOT_MNEMONIC = '<mnemonic>'
npm run daemon
```

Dry-run:

```powershell
$env:MOULTBOOK_DRY_RUN = 'true'
npm run once
```

## How it works

1. Polls the `dao-proposal-single` module via `list_proposals`.
2. Skips anything already recorded in `indexer-state.json`.
3. Looks for a local markdown override named `A<id>_*.md` in `drafts/`.
4. Builds an AKB v1.1 export envelope.
5. Signs and broadcasts via `tools/reply-bot/src/moultbook.js`.
6. Records the resulting `moult_id` / `tx_hash` so the same proposal is never posted twice.

## Configuration (env)

- `DAO_CORE`, `PROPOSAL_MODULE`, `REST_ENDPOINT`, `MOTHER_MOULT_ID`
- `REPLY_BOT_NAME`
- `PROPOSAL_OVERRIDE_DIR` — where to look for `A<id>_*.md` overrides
- `PROPOSAL_DEFAULT_TAGS`, `PROPOSAL_DEFAULT_REFS`
- `POLL_INTERVAL_MS` — default 5 minutes
- `INDEXER_STATE_FILE` — path to the idempotency state file
