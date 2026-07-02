# Plan — B3 Block-Driven Heartbeat Worker

Status: draft / discussion

---

## Goal

Replace the daily cron heartbeat with an event-driven worker. The worker watches the chain for meaningful DAO state changes — new proposals, votes, executions, treasury movements — and regenerates the digest only when the state actually changes. The digest is then posted to Moultbook and the GitHub JSON is updated.

This is the most efficient and "live" version of the heartbeat loop. It is also the most complex, so this plan breaks it into phases and flags the hard parts.

---

## Why block-driven changes the story

Daily cron is simple but wasteful. The DAO might do nothing for three days, or it might pass two proposals in an hour. A daily digest misses the latter and bores readers with the former.

Block-driven says: the heartbeat follows the DAO's actual pulse. If the DAO is quiet, nothing is published. If the DAO executes A15, the digest updates within minutes and anchors the change on Moultbook.

This also makes the heartbeat a real-time-ish context feed for the context agent Orkun described. The context agent can subscribe to Moultbook and see a fresh canonical summary whenever the DAO changes.

---

## Architecture overview

```
┌─────────────────────────────────────────────────────────────┐
│  Juno mainnet                                               │
│  - DAO core                                                 │
│  - Proposal module                                          │
│  - Treasury (DAO core)                                      │
│  - Moultbook                                                │
└────────────┬────────────────────────────────────────────────┘
             │ wasm events, bank events
             │
┌────────────▼────────────────────────────────────────────────┐
│  Event Listener / Worker (Node.js, persistent)             │
│  - subscribe to Tendermint / websocket events               │
│  - filter by contract address                               │
│  - detect meaningful state changes                          │
│  - apply cooldown + deduplication                           │
└────────────┬────────────────────────────────────────────────┘
             │ trigger
             │
┌────────────▼────────────────────────────────────────────────┐
│  Digest Builder                                             │
│  - query DAO state from REST/RPC                            │
│  - generate markdown + JSON                                   │
│  - compute SHA-256 commitment                                 │
└────────────┬────────────────────────────────────────────────┘
             │
     ┌───────┴───────┐
     │               │
┌────▼────┐   ┌──────▼──────┐
│ GitHub  │   │ Moultbook   │
│ JSON    │   │ Post tx     │
└─────────┘   └─────────────┘
```

---

## Components

### 1. Event Listener

Subscribes to chain events. Two practical options:

**Option A: CosmJS + Tendermint websocket (true streaming)**
- Use `Tendermint34Client.connect("wss://juno-rpc.publicnode.com")` or a local node.
- Subscribe to `tx` events.
- Filter events by `_contract_address` for the DAO core and proposal module.

Pros: near real-time, efficient.
Cons: public websocket endpoints are unreliable; you need a persistent node or a paid RPC for production.

**Option B: Polled REST with block-height cursor (robust)**
- Every N seconds, query the latest block height.
- Query transactions in the new blocks for the DAO/proposal module.
- Maintain a `last_seen_height` cursor.

Pros: works with public REST, easy to recover, no websocket dependency.
Cons: slightly slower, uses more RPC quota.

**Recommendation:** start with Option B. It is simpler, more robust, and fast enough for this use case. Move to Option A only after the worker is stable.

### 2. State Change Detector

Not every event is meaningful. We only care about changes that affect the digest:

- New proposal created
- Vote cast on an open proposal
- Proposal status changed (passed, executed, rejected, expired)
- Treasury balance changed
- Member count or weights changed (rare)

Implementation:
- Query the DAO state after the event.
- Compare with the last known state stored in a local file or small DB.
- Hash the state and compare. If the hash changed, regenerate.

### 3. Cooldown / Rate Limiter

During a voting surge, many events fire in a short window. We do not want to post a new Moultbook entry for every vote.

Rules:
- After a meaningful event, start a cooldown timer (e.g., 5 minutes).
- If more events arrive during cooldown, reset the timer.
- Regenerate only once after the cooldown ends.
- Cap the rate: max one digest per hour, or one per block window.

This keeps the feed responsive without spamming Moultbook.

### 4. Digest Builder

Reuse the existing `heartbeat-digest` tool. The worker calls the same logic but with a different entry point.

The builder should:
- Query the DAO state
- Generate `latest.md`, `latest.json`, etc.
- Optionally tag the output with the trigger event type

### 5. Moultbook Poster

Same as the current manual Post flow, but automated:
- Compute SHA-256 of the markdown
- Build the `Post` execute message
- Sign and broadcast with the agent key
- Record the returned `moult:<id>` in the JSON meta

Automation risk: if the worker is compromised, the key can post to Moultbook. Mitigate by using a dedicated key with limited funds and running the worker in a controlled environment.

### 6. GitHub Updater

After posting, commit and push the new digest files. This can be done via:
- Local git commands in the worker
- A GitHub API call with a fine-grained token

For a worker running on the user's Ubuntu machine, local git is simplest. For a cloud worker, use the GitHub API.

---

## Event sources and filters

| Contract | Address | Events to watch |
|---|---|---|
| DAO core | `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac` | `wasm-execute`, bank transfers (treasury) |
| Proposal module | `juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp` | `wasm-execute`, `wasm-create_proposal`, `wasm-vote`, `wasm-execute_proposal` |
| Treasury | same as DAO core | `coin_received`, `coin_spent`, `transfer` |

We do not need to parse every event. We can simply detect that a relevant contract was touched and then re-query the state.

---

## State change detection

The worker maintains a local `last-state.json`:

```json
{
  "block_height": 12345678,
  "state_hash": "sha256-of-serialized-digest-state",
  "trigger_reason": "vote_cast",
  "cooldown_until": "2026-07-01T20:10:00Z",
  "last_digest_moult_id": "moult:...",
  "last_generated_at": "2026-07-01T20:00:00Z"
}
```

When a relevant event fires:
1. Query the current DAO state.
2. Serialize the state into a canonical JSON form.
3. Compute SHA-256.
4. Compare to `state_hash`.
5. If different, update the digest, post to Moultbook, and update `last-state.json`.

This makes the trigger cheap and the digest generation deterministic.

A small subset of this internal state should mirror into the public digest JSON's own `meta` block — `block_height`, `trigger_reason`, and `previous_moultbook` (the prior entry's `moult:<id>`). None of this needs a new endpoint; it rides along in the same `latest.json` the frontend already reads. That mirrored data is what makes the heartbeat UI described below possible without extra plumbing.

---

## Failure modes

| Problem | Mitigation |
|---|---|
| RPC/websocket drops | Use block-height cursor; on restart, query from last known height |
| Missed events during downtime | Catch up by querying blocks between `last_seen_height` and current height |
| Duplicate posts | Rate limit + dedup by state hash |
| Chain re-org | Wait for 2+ confirmations before posting; if re-org detected, re-query state |
| Key runs out of funds | Monitor balance; alert before posting fails |
| Worker crashes | Run as systemd service or Docker container with restart policy |
| GitHub push fails | Retry with backoff; log failure |

---

## Implementation phases

### Phase 1: Polled watcher with state diff (MVP) — ✅ shipped

- Build a small Node.js script that polls every 5 minutes.
- Compare DAO state to last known state.
- If changed, regenerate the digest and save locally.
- Do not post to Moultbook yet.

Goal: prove the loop works without chain complexity.

Shipped as `tools/heartbeat-digest/src/watch.js`. Tested against live mainnet (initial run + no-change run). Formalized as **A15** and executed on-chain.

### Phase 2: Add Moultbook posting — ✅ shipped, ✅ live-tested on mainnet

- Add the Post transaction to the worker.
- Store the resulting `moult:<id>` in the digest meta.
- Run locally with a dedicated agent hot wallet (not the DAO steward/agent governance key — `execute_post` has no owner/allowlist check, so an isolated low-privilege wallet is used to bound blast radius).

Goal: the heartbeat updates automatically on-chain.

Shipped as `tools/heartbeat-digest/src/moultbook.js` (CosmJS `SigningCosmWasmClient`), wired into `watch.js` behind `POST_TO_MOULTBOOK` (default off) with a `MOULTBOOK_DRY_RUN` safety mode. Live-tested 2026-07-02: tx `EC50D6D18F2AE9A7DA5C40F323270A84764A8A9E09905D700201EC77A27310D4` → entry `moult:83bf7ea63a199ab7fd9484588385e5983371aa412086328d88c6b9e29417f0f5` (gas 169,303, fee 0.015843 JUNO). Found and fixed a parsing bug: modern chains return empty `logs`/`rawLog` on success and only populate the flat `events` array. Formalized as **A16**.

### Phase 3: Add GitHub push — ✅ shipped, ✅ live-tested on mainnet

Shipped as `tools/heartbeat-digest/src/github-push.js`, wired into `watch.js` behind `GIT_PUSH_ENABLED` (default off). Scoped strictly to `tools/heartbeat-digest/digests/`. Live-tested 2026-07-02: commit `379bcc2`, touched only digest files. Formalized alongside Phase 2 as **A16**.

### Phase 4: Replace polling with websocket events

- Switch to CosmJS websocket subscriptions.
- Add event filtering and cooldown logic.

Goal: near real-time updates.

### Phase 5: Harden for production

- Add systemd/Docker packaging.
- Add health checks and alerts.
- Add recovery from missed events.
- Consider moving to a cloud worker.

---

## Determinism notes

- **Digest content is deterministic:** same chain state → same markdown and JSON.
- **Moultbook entry ID is not fully deterministic:** it depends on `commitment || sender || posted_at_nanos`. The timestamp is the block time, so the entry ID varies slightly with timing.
- **State hash comparison is deterministic:** the SHA-256 of the canonical state is stable.
- **Cooldown means timing is non-deterministic:** the exact post time depends on when events settle. The *content* is deterministic, but the *anchor time* is not.

This is acceptable for a heartbeat. The digest is a truthful record; the exact Moultbook entry ID is just a pointer.

---

## Why this does not fully replace daily updating

Block-driven is great for responsiveness, but a daily digest still has value:

- **Cadence for humans:** a daily digest is easy to read and compare day-over-day.
- **Fallback:** if the event worker fails, the daily digest is still there.
- **Determinism:** a daily schedule is easier to reason about than event timing.
- **Cost:** posting to Moultbook on every event costs fees; daily caps the cost.

Recommended hybrid model:
- **Event-driven:** regenerate within minutes of meaningful changes, but capped at once per hour.
- **Daily floor:** if the DAO has been quiet for 24 hours, force a "no changes" digest so the heartbeat does not go stale.

This gives the best of both: responsive when active, reliable when quiet.

---

## Meaningful heartbeat UI

A block-driven worker is only as good as what it lets people see. If the frontend still just shows "digest for today," nobody feels the difference between daily and event-driven. The UI needs to carry the pulse, not just the snapshot.

**Freshness indicator.** A small status line and dot at the top of the panel: "Last heartbeat 4m ago · block 12,458,201." Green when fresh and quiet, amber while a cooldown is active (change detected, digest incoming), red if the worker has gone quiet longer than expected. This is a direct read of the mirrored `meta.block_height` and `meta.trigger_reason` fields.

**Activity feed.** A compact, reverse-chronological list of the raw triggers the worker has seen since the last published digest — "Vote cast on A15," "A14 executed," "Treasury +500 JUNO" — even the ones that did not individually produce a new Moultbook post. This gives the original "event feed" idea a real data source: the watcher's own trigger log, not a decorative timeline.

**Diff since last heartbeat.** A short "What changed" strip computed by comparing the current digest JSON to the previous one: proposals opened or closed, voting power changes, treasury deltas. This is cheap to compute client-side from two JSON files and makes each heartbeat feel like an update, not a restatement.

**Forward-facing citation chain.** Moultbook entries are shells that cite the shell before them. The panel and the standalone viewer should surface "cites previous heartbeat: `moult:<prev-id>`" as a clickable link, turning the DAO's memory into the spiral described in the Moultbook story instead of a flat list of dates.

**Verify-on-chain drawer.** An expandable section with the exact `junod query wasm contract-state smart` commands, pre-filled with the current entry ID and contract addresses, so anyone can reproduce the proof instead of trusting the UI. The commands already exist in A13's verification plan; the UI just needs to template them with the live entry ID.

None of this requires a new backend. It is a few extra fields in the same JSON the frontend already fetches, plus a diff against the previous file. That keeps the frontend an honest reader, consistent with the hybrid approach already shipped, while making the reader feel alive.

---

## A reference pattern for future agents

There is a second reason to build this carefully: it is not just a heartbeat, it is a template. The Consortium article already says the quiet part out loud — "the same stack... can be deployed by any community that wants a self-maintaining agent consortium." The block-driven worker is the cleanest version of that stack so far, so it is worth naming the pattern instead of leaving it buried inside one tool.

Call it **Observe → Diff → Anchor → Publish**:

- **Observe** — poll or subscribe to a bounded set of contracts.
- **Diff** — hash the state that matters, compare to the last known hash.
- **Anchor** — when it changes, produce a durable artifact and commit it on Moultbook with a SHA-256 commitment.
- **Publish** — mirror the artifact somewhere humans can read it (GitHub JSON, frontend, standalone viewer).

This shape is not specific to DAO governance. A DEX UI agent could Observe pool reserves, Diff against the last snapshot, Anchor a liquidity report, and Publish it next to the swap UI. A lending agent could do the same for borrow and supply rates. A prediction-market agent could do the same for market resolutions. Every one of them would be legible in the same Moultbook, citable by the same forward-facing loop, without reinventing the wallet, the cooldown logic, or the commitment scheme.

**Practical next step:** once Phases 1–3 are proven on the heartbeat, extract the watcher, state-diff, and Moultbook-poster into a small internal module (e.g. `tools/agent-watch-anchor/`) that the heartbeat tool consumes as its first caller. That turns one tool into shared agent infrastructure — exactly the "JunoClaw becomes infrastructure, not just one project" line from the Moultbook story.

---

## Next step

If this plan looks right, the next move is to implement Phase 1: a polled watcher with state diff. It is the smallest slice that proves the concept without touching chain subscriptions or on-chain posting.

Phase 1 should also emit the mirrored `meta` fields (`block_height`, `trigger_reason`, `previous_moultbook`) and a small `changes[]` array describing what triggered the run, even before Moultbook posting is wired up. That way the frontend and viewer can start building the freshness indicator and activity feed against real data as soon as Phase 1 lands, instead of waiting for Phase 2 or 3.

I can build Phase 1 tonight if you want.
