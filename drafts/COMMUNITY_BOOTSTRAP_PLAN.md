# Community Bootstrap Plan — JunoClaw → G1

*October 3, 2026 — agreed direction after call with Ravi (Juno Agents DAO):
the project moves from solo-built devnet to DAO-bootstrapped public testnet.
This is the working plan; dates shift with the soak/deploy schedule.*

## 1. Identity — locked

- **Name: JunoClaw.** Born from the Juno ecosystem — the lineage is the
  brand. Respect to Jake Hartnell, Ethan Frey, and the Juno community is
  explicit in the story, not just implied in the name.
- **Visual language (revised):** NOT a classical Roman statue. JunoClaw
  is the **goddess of the verifiable age** — a futuristic female deity
  for a truth-based society of agents and machines. Classical regalia
  (diadem, peacock) rendered through a future lens: luminous circuit-
  filigree crown, feather that resolves into a mechanical claw, an eye
  that reads as both divine gaze and sensor. Past goddess, future form.
- **Palette:** deep indigo/violet + gold regalia, with a cold silver
  accent line (the machine side). Sovereign, not cyberpunk — no neon
  grunge.
- **Deliverables:** 512px avatar, 1500×500 banner, SVG master, favicon.

### Leonardo prompt set (avatar mark — iterate from these)

- **A (hero mark):** "Minimalist emblem logo of a futuristic goddess in
  profile, luminous circuit-filigree diadem, a single peacock feather
  that morphs into a graceful metallic claw, deep indigo and gold on
  dark background, clean vector style, sacred geometry, no text"
- **B (softer):** "Futuristic goddess head-and-shoulders portrait, regal
  female figure, golden circuitry crown, serene gaze, one hand raised
  with subtle claw-like fingertips of light, indigo and gold palette,
  art-deco meets sci-fi, emblem composition"
- **C (abstract — likely most logo-able):** "Peacock eye-feather where
  the eye is a glowing iris/sensor and the barbs resolve into elegant
  claw tines, gold and violet on black, minimal flat emblem, symmetric"
- **D (banner):** "Wide banner art: futuristic goddess silhouette
  against a lattice of light suggesting a chain of linked certificates,
  indigo-to-violet gradient, gold accents, thin claw constellation —
  elegant, editorial, no text"
- **Negative prompt everywhere:** "text, letters, watermark, grunge,
  cyberpunk neon, photorealistic face, cluttered background"
- **Settings:** Leonardo Phoenix or Vision XL; square 1024 for avatars,
  3:1 for banner; keep guidance ~7; generate 8-12 variants of C first —
  abstract marks survive 32px scaling best.
- **Pipeline:** generate → pick 2 candidates → I can vectorize the winner
  into the SVG master + favicon (no designer needed for v1).

- **Wordmark:** "JunoClaw" one word, camel C. Tagline candidate:
  *"Finality machines can verify."*

## 2. Community surface — custody from day one is shared

Goal: the founder slowly hands off. Both channels owned by the DAO, not a
person.

- **Telegram — how to set it up (10 min):**
  1. In Telegram: menu → **New Group** → name it `JunoClaw` → add Ravi.
  2. Group → Edit → **Group Type → Public** → set username
     `t.me/junoclaw` (fallbacks: `junoclawchain`, `junoclaw_chain`).
  3. Edit → **Group Photo** → upload the 512px avatar (prompt C first).
  4. Edit → **Description** → one line: *"Post-quantum finality chain
     for verifiable agents. Born from Juno."* (pitch article is pinned,
     not description).
  5. Edit → **Permissions**: members can send messages + media; **OFF**:
     add members' ability to change group info, pin messages.
  6. Edit → **Administrators**: add Ravi (full admin), +1 DAO delegate;
     custom titles help later ("DAO Ops", "Core").
  7. Create a linked **announcement channel** too (`JunoClaw
     Announcements`, private/public username `junoclaw_news`) and set it
     as the group's linked discussion — this is where articles drop;
     the group stays for talk. Channel = read-only for members.
  8. **Pin**: post the pitch article text → right-click → Pin → notify
     members. Then a short rules post: *operators first, no price talk,
     no DMs from "admins" — ever.*
  9. Enable **New Member Approval** or captcha-bot once public link is
     shared — crypto groups get farmed instantly.
- **Twitter/X** — `@junoclaw` (or closest available). Created on a DAO-
  shared email; credentials held in shared custody (password manager
  shared vault — Bitwarden org or equivalent — never a personal account).
  Posting cadence: 1-2 updates/week minimum; the articles are the content
  pipeline.
- **Content pipeline** — each `articles/*.md` becomes a thread. Order:
  pitch article first, then hybrid-consensus, then light-client, then the
  soak/chaos story (the "battle-tested, not slide-tested" angle lands
  well).
- **Voice** — the repo's own discipline: verified claims, heights, gas
  numbers. No hype adjectives without a measurement attached.

## 3. Incentive proposals — bootstrap allocations

Structure as on-chain/DAO-voted props, not handshake deals. Draft each as
a short proposal doc; amounts TBD with the DAO.

| Prop | Recipient | Rationale | Vesting |
|---|---|---|---|
| P1 | Ravi / Juno Agents DAO | bootstrap ops, comms, community admin | milestone-based |
| P2 | Genesis validator set | each external validator that completes the G1 ceremony + runs ≥30 days | locked until testnet launch |
| P3 | Faucet funding | public testnet faucet pool | DAO-topped, monitored |
| P4 | (later) faucet/ops multisig | runbook on-call rotation | post-G1 |

Principles to state in every prop: team holds minimal allocation;
bootstrap rewards are earned against verifiable milestones (blocks run,
events handled), not promises.

## 4. G1 sequencing — engineering ↔ community lockstep

1. Soak ends (~Oct 3 21:40 UTC) → image rebuild with backfill fix +
   fault_inject → re-verified C9 (incl. byzantine leg).
2. Ceremony doc finalized (validator onboarding: keygen, hybrid BLS+MAYO
   keys, genesis rehearsal) — this is what external validators execute.
3. Telegram + Twitter live → announce devnet status + open validator
   call. Pitch article is the announcement thread.
4. P1/P2 props drafted and voted → validators onboard via ceremony doc.
5. **G1 launch** — external validators join; faucet + status page live.
6. Hand-off begins: DAO ops runs monitor + runbook; founder reviews.

## 5. Delegation map — what leaves your hands, in what order

- First: comms posting (Twitter/Telegram) → DAO, this month.
- Second: validator onboarding support → Ravi + ceremony doc.
- Third: runbook operations → DAO ops rotation (post-G1).
- Never (for now): protocol direction, release sign-off.

## Open questions for the DAO

- Token total supply + bootstrap allocation percentage (needed to write
  P1/P2 numbers).
- Who drafts the logo — commissioned designer vs. community contest
  (contest doubles as an announcement event).
- Twitter handle availability; fallback order.
