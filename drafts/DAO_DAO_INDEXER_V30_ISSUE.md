# DAO DAO Indexer — Proposals Not Loading After Juno v30 Upgrade

## Summary

After the Juno v30 mainnet upgrade (prop #377, activated ~July 28, 2026), DAO DAO UI proposal pages are not rendering proposals for DAOs on juno-1. The DAO home page loads (daoCore dump_state, config, voting module info all return data), but the proposal list and individual proposal views fail to populate.

## Environment

- **Chain**: juno-1 (mainnet)
- **Upgrade**: v29 → v30 (prop #377, ~July 28, 2026)
- **v30 changes**: wasmvm v2.2.4 → v3.0.4, new `x/voting-snapshot` module, deleted `x/feeibc` (ICS-29) and `x/async-icq` (interchainquery) modules
- **DAO DAO UI**: daodao.zone (production, build `-NEymAhtuaFv-o8OYCbYd`)
- **Indexer**: Argus state-based indexer + TX indexer fallback
- **Affected DAO**: juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac (Juno Agents DAO)
- **Proposal module**: juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp (dao-proposal-single v2.7.0)

## What Works

- DAO home page renders (name, description, image, treasury)
- `daoDaoCore/dump_state` returns correct data via indexer
- `daoProposalSingle/config` returns correct voting config
- `daoProposalSingle/creationPolicy` returns correct policy
- `indexer/isCaughtUp` returns `true` for juno-1
- Direct cosmwasm smart queries via REST (`/cosmwasm/wasm/v1/contract/{addr}/smart/{base64_query}`) return proposal data correctly — proposals exist on-chain (A42 and A43 both visible via direct query)

## What Doesn't Work

- Proposal list on DAO home page shows empty or fails to load
- Individual proposal pages (e.g., `/dao/{addr}/proposals/A42`) don't render proposal content
- Vote tallies not displayed

## Root Cause Analysis

The Argus indexer has two modes: a **tracer** (state-based, uses `--trace-store` FIFO pipe to ingest raw KV store events) and a **listener** (event/transaction-based, processes blocks via RPC). Both modes have wasm-specific handlers/extractors that decode CosmWasm store keys and events.

The v30 upgrade jumped wasmvm from **v2.2.4 → v3.0.4** (via wasmd v0.50.x → v0.61.11). This is a major version break.

### Why some queries work but proposals don't

The DAO DAO UI uses two query paths:

1. **Contract smart queries** (e.g., `daoDaoCore/dump_state`, `daoProposalSingle/config`) — these are direct CosmWasm `QuerySmart` calls that go through the indexer's formula system but ultimately execute the contract's query handler. They work because wasmvm v3 can still execute queries against v2-era contracts.

2. **Indexer formulas that require indexed data** (e.g., `daoProposalSingle/reverseProposals`, proposal lists, vote lists) — these depend on the indexer having *extracted and stored* proposal creation/vote events from block processing. If the tracer's wasm handler can't decode wasmvm v3 store keys, or the listener's extractors can't parse wasmvm v3 event attributes, these formulas return empty.

### The chain fallback gap

Commit `95ae42f` (Oct 2025) added chain fallback for *individual* proposals and votes — if the indexer returns an error for `proposal/{id}`, the UI falls back to a direct `QuerySmart` call. This is why individual proposal pages might partially work.

However, there is **no chain fallback for the proposal list**. The `reverseProposals` / `listProposals` query depends entirely on the indexer having indexed the proposal creation events. Without the indexer, the UI would need to iterate proposal IDs from 1 to N — which it doesn't do.

### Specific wasmvm v3 breaking changes

1. **Store key format**: wasmvm v3 may have changed the internal store key prefix format for contract state. The Argus tracer's wasm handler (`src/tracer/handlers`) decodes raw KV store events by matching key prefixes. If the prefix format changed, all wasm state events are unparseable.

2. **Event attribute schema**: wasmvm v3's `handleContractResponse` emits `wasm` events with `_contract_address` and custom attributes. If the event type or attribute key format changed, the listener's extractors can't find contract interactions.

3. **Deleted module stores**: v30 deleted `x/feeibc` and `x/async-icq` stores. If the tracer has handlers that reference these modules' store prefixes, it may error on ingestion, causing a cascade failure that blocks all processing.

## Debugging Info

### Direct chain query confirms proposals exist

```
# Query A42 via REST
GET https://juno-rest.publicnode.com/cosmwasm/wasm/v1/contract/juno1jar50.../smart/{base64({"proposal":{"proposal_id":42}})}

# Returns: status=open, votes={yes:0, no:0, abstain:0}, total_power=6
```

### Indexer status page

The daodao.zone/status page shows Juno as "Up to date" at block 40,031,364 (07/21/2026), but this may be stale — the v30 upgrade happened after this date.

### DAO DAO UI query state

The dehydrated React Query state on the DAO page shows:
- `indexer/isCaughtUp` → `true`
- `daoCore/dumpState` → returns data
- `daoProposalSingle/config` → returns data
- No `reverseProposals` or `listProposals` query visible in dehydrated state (suggests it may be failing silently)

## Suggested Fix

### Option A: Argus indexer update (upstream)

1. **Update wasmvm v3 event parsing** in `src/listener/extractors/` — verify that wasm event attribute keys (`_contract_address`, `action`, custom attributes) are decoded correctly under wasmvm v3 / wasmd v0.61.
2. **Update tracer wasm handler** in `src/tracer/handlers/` — verify store key prefix matching against wasmvm v3 internal format.
3. **Add graceful error handling for deleted modules** — skip `x/feeibc` and `x/async-icq` store events without crashing.
4. **Re-index from v30 upgrade height** — after fixing handlers, reprocess blocks from 40,420,069 forward.

### Option B: DAO DAO UI fallback (quick mitigation)

Add a chain fallback for the proposal list query. When the indexer returns an empty proposal list for a DAO with a known proposal module, the UI should fall back to querying `ReverseProposals { start_before: null, limit: 30 }` directly via `QuerySmart` on the proposal module contract. This is the same pattern already used for individual proposals in commit `95ae42f`.

The fallback would look like:

```typescript
// In DaoProposalSingle.v2.ts, after indexer query returns empty:
if (!indexerResult || indexerResult.length === 0) {
  const client = await getCosmWasmClientForChainId(chainId)
  const proposalModule = new DaoProposalSingleV2QueryClient(client, address)
  const result = await proposalModule.reverseProposals({
    startBefore: null,
    limit: 30,
  })
  return result.proposals
}
```

This is a one-function change that would immediately restore proposal visibility for all DAOs on chains where the indexer is broken, without requiring an Argus deployment.

### Option C: Both

Ship Option B as an immediate UI patch, then work on Option A as the proper indexer fix.

## Reproduction

1. Visit https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac
2. Observe: DAO home page loads with name "Juno Agents DAO" and description
3. Observe: Proposals tab shows no proposals or fails to load
4. Verify on-chain: Query the proposal module contract directly via REST — proposals A42 and A43 exist with status=open

## Context

The Juno Agents DAO has active proposals (A42, A43) that need community votes. Without the DAO DAO UI rendering them, members can't easily view or vote on proposals. The proposals are accessible via direct contract queries but not via the UI.

---

*Filed by JunoClaw DAO builder. We have a proposal-indexer tool that queries the chain directly and works fine, but the broader Juno community relies on daodao.zone for governance participation.*
