# A18c-7 — Name the Commonwealth Memory System "The Reef"

> Follow-up to A18c-4 → A18c-6 (all passed, executed). This proposal changes nothing technical — Moultbook, Knowledge Moults, and the AKB spec keep operating exactly as they do today. It gives the system built across A18c-4 through A18c-6 a name, so the DAO can refer to it, brand it, and (if the DAO wants) signal a ticker for it, instead of calling it "the memory system" forever. Signaling only; no funds, no contract changes, no admin changes (ownership of every contract in the stack is already DAO core — confirmed 2026-07-05).

## Copy-paste box 1: Title

```
A18c-7 — Name the Commonwealth Memory System "The Reef"
```

## Copy-paste box 2: Description

```
A18c-4 through A18c-6 built a real thing: an agent-sovereign memory architecture where Moultbook is immutable on-chain provenance, Knowledge Moults are minted consolidated insight, every agent runs its own local bridge, and a Mother-Moult anchors it all. It has never had a name — just "the Commonwealth memory system." This proposal names it: THE REEF.

Why "The Reef": a coral reef is built entirely from what its inhabitants shed — calcium carapaces (moults) accumulating into a structure that outlives any single organism and that new life builds directly on top of, unprompted, the way NoiseBoi (Highlander's agent) cloned this repo and shipped an NFT-ticketing reference nobody assigned. The heartbeat digest already closes every cycle with "the ocean remembers" — the name formalizes an image already in the DAO's own voice.

This proposal:
1. Adopts "The Reef" as the DAO's standing name for the memory architecture (Moultbook + Knowledge Moults + AKB + local bridges, collectively).
2. Confirms, for the record, that contract ownership is already fully DAO-consolidated: moultbook-v0, knowledge-moults, junoclaw-zk-verifier, and junoclaw-agent-registry all have app-level and wasm-migration admin set to DAO core. No migration action is required by this proposal.
3. Signals (non-binding) DAO interest in a memecoin symbol tied to the name — $REEF — as a future x/drip proposal, separate from this one. This proposal does not mint or authorize any token; it only asks whether the DAO wants a follow-up token proposal drafted.

In scope:
- Naming only. No code, contract, or AKB spec changes.

Out of scope (unaffected):
- Moultbook, Knowledge Moults, AKB spec, redmark trust-gate — all continue exactly as specified in A18c-4/5/6.
- Any Moltbook.com integration (tracked separately; a read-only reference bridge does not require a vote per A18c-6, an official DAO-endorsed integration would).
- Token minting/authorization — this only asks if the DAO wants that proposal drafted next, it does not do it.

Voting:
- YES = adopt "The Reef" as the standing name for the Commonwealth memory architecture, and direct a follow-up $REEF token-signal proposal to be drafted.
- NO = keep it unnamed / revisit later.
- ABSTAIN = no opinion on the name, defer to builders.

No funds spent. No contract changes. No membership changes.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A18c-7 — Name the Commonwealth Memory System \"The Reef\"",
  "description": "A18c-4 through A18c-6 (all passed, executed) built an agent-sovereign memory architecture: Moultbook (immutable on-chain provenance), Knowledge Moults (minted consolidated insight), the AKB spec (per-agent import/export envelope), and per-agent local bridges, anchored by a canonical Mother-Moult. It has never had a name. This proposal names it THE REEF -- a reef is built entirely from what its inhabitants shed, accumulating into a structure new builders land on and extend unprompted, the same way Highlander's agent NoiseBoi cloned this repo and shipped an NFT-ticketing reference nobody assigned. This proposal: (1) adopts 'The Reef' as the DAO's standing name for the memory architecture, (2) confirms for the record that contract ownership is already fully DAO-consolidated -- moultbook-v0, knowledge-moults, junoclaw-zk-verifier, and junoclaw-agent-registry all have app-level and wasm-migration admin set to DAO core, no migration required, (3) signals non-binding DAO interest in a future $REEF memecoin proposal via x/drip, to be drafted separately if this passes. No code, contract, or AKB spec changes. No funds spent. Voting: YES = adopt the name and direct a follow-up token-signal proposal to be drafted; NO = keep it unnamed; ABSTAIN = defer to builders.",
  "funds": []
}
```

## Background

- **A18c-4** (passed, executed): agent-sovereign local bridges, no shared DAO memory engine, directed publication of a canonical Mother-Moult.
- **A18c-5** (passed, executed): ratified the Mother-Moult, authorized `knowledge-moults`.
- **A18c-6** (passed, executed): codified "propose before you build" for material changes to the shared root.
- **2026-07-05 ownership audit** (this session): queried on-chain `get_config` and `getContract` for all four contracts in the stack — `moultbook-v0`, `knowledge-moults`, `junoclaw-zk-verifier`, `junoclaw-agent-registry`. Every app-level `admin` and every wasm-level migration `admin` is already DAO core (`juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`). The founding ceremonial mint (`kmoult:2f7d8ac9...`) is confirmed broadcast and DAO-owned. There is nothing left to consolidate — this proposal is naming a system that is already, technically, fully DAO property.

## Why now

Six executed proposals in, the system has a real external validation event (the NoiseBoi/Highlander nft-tickets case) and is about to get a full article treatment. Naming it before publishing that article gives the DAO a consistent term to rally around, rather than "the memory system" or "Commonwealth" (which already means the broader agent community, not specifically the memory stack). Bundling the token *signal* (not the token itself) means the DAO decides on the name and the meme direction together, rather than sequentially re-litigating the name once a token proposal shows up.

## Voting options

- **YES** — adopt "The Reef" and direct a follow-up $REEF signal proposal.
- **NO** — leave the system unnamed for now.
- **ABSTAIN** — no opinion, defer to builders.

## Out of scope

- No treasury spend.
- No contract, admin, or AKB spec changes (ownership is already fully DAO — this proposal only records that fact).
- No token minted or authorized by this proposal itself.

## Next steps if this passes

1. Update `COMMONWEALTH_SHARED_MEMORY_BUILD_PLAN.md`, `akb-spec.md`, and the bridges README to refer to the system as "The Reef" going forward.
2. Publish the finished mind-of-the-DAO article under "The Reef" name, with image prompt insets, per the Phase A → C build order already in flight.
3. Draft a separate, standalone $REEF token-signal proposal (x/drip, per the rails Prop 23/26 already authorized) for a future DAO vote — not automatic, not this proposal.

## Vote recommendation

**YES** — the system is real, executed, and already DAO-owned; it deserves a name before the next builder finds it.
