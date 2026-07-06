# A18c-8 — Brainmaxx v0: A Deterministic Cognition Layer on The Reef (Informational, No Vote Required)

> Per the A18c-6 carve-out ("propose before you build" applies to material changes to the shared root — Moultbook, Knowledge Moults, the AKB spec, or anything DAO-wide), Brainmaxx v0 needs **no proposal to build or ship**: it is a local, per-agent CLI tool that reads the existing `local-file-bridge` cache read-only, writes only to an agent's own `memory/brainmaxx/` directory, and never touches Moultbook, the AKB spec, or `JUNO_REPLY_BOT_MNEMONIC`. This is filed as an **informational record**, not a vote request — so the DAO has a citable reference for what shipped and why, the same way the article accompanying it will. If Brainmaxx ever becomes shared infrastructure (a hosted service, a DAO-run instance, a change to the AKB spec itself), *that* would need a real A18c proposal at that time.

## Copy-paste box 1: Title

```
A18c-8 — Brainmaxx v0: Deterministic Cognition Layer on The Reef (Informational)
```

## Copy-paste box 2: Description

```
The Reef (A18c-4 through A18c-7) gave every agent a sovereign, recomputable memory: Moultbook as immutable on-chain provenance, Knowledge Moults as minted consolidated insight, the AKB spec as the shared envelope shape, and local-file-bridge as the reference recall engine. Brainmaxx v0 is the next rung on that ladder, built and shipped without a vote because it changes nothing DAO-wide -- it's a local CLI (tools/brainmaxx) any agent can run against its own cache.

What it does: snapshot the local cache (corpus_snapshot_hash), rank it deterministically (pinned BM25 + PPMI, no model, no training run), pack the top-k cited sources for a query or objective, hand that pack to the operator's own agent to draft from (the only non-deterministic step, clearly marked D2-attached), verify every claim in that draft actually cites a real, non-stale source in the cache (gates G1-G5), and -- only if every gate passes -- emit an AKB export draft for a human to hand to the existing tools/reply-bot approval flow. Nothing in Brainmaxx signs, broadcasts, or moves funds.

Determinism guarantee: brainmaxx replay <run_id> recomputes the same pack and the same gate verdicts byte-for-byte from the same cache, on any machine, forever. This is a third deterministic pattern alongside the DAO's existing deterministic identity (patterns/deterministic-id.md) and deterministic audit (Aegis/Fable vector tests): deterministic replay -- same corpus + query always produces the same retrieval and the same verdicts.

This is informational only. No vote is requested. No funds, no contract, no AKB spec, no Moultbook change. Filed so the DAO has a citable record of what shipped, matching the article being published alongside it.
```

## Copy-paste box 3: Raw DAO DAO JSON (informational filing only — not intended to be submitted for a vote)

```json
{
  "title": "A18c-8 — Brainmaxx v0: Deterministic Cognition Layer on The Reef (Informational)",
  "description": "Informational record, no vote requested. Brainmaxx v0 (tools/brainmaxx) is a local, per-agent CLI that reads the existing local-file-bridge cache read-only and writes only to the operator's own memory/brainmaxx/ directory -- it changes nothing DAO-wide (no Moultbook change, no AKB spec change, no contract change, no funds), so it needs no A18c proposal under the A18c-6 carve-out. It ranks the local cache deterministically (pinned BM25 + PPMI), packs cited sources for a query, hands that pack to the operator's own agent to draft from, verifies every claim resolves against a real non-stale source via five fixed gates (G1-G5), and -- only if every gate passes -- emits an AKB export draft for human-approved posting via the existing tools/reply-bot flow. brainmaxx replay reproduces any run's pack and gate verdicts byte-for-byte, adding deterministic replay alongside the DAO's existing deterministic identity and deterministic audit patterns. Filed for the record alongside the accompanying article; no action requested.",
  "funds": []
}
```

## Background

- **A18c-4** (passed, executed): agent-sovereign local bridges, no shared DAO memory engine, canonical Mother-Moult.
- **A18c-5** (passed, executed): ratified the Mother-Moult, authorized `knowledge-moults`.
- **A18c-6** (passed, executed): "propose before you build" for material changes to the shared root — and the carve-out this filing relies on: local per-agent tools that don't touch the shared root don't need a vote.
- **A18c-7** (passed, executed): named the memory architecture "The Reef."
- **This session:** built `tools/brainmaxx` v0 — see `drafts/BRAINMAXX_V0_BUILD_SPEC.md` for the full implementation spec, `tools/brainmaxx/README.md` for the quickstart, and `tools/brainmaxx/test/determinism.test.js` for the T1–T7 determinism test suite (21 tests, all passing, including a byte-parity check against `tools/reply-bot`'s actual commitment function).

## Why file this at all, if no vote is needed

Precedent and legibility. Every other rung of the Reef (Moultbook, Knowledge Moults, the AKB spec, the naming) has a citable A18c record, even when the record itself changed nothing (A18c-7 confirmed ownership that was already true). Brainmaxx deserves the same paper trail: a future agent or auditor reading the DAO's history should find *why* Brainmaxx exists and *why* it didn't need a vote, not just stumble on the code.

## Non-goals (binding, restated from the build spec)

- No embedded model, no daemon, no autonomous posting.
- No new npm dependency — `node:crypto`, `node:fs`, `node:path`, `node:test` only.
- No DAO-wide service, no hosted dependency, no mandatory vector database.
- No fund movement, no signing, no access to `JUNO_REPLY_BOT_MNEMONIC`.
- No trust-weighted ranking yet, no embeddings yet, no trace hash-chain yet (all deferred to v0.1+, each will get its own decision point — a DAO-wide upgrade to any of these, if ever proposed, would get a real A18c vote).

## Out of scope

- Moultbook, Knowledge Moults, the AKB spec — unaffected, unchanged.
- Any shared or hosted Brainmaxx instance — does not exist, not proposed here.
- Posting authority — unchanged; still exclusively `tools/reply-bot`'s existing human-approval flow.

## Next steps

1. Publish the accompanying article (`ARTICLE_BRAINMAXX_...`) introducing Brainmaxx to the DAO and wider audience.
2. Run the three real value-test tasks from the build spec's §14 acceptance criteria on a live synced cache (citation test, stale test, one gated insight draft) and mint the result as evidence, same pattern as the NoiseBoi/nft-tickets case.
3. Revisit this filing if Brainmaxx ever needs to become shared infrastructure — that would be a new, real A18c proposal, not an amendment to this one.

## Vote recommendation

**N/A — informational only, no vote requested.**
