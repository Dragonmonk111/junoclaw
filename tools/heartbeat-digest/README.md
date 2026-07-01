# Juno Agents DAO Heartbeat Digest

A free, automated daily digest of the Juno Agents DAO state.

## Where to read the latest digest

```text
https://github.com/Dragonmonk111/junoclaw/blob/main/tools/heartbeat-digest/digests/latest.md
```

## How it works

- A GitHub Actions workflow runs once per day at 00:00 UTC.
- A Node.js script queries the Juno public REST endpoint.
- It reads the DAO core, proposal module, voting module, and treasury.
- It writes a markdown digest to `digests/latest.md` and `digests/YYYY-MM-DD.md`.
- The bot commits the files back to the repo.

## Run locally

```bash
cd tools/heartbeat-digest
npm run dry-run
```

To actually write files:

```bash
npm run digest
```

## Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `DAO_CORE` | `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac` | DAO core address |
| `PROPOSAL_MODULE` | `juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp` | DAO DAO proposal module |
| `REST_ENDPOINT` | `https://juno-rest.publicnode.com` | Juno REST API |
| `DRY_RUN` | `false` | If `true`, print digest to stdout instead of writing files |

## Costs

All costs are zero:

- Compute: GitHub Actions free tier for public repos (2,000 minutes/month).
- RPC queries: Free public Juno REST endpoints.
- Storage: Markdown files committed to the GitHub repo.
- Execution: Triggered by GitHub Actions or the agent's local machine.

## Manual trigger

Go to the repo's **Actions** tab, select **Juno Agents DAO Heartbeat Digest**, and click **Run workflow**.

## Block-driven watcher (B3 Phase 1)

`src/watch.js` polls the DAO's on-chain state and only regenerates the digest when something meaningful changes (new proposal, vote, status change, treasury movement, membership change), instead of on a fixed daily clock. It does **not** post to Moultbook yet — see `drafts/PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md` for the full phased plan.

```bash
cd tools/heartbeat-digest
npm run watch:once   # one poll cycle, then exit
npm run watch        # poll every POLL_INTERVAL_MS (default 5 minutes) until stopped
```

State is kept in `tools/heartbeat-digest/state/last-state.json` (git-ignored): the last state hash, the block height it was observed at, and a human-readable diff used to populate `meta.trigger_reason` / `meta.changes` in `latest.json`.
