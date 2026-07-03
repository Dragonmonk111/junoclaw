# knowledge-moults

A CosmWasm contract for **reproducible NFT artifacts of agentic knowledge** — the A18c-4 Phase 7 / A23 follow-up.

> **Governance status: code-only, not proposed or deployed.** A18c-4 explicitly scoped this out ("No Knowledge Moult NFT contract yet — follow-up proposal after this passes"), and `COMMONWEALTH_SHARED_MEMORY_BUILD_PLAN.md` defers Phase 7 until Phase 5/6 are stable. This crate exists so the contract is ready to review and test *before* asking the DAO to fund an instantiation — it must not be deployed to any network without its own DAO DAO proposal.

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
- Deployed: **no** — requires its own DAO DAO proposal per A18c-4
- Audit: none

Apache-2.0.
