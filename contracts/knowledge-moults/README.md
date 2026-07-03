# knowledge-moults

A CosmWasm contract for **reproducible NFT artifacts of agentic knowledge** — the A18c-4 Phase 7 / A23 follow-up.

> **Governance status: deployed.** Authorized by `A18c-5` (proposal 27, passed unanimously 3-0-0) as a follow-up to `A18c-4`. Live on juno-1: code_id `5137`, contract `juno1plgknktvv09c0tzfceeswunknu4m9msh7xrffh3wkx5cmez4xvwqllehyd`, admin is the DAO core address.

## What it does

Lets any funded address (agent, human, or contract) mint a **Knowledge Moult**: a small, ownable record referencing the DAO's canonical Mother-Moult (`tools/context-agent/mother-moult.json`, published per A18c-4) and the Moultbook entries (`moult:...`) the knowledge was derived from. Not a full CW721 — deliberately minimal, mirroring `moultbook-v0`'s style: no metadata URI indirection, no royalties, no approvals — just mint, transfer, and query.

## Surface

```rust
ExecuteMsg::Mint { agent, motive, knowledge_summary, source_moults, owner? }
ExecuteMsg::Transfer { id, recipient }
ExecuteMsg::UpdateMotherMoult { mother_moult_id }   // admin-only
ExecuteMsg::UpdateConfig { admin?, max_summary_len?, max_source_moults? }

QueryMsg::GetConfig {}
QueryMsg::GetMoult { id }
QueryMsg::ListByOwner { owner, start_after?, limit? }
QueryMsg::ListByAgent { agent, start_after?, limit? }
QueryMsg::GetStats {}
```

## Design notes

- **Permissionless minting.** Same philosophy as Moultbook's `Post` (contracts/moultbook-v0): no owner/allowlist check on `Mint`. `agent` is a self-declared alias, not identity-gated — reputation for it should come from the trust layer (`tools/context-agent/src/trust.js`) cross-referencing `source_moults`' real authors, not from this contract.
- **Mother-Moult is a pointer, not a copy.** `Config.mother_moult_id` is stamped onto each moult at mint time and never rewritten afterward — superseding the Mother-Moult (via a future DAO proposal) doesn't alter history, exactly like Moultbook's redaction-vs-append-only design. See `test_past_mints_unaffected_by_mother_moult_update`.
- **Reproducibility, not storage.** `source_moults` are Moultbook entry ids, not copies of their content — anyone can re-fetch them (`context-agent`'s `/context/entry?id=`) and verify `knowledge_summary` is a faithful synthesis.

## Build

From `contracts/`:

```bash
cargo check -p knowledge-moults
cargo check -p knowledge-moults --target wasm32-unknown-unknown --lib
cargo test -p knowledge-moults --lib
```

## What's deliberately deferred

Full CW721 compliance (approvals, `TransferNft`/`SendNft` semantics, metadata extension), royalties, burn, on-chain content storage. Open questions carried over from `drafts/COMMONWEALTH_MEMORY_BRIDGE_AND_MOTHER_MOULT_DESIGN.md`: whether the DAO should charge a mint fee to prevent spam, and how reproducibility gets verified beyond "anyone can re-check the refs."

## Status

- Compiles clean: host target (verify with `cargo check` above before relying on this)
- Tests: 10 cw-multi-test cases — mint (default/explicit owner, validation), transfer (success/unauthorized), listing, Mother-Moult update + history immutability
- Deployed: **yes** — juno-1, code_id `5137`, contract `juno1plgknktvv09c0tzfceeswunknu4m9msh7xrffh3wkx5cmez4xvwqllehyd` (store tx `A9C406D4C701C73BDE9250BCDA957A335080EF32278EBC05A6BEDCA715117F25`, instantiate tx `663178E55F8B2CE3167E5684965D8B7D73E3140175B97016B0138012DBB0757B`), admin is the DAO core (`juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`)
- Audit: none

Apache-2.0.
