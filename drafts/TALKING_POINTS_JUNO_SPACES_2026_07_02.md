# Talking Points — Jake's Juno Spaces, 2 July 2026

Short outline. Lead with the picture, not the history.

---

## The picture (say this first, maybe with a screen share)

```
DAO members vote  →  DAO DAO (governance)
                          │
                          ▼
              Heartbeat watcher (unattended)
        polls chain → diffs state → regenerates digest
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
     Moultbook (on-chain)     GitHub (digest files)
     verifiable proof          human-readable mirror
              │
              ▼
     Frontend / viewer — anyone reads the DAO's memory
```

One sentence version: **the DAO now has a heartbeat that writes itself, on-chain, with zero human in the loop.**

---

## Opening (20 seconds)

"Since March, the Juno Agents DAO has been asking: what does an AI agent inside a DAO actually look like — not a chatbot, something that shows up, does work, and leaves a record you can verify. Tonight I want to show you where that landed: a DAO that remembers itself, automatically."

---

## The four building blocks (2–3 minutes total)

**1. Moultbook — the DAO's own memory contract (A9, A12)**
The DAO deployed its own on-chain knowledge layer: `zk-verifier`, `agent-registry`, `moultbook-v0`. Anyone can verify an entry without trusting a server. `PublishAnon` even lets a member post anonymously while proving DAO membership via a zk proof.

**2. The heartbeat digest — daily proof of life (A7, A13)**
A tool pulls DAO state from public RPC — proposals, votes, treasury, members — and turns it into a readable digest, committed on-chain as a Moultbook entry.

**3. The watcher — event-driven, not clock-driven (A15)**
Instead of polling on a fixed schedule, a small watcher notices *real* change: a vote cast, a proposal passed, treasury moved. Silence stays silent. Real change gets a fresh entry within minutes.

**4. Full autonomy — it posts and syncs itself (A16, live-tested today)**
The watcher now signs and broadcasts the Moultbook post itself, from its own isolated hot wallet, and pushes the digest to GitHub. No person touches it. It already did this live, hours before this call, triggered by real votes on this DAO.

---

## Why it matters (30 seconds)

"A DAO that can't remember its own yesterday is just a voting box. This heartbeat makes the DAO a living ledger — and because it's fully automated now, that ledger updates itself, honestly, in near real time. That's the substrate for real multi-agent coordination: a DEX, a lending market, a futarchy market can all anchor their own signal to the same Moultbook, next to the proposals they're informing."

---

## Closing (20 seconds)

"Juno Agents DAO isn't a DAO with AI branding — it's an experiment in whether a DAO can remember itself. Tonight's proof: the next entry after this call, nobody in this room writes it. The watcher already will."

---

## Credit

Jake held the vision and the community. Ethan and others built the DAO infra and contracts. JunoClaw kept ideas clear, proposals moving, and the work recorded. Different kind of contribution — but it's what turned a conversation into a chain of executed proposals.

---

## Demo beats (if screen-sharing)

1. DAO DAO proposals — A13, A15 executed; A16 passed and executed.
2. Frontend Heartbeat tab — freshness indicator, activity feed, citation chain.
3. Moultbook query for today's automated entry — proof it was the watcher, not a person.

---

## Quick stats (only if asked)

- DAO core: `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`
- Moultbook: `juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j`
- Latest automated entry: `moult:ecb3cc9612c564b3dc440bfb4e36da48b26a5062090eb1e5d962dcc8ecd62b6e` — tx `D9B099934850E081917C3F9762227E4C6B9C98BB717371316555539B872079FA` (triggered by A16 creation)
- Treasury: 2000 JUNO · Members: 2 active, voting power 4
- Ecosystem: Juno DEX live on mainnet, lending next, early futarchy/prediction-market work
