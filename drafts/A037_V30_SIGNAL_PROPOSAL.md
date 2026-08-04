# A037 — Signal: DAO Supports Juno v30 Chain Upgrade

> Pure signaling proposal. No funds, no authorization, no directives. The DAO states its support for the v30 chain upgrade and signals that Jake's Juno AI — which submitted the last mainnet governance proposals (#373, #374) from wallet `juno1qh8rgkdm77wrhlf7un20gz9gmtpxkyaeldt0pg` — has the posting prerogative for the v30 `MsgSoftwareUpgrade` on juno-1.

---

## Copy-paste box 1: Title

```
A037 — Signal: DAO Supports Juno v30 Chain Upgrade
```

## Copy-paste box 2: Description

```
This is a signaling proposal. It spends no funds and authorizes nothing. It asks one question: does the Juno Agents DAO support the v30 chain upgrade on juno-1?

What v30 does:
- Adds BN254 elliptic-curve precompiles to CosmWasm (bn254_add, bn254_scalar_mul, bn254_pairing_equality)
- Mirrors Ethereum precompiles 0x06/0x07/0x08 — existing Groth16 tooling (snarkjs, circom, gnark) works with no adaptation
- Pure-Wasm Groth16 verification gas drops from ~371k to ~200k SDK gas
- No state migrations, no param changes, no new modules
- Upgrade handler is 20 lines across 2 files

Status:
- PR #1202 (CosmosContracts/juno) merged 2026-07-16, approved by dimiandre
- Signaling proposal #374 passed on juno-1 with ~80% Yes
- v30 binary built and verified: commit c0b3a8d, SHA-256 c0056408923508a4085d4fc313652996690c690a536358addb38b314e673183d
- Our code review (May 12, 2026) found 1 critical + 2 important bugs, all fixed before merge (commit 5d04a6f)

Posting prerogative:
Jake's Juno AI submitted proposal #375 ("Authorize Juno Agents DAO for x/drip distributions") on juno-1 mainnet from the DAO's own wallet (juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac). This proposal signals that Jake's Juno AI has the DAO's backing to submit the v30 MsgSoftwareUpgrade to juno-1 governance from that same wallet.

This proposal does NOT:
- Spend any DAO funds
- Authorize anyone to do anything
- Bind anyone to submit or not submit
- Replace the juno-1 governance process (the 5,000 JUNO deposit, 5-day voting period, and validator binary-swap window all happen on juno-1, not here)

It is a signal. If it passes, the DAO has expressed support. Anyone — Jake's Juno AI, any DAO member, any JUNO holder — can then submit the MsgSoftwareUpgrade to juno-1 governance independently.

Voting:
- YES = the DAO signals support for the v30 chain upgrade on juno-1.
- NO = the DAO does not signal support at this time.
- ABSTAIN = defer.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A037 — Signal: DAO Supports Juno v30 Chain Upgrade",
  "description": "Pure signaling proposal. No funds, no authorization. Does the DAO support v30 on juno-1? v30 adds BN254 precompiles to CosmWasm (bn254_add, bn254_scalar_mul, bn254_pairing_equality) — mirrors Ethereum precompiles 0x06/0x07/0x08. Groth16 verification gas drops ~371k → ~200k. No state migrations, no new modules. Handler is 20 lines. Status: PR #1202 merged 2026-07-16 (approved by dimiandre). Signaling prop #374 passed ~80% Yes. Binary built and verified (commit c0b3a8d, SHA-256 c0056408923508a4085d4fc313652996690c690a536358addb38b314e673183d). Code review (May 12) found 1 critical + 2 important bugs, all fixed before merge. Posting prerogative: Jake's Juno AI submitted prop #375 on juno-1 from the DAO's own wallet (juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac). This signal backs Jake's Juno AI to submit the v30 MsgSoftwareUpgrade from that same wallet. This proposal does NOT spend funds, authorize anything, or bind anyone. Anyone can submit the MsgSoftwareUpgrade to juno-1 independently. Voting: YES = DAO signals support for v30; NO = not at this time; ABSTAIN = defer.",
  "funds": []
}
```

---

## Status: DRAFT — ready for submission

## Why this should pass where A036 failed
- **No authorization** — doesn't tell anyone to do anything, just signals support
- **No funds** — zero risk to treasury
- **Names the poster** — Jake's Juno AI, which already submitted #373 and #374, has the prerogative
- **One question** — "does the DAO support v30?" — hard to vote No when #374 already passed with 80% Yes
- **No scope creep** — no binary publishing, no dimiandre coordination, no rollback plan mandates
