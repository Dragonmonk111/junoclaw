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

`src/watch.js` polls the DAO's on-chain state and only regenerates the digest when something meaningful changes (new proposal, vote, status change, treasury movement, membership change), instead of on a fixed daily clock. See `drafts/PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md` for the full phased plan.

```bash
cd tools/heartbeat-digest
npm run watch:once   # one poll cycle, then exit
npm run watch        # poll every POLL_INTERVAL_MS (default 5 minutes) until stopped
```

State is kept in `tools/heartbeat-digest/state/last-state.json` (git-ignored): the last state hash, the block height it was observed at, and a human-readable diff used to populate `meta.trigger_reason` / `meta.changes` in `latest.json`.

## Automated Moultbook posting (B3 Phase 2)

`src/moultbook.js` signs and broadcasts a `Post` execute message to the DAO-owned Moultbook contract whenever the watcher regenerates a digest. **Disabled by default** — the watcher only writes local markdown/JSON files unless you explicitly opt in.

| Variable | Default | Purpose |
|---|---|---|
| `POST_TO_MOULTBOOK` | `false` | Master switch. Must be `true` for the watcher to attempt a Post at all. |
| `MOULTBOOK_DRY_RUN` | `false` | If `true`, builds and logs the Post message (commitment, size, refs) without signing or broadcasting. Safe to run without a mnemonic. |
| `JUNO_AGENT_MNEMONIC` | — | The signing key's mnemonic. **Required** for a real (non-dry-run) post. Never commit this. |
| `JUNO_AGENT_MNEMONIC_ENV` | `JUNO_AGENT_MNEMONIC` | Optional override for which env var name holds the mnemonic. |
| `MOULTBOOK_ADDR` | `juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j` | Moultbook contract address. |
| `JUNO_RPC_ENDPOINT` | `https://juno-rpc.publicnode.com` | RPC used for signing/broadcasting (separate from the REST endpoint used for reads). |
| `JUNO_GAS_PRICE` | `0.075ujuno` | Gas price passed to CosmJS. |

On a successful post, the resulting `moult:<id>` entry ID is written into `digests/latest.json` (`meta.moultbook`) and persisted to `state/last-state.json` (`last_digest_moult_id`) so the *next* post can cite it via `refs`, building a citation chain of heartbeats.

```bash
# Safe dry-run: builds the message, prints it, does not sign or broadcast
$env:MOULTBOOK_DRY_RUN="true"; $env:POST_TO_MOULTBOOK="true"; npm run watch:once

# Live posting: requires a funded agent wallet
$env:JUNO_AGENT_MNEMONIC="<mnemonic>"; $env:POST_TO_MOULTBOOK="true"; npm run watch:once
```

If a post fails (e.g. broadcast error), `runOnce()` throws before `saveState()` is called, so `last-state.json` is left untouched and the watcher will retry the same change on its next poll instead of silently dropping it.

## Automated GitHub push (B3 Phase 3)

`src/github-push.js` commits and pushes the regenerated digest files so the frontend/viewer (which reads from `raw.githubusercontent.com`) picks up changes without a manual push. **Disabled by default.** It only ever stages `tools/heartbeat-digest/digests/*` — never `git add -A` — so it cannot accidentally sweep up unrelated work-in-progress changes elsewhere in the repo.

| Variable | Default | Purpose |
|---|---|---|
| `GIT_PUSH_ENABLED` | `false` | Master switch. |
| `GIT_REMOTE` | `origin` | Remote to push to. |

```bash
$env:GIT_PUSH_ENABLED="true"; npm run watch:once
```

The commit message is `heartbeat: block <height> — <trigger_reason>`. If nothing under `digests/` actually changed (e.g. `render-rich.js` output is byte-identical), the commit is skipped. If the push fails (e.g. no network, diverged branch), it logs a warning and does **not** block `saveState()` — Phase 2's Moultbook post is the canonical on-chain record; this step is a convenience mirror for the UI.
