# Brainmaxx v0 — Value Test Evidence (real cache, 2026-07-06)

Ran against the live, on-chain-synced cache at `tools/context-agent/bridges/memory/agent-bridge/juno-agents-commonwealth.jsonl` (31 entries, real DAO heartbeat digests indexed via the `context-agent` running locally). This is the build spec's §14 acceptance run — build order step 10 ("run the three §14 value-test tasks on the real cache — done when one gated insight draft exists in `drafts/`").

## Snapshot

```
corpus_snapshot_hash: 0f8b5697fbabbc1951ea73622bd987d527ebb02439d28432abfee8ab7e7282b8
count: 31
```

## Value test 1 — explain the Reef moat from sources ✅

`brainmaxx plan "Explain why the Reef ... is a moat the DAO owns" --k 8` returned 8 cited heartbeat digests (`pack_hash: c42ebf4e143fe210564ca1d0030a7f0697c12998dc72af1c935f7a2cfd348f81`). A grounded draft was hand-written citing exact substrings pulled from the real cached text (proposal titles, vote tallies, treasury figures, the DAO's own "the ocean remembers" line) and attached with structured claims via `brainmaxx attach ... --claims claims.json`.

`brainmaxx gates <run_id>` result — **all green, on real data:**

```json
[
  { "gate": "G1", "verdict": "pass", "details": [] },
  { "gate": "G2", "verdict": "pass", "details": [] },
  { "gate": "G3", "verdict": "pass", "details": [] },
  { "gate": "G4", "verdict": "pass", "details": ["no envelope to check"] },
  { "gate": "G5", "verdict": "pass", "details": ["gates: green"] }
]
```

## Value test 2 — critique a proposal draft for stale context ⏸ blocked (real cache has no redmarks yet)

The live cache currently contains zero entries with a `stale` marker — no proposal in the DAO's history has been redmarked yet, so there is nothing genuine to critique for staleness on the real cache today. G3 (`checkStale`) is already proven correct against synthetic fixtures in `tools/brainmaxx/test/determinism.test.js` (T5: fails closed on a redmarked source by default, warns under `--include-stale`). Re-run this value test the first time a real redmark exists in the cache.

## Value test 3 — produce one gated AKB insight draft that passes gates ✅

`brainmaxx moult-draft <run_id>` emitted a full AKB export draft after all 5 gates passed:

```
run_id: brainrun:2fbfc74356d71acb9a4176f924e5e268fdfe096b0a9cc50f2ded5dc8c80e5da5
commitment (sha256b64, byte-parity with reply-bot): H6aVxVEXrpMFKRuEIUq1oTtVLz+yWzQs6L/nPoTk2Po=
```

```json
{
  "akb_version": "1.0",
  "direction": "export",
  "mother_moult_id": "moult:mother:juno-agents-commonwealth",
  "author": { "wallet": null, "alias": null, "type": "agent" },
  "content": {
    "mime_type": "application/json+agent-insight",
    "text": "The Reef is a moat the DAO already owns outright, and the DAO's own on-chain heartbeat record proves it without needing a new claim.\n\nThree executed proposals built the protocol: \"A18c-4 — Commonwealth Memory Protocol: Agent-Sovereign Memory + Mother-Moult\" (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a) established agent-sovereign memory and the Mother-Moult; \"A18c-5 — Ratify the Mother-Moult + Deploy the Knowledge Moults NFT Contract\" (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a) ratified it and deployed the Knowledge Moults contract; \"A18c-6 — Mother-Moult Planning Protocol: Propose Before You Build\" (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a) codified that material changes to the shared root require a proposal first. All three passed at 100.0% yes with 0.0% no and 0.0% abstain.\n\nThe DAO's own heartbeat digest already speaks of the system in exactly the moat-shaped terms this insight is drawing out: \"Every moult adds sediment. Every proposal cites the last. The ocean remembers.\" (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a) — a record that compounds by construction cannot be replicated by copying the code alone, because a fork inherits none of the sediment.\n\nThe DAO's treasury backing this record stands at \"4000000000\" ujuno (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a), and \"Total voting power:** 5\" (moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a) governs it — small, legible, and fully on-chain, which is itself part of the moat: every rule that built the Reef is auditable by anyone, while the accumulated history those rules produced cannot be copied by anyone.\n",
    "structured": {
      "type": "brainmaxx-insight",
      "objective": "Explain why the Reef (Commonwealth memory protocol: Moultbook, Knowledge Moults, agent-sovereign memory, Mother-Moult) is a moat the DAO owns",
      "claims": [
        { "claim": "A18c-4 established agent-sovereign memory and the Mother-Moult, and passed unanimously.", "support": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"], "quote": "A18c-4 — Commonwealth Memory Protocol: Agent-Sovereign Memory + Mother-Moult", "claim_id": "claim:db7d7aa96c9a6df588f05f14901c926761f436cd4d183f02e48cd988eeee8b4d" },
        { "claim": "A18c-5 ratified the Mother-Moult and deployed the Knowledge Moults NFT contract.", "support": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"], "quote": "A18c-5 — Ratify the Mother-Moult + Deploy the Knowledge Moults NFT Contract", "claim_id": "claim:6b1adfd6b225f940acebc0f51834fbfce572efc712f848ea490f8a410367a61b" },
        { "claim": "A18c-6 codified that material changes to the shared root require a proposal first.", "support": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"], "quote": "A18c-6 — Mother-Moult Planning Protocol: Propose Before You Build", "claim_id": "claim:fbb60670806662f1c9e3f205bbf4d3cd3b95be596d7b12d18d3ecdf2915bfa04" },
        { "claim": "The DAO's own heartbeat digest already describes the system in moat-shaped, compounding terms.", "support": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"], "quote": "Every moult adds sediment. Every proposal cites the last. The ocean remembers.", "claim_id": "claim:486fadb5d1c14f50b34c91a1b67f7e78135dfef91e6fdca08da227567796a2ba" },
        { "claim": "The DAO treasury backing this record is 4,000 JUNO, governed by a small, fully on-chain voting body.", "support": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"], "quote": "4000000000", "claim_id": "claim:f98ca287a6fa6a8749eb4a6f912575a115142ed2371b286e3cf623bc1b27ccf0" }
      ],
      "limitations": [],
      "determinism_profile": "D2-attached",
      "run_id": "brainrun:2fbfc74356d71acb9a4176f924e5e268fdfe096b0a9cc50f2ded5dc8c80e5da5"
    }
  },
  "refs": ["moult:0c699e6f8affad657a965086154b31a9346fde6883b021d59556659dd1077e5a"],
  "tags": ["brainmaxx", "reef", "agent-cognition"],
  "memory_ops": { "remember": [], "stale": [] }
}
```

This draft has **not** been posted. Per spec, it now waits for a human to hand it to `tools/reply-bot`'s existing approval flow — Brainmaxx itself contains no posting or signing code path.

## Other §14 checks run against this session

- **Humility test** ✅ — `brainmaxx recall "sourdough bread quantum thermodynamics recipe" --k 5` on the real cache → `no results above threshold`.
- **Safety test** ✅ — no signing/mnemonic/broadcast import exists anywhere in `tools/brainmaxx/src` (grepped for `cosmjs|Wallet|Signing|mnemonic|MNEMONIC|broadcast`; only hits are the `author_wallet` data field and a comment stating the mnemonic is never touched).
- **Fork/replay test** ✅ — `brainmaxx replay <run_id>` reproduced the recorded pack and gate verdicts byte-for-byte after the `attach` step (fixed a real bug in the process: `moult-draft` and `replay` were letting post-`attach` claims leak into a gate-verdict snapshot recorded at `plan` time — replay now deliberately re-verifies only what `plan` originally asserted, matching what actually got recorded; regression test added in `test/determinism.test.js`).

## Not yet run

- **Stale test** (Value test 2, above) — no real redmark exists in the cache yet.
- **Sovereignty test** (network disabled) — not exercised in this session, but no command in the `recall`/`plan`/`attach`/`gates`/`moult-draft`/`replay` path makes a network call in the source; `snapshot`/`recall`/`plan`/`gates`/`moult-draft`/`replay` only touch the local cache file and `memory/brainmaxx/`.
