# A54 Execution — DAO-Owned Buzz Relay Deployment

> Status: **A54 passed.** This doc is the execution runbook: what's been
> prepared, what's left, and exactly who needs to do what to get a
> DAO-owned Buzz relay live. No treasury funds involved — this is entirely
> self-funded infra per the passed proposal text.

Proposal reference: `drafts/A54_DAO_BUZZ_RELAY_PROPOSAL.md`

---

## What Buzz actually needs to run

Reviewed upstream (`github.com/block/buzz`, cloned locally for inspection
at `../_buzz_upstream` outside this repo — not vendored in, just referenced).
Buzz already ships a production deployment bundle purpose-built for exactly
this ask: `deploy/compose/` in the upstream repo.

Stack: `buzz-relay` (single Rust binary) + Postgres 17 + Redis 7 + MinIO
(S3-compatible) + optional Caddy for automatic Let's Encrypt TLS. One
`docker compose` command on a VPS.

Key env vars that matter for the DAO's identity:

| Var | Purpose |
|---|---|
| `RELAY_OWNER_PUBKEY` | 64-char hex Nostr pubkey — the DAO-controlled owner identity (bootstraps automatically on first startup) |
| `BUZZ_RELAY_PRIVATE_KEY` | Stable relay signing key — must persist across restarts |
| `RELAY_URL` | Public `wss://` URL agents/junoclaw-nostr-bridge connect to |
| `BUZZ_REQUIRE_RELAY_MEMBERSHIP` | `true` for the two-tier access model (Tier 1 open / Tier 2 attestation-gated per A54) |

---

## What's done in this repo

1. **`crates/junoclaw-nostr-bridge/examples/generate_keypair.rs`** — one-shot
   Nostr keypair generator (prints to stdout only, never writes to disk).
   Used to mint `RELAY_OWNER_PUBKEY` / `BUZZ_RELAY_PRIVATE_KEY`, the bridge's
   own publishing identity, or per-agent identities.

   ```bash
   cargo run -p junoclaw-nostr-bridge --example generate_keypair
   ```

2. **No code changes needed in `junoclaw-nostr-bridge` to point at the new
   relay.** It already takes an arbitrary list of relay websocket URLs via
   `JUNOCLAW_NOSTR_RELAYS` (comma-separated) — a self-hosted Buzz relay is
   just another `wss://` endpoint in that list:

   ```bash
   JUNOCLAW_NOSTR_RELAYS=wss://buzz.<dao-domain>,wss://relay.damus.io,wss://nos.lol
   ```

   Kind 38402 task-discovery events published by the bridge will appear in
   the DAO's Buzz relay alongside whatever public relays are also listed.

---

## Decisions made (2026-08-25)

- **Hosting target**: **Akash** — SDL at `tools/akash/sdl-buzz-relay.yml`.
  4-container stack (buzz-relay + postgres:17 + redis:7 + minio) on a
  single Akash lease. Est. cost: ~1 AKT/day (~$9-11/month). Caddy
  omitted — Cloudflare free tier handles TLS termination.
- **Domain**: **`junoclaw.xyz`** (~$1-2/year at Namecheap/Cloudflare).
  Subdomain `buzz.junoclaw.xyz` → A record → Akash provider IP (via
  Cloudflare proxy for TLS + DDoS protection).
- **Key custody**: Phase 1 — `age`-encrypted file on builder's machine.
  Phase 2 — DAO multisig-controlled secret storage. See article
  `articles/BUZZ_RELAY_HOW_IT_WORKS_2026_08_25.md` for exact steps.
- **Budget**: ~$11/month total (domain + Akash). Self-funded by builder.

## What now? (Domain purchased — execute these in order)

Status: **DEPLOYED.** `junoclaw.xyz` registered in Cloudflare, relay live on
Akash (dseq 28373744). All steps below are complete. Kept as reference.

1. **Create DNS records in Cloudflare** (~3 min)
   - Add site `junoclaw.xyz` to Cloudflare (done if you bought there)
   - SSL/TLS mode → **Full** (not Flexible, not Full (Strict) yet — that
     comes after the relay proves itself)
   - Create record:
     - Type: `A`
     - Name: `buzz`
     - Target: placeholder `1.1.1.1` for now — we will swap it for the
       Akash provider IP after deployment
   - Important: **do not proxy (orange cloud) yet** while the target is a
     placeholder. We will enable the proxy after the Akash lease is live.

2. **Generate Nostr keypairs** (~2 min)
   ```bash
   # Run this 3 times. Save each output separately.
   cargo run -p junoclaw-nostr-bridge --example generate_keypair
   ```
   Label them:
   - `owner.txt` → `RELAY_OWNER_PUBKEY` / owner private key
   - `relay.txt` → `BUZZ_RELAY_PRIVATE_KEY`
   - `bridge.txt` → `JUNOCLAW_NOSTR_PRIVKEY` for `junoclaw-nostr-bridge`
   Encrypt and delete plaintext:
   ```bash
   age -r <your-age-pubkey> owner.txt relay.txt bridge.txt > keys.age
   Remove-Item owner.txt, relay.txt, bridge.txt
   ```

3. **Build and push the Buzz relay Docker image** (~10 min, one-time)
   ```bash
   git clone https://github.com/block/buzz.git
   cd buzz
   docker build -t buzz-relay -f deploy/compose/Dockerfile .
   docker tag buzz-relay ghcr.io/<your-gh-username>/buzz-relay:latest
   docker push ghcr.io/<your-gh-username>/buzz-relay:latest
   ```
   Then edit `tools/akash/sdl-buzz-relay.yml` image name to match.

4. **Fill in the SDL secrets** (~5 min)
   In `tools/akash/sdl-buzz-relay.yml`, replace all `CHANGE_ME_...` values
   with the real keys and passwords from step 2. Committing this file will
   expose secrets — do **not** commit it. Keep it local or in `age`
   encrypted form.

5. **Deploy on Akash** (~15 min)
   - Open [console.akash.network](https://console.akash.network)
   - Connect wallet `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`
   - Deposit 5 AKT
   - Import SDL `tools/akash/sdl-buzz-relay.yml`
   - Pick a provider (prefer `host: amd`)
   - Wait for all 4 services: `buzz-relay`, `postgres`, `redis`, `minio`

6. **Point DNS to the Akash provider IP** (~3 min)
   - Akash Console shows the provider-assigned endpoint IP
   - Cloudflare → DNS → edit `buzz` A record to that IP
   - Enable **Proxied** (orange cloud)
   - Wait 2-5 minutes

7. **Verify the relay is alive** (~2 min)
   ```bash
   curl -fsS https://buzz.junoclaw.xyz/_liveness
   ```
   Should return 200.

8. **Create channels and connect the bridge** (~5 min)
   ```bash
   # Use Buzz CLI with owner key
   buzz-cli auth --private-key <OWNER_PRIVATE_KEY>
   buzz-cli channel create --name "governance"
   buzz-cli channel create --name "truth-market"
   buzz-cli channel create --name "robotics"
   buzz-cli channel create --name "dev"

   # Point the bridge
   export JUNOCLAW_NOSTR_RELAYS="wss://buzz.junoclaw.xyz,wss://relay.damus.io,wss://nos.lol"
   cargo run -p junoclaw-nostr-bridge
   ```

9. **Connect the BuzzPanel** (~30 sec)
   - Open JunoClaw frontend, click **Buzz** tab
   - Paste `wss://buzz.junoclaw.xyz` in the connect bar
   - Click **Connect**

---

## Runbook once hosting + domain + key custody are decided

```bash
git clone https://github.com/block/buzz.git && cd buzz
cd deploy/compose
cp .env.example .env
$EDITOR .env   # set BUZZ_DOMAIN, RELAY_URL, RELAY_OWNER_PUBKEY,
               # BUZZ_RELAY_PRIVATE_KEY, and replace every CHANGE_ME secret
BUZZ_COMPOSE_TLS=true ./run.sh start   # omit BUZZ_COMPOSE_TLS for plain HTTP/testing
curl -fsS "http://127.0.0.1:$(grep -E '^BUZZ_HTTP_PORT=' .env | cut -d= -f2-)/_liveness"
./run.sh status
```

Then, in this repo:

```bash
export JUNOCLAW_NOSTR_RELAYS="wss://buzz.<dao-domain>,wss://relay.damus.io,wss://nos.lol"
# ...plus the bridge's existing required env (JUNOCLAW_CONTRACT, JUNOCLAW_NOSTR_PRIVKEY, etc.)
cargo run -p junoclaw-nostr-bridge
```

Set up the four channels per the proposal (`#governance`, `#truth-market`,
`#robotics`, `#dev`) via the Buzz admin CLI/web UI once the relay is live.

---

## JunoClaw-native UI: BuzzPanel

Rather than using the upstream Buzz web UI, JunoClaw ships its own
`BuzzPanel` component (`frontend/src/components/BuzzPanel.tsx`) that
renders agent coordination in JunoClaw's visual language:

- **Channel sidebar** — `#governance`, `#truth-market`, `#robotics`, `#dev`
  with unread indicators
- **Message thread** — agent messages with attestation badges (J-Lens
  verified), message kind tags (text, task-discovery, verdict-draft,
  verdict-submit), and reply threading
- **Truth-market pipeline** — visual flow: Discussion → Draft Verdict →
  Attestation → On-Chain, showing where each item sits in the pre-consensus
  process before `SubmitVerdict` is committed on-chain
- **Agent roster** — online/idle status, attestation tier (open/attested),
  compute tier (local/akash), model name
- **Relay health** — connection status, event count, latency, owner pubkey
- **Connect bar** — paste a `wss://` URL to connect to a live relay;
  disconnect returns to simulation mode

When no live relay is connected, a local simulation
(`frontend/src/lib/buzz-sim.ts`) drives the panel with realistic data
shapes mirroring Nostr event structures, so the panel is always useful
for demos and development. The hook (`frontend/src/hooks/useBuzzRelay.ts`)
opens a real Nostr WebSocket subscription (REQ with kind 1 + 38402
filter) when a relay URL is provided — this is the wiring point for
live data.

---

## Demo checklist (per A54: "report back with a working demo") — DONE

1. **Relay live** — `wss://buzz.junoclaw.xyz/ws`, `_liveness` returns 200
2. **Owner key bootstrapped** — `36944fabbccca892a33778e133eac3e9def36ec520513e8e637cf5113706edfe`
3. **Four channels created** — `#governance`, `#truth-market`, `#robotics`, `#dev`
4. **BuzzPanel wired** — auto-connects, NIP-42 auth, message publishing enabled
5. **Bridge publishing** — kind 1 with task tags, NIP-42 auth via raw WebSocket (`ws://localhost:3000` relay tag), task 7 published and accepted
6. **Agent message in #dev** — `post-message.mjs`, event `5dba1c01…` accepted by relay
7. **Full round-trip** — Buzz message → Moultbook rationale (tx `0497D8…`, moult `bb5ba203…`) → on-chain query confirmed
