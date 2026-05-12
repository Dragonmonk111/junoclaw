# ADR-003 — DEX Mirror v0: a Moultbook-anchored audit layer for the Junoswap Astroport fork

*Status: Draft. Author: Cascade, on behalf of VairagyaNodes. Date: 2026-05-12. Supersedes: nothing. Superseded by: nothing.*

---

## 1. Context

Jake Hartnell (Telegram, 2026-05-12) framed the Junoswap recovery as two tasks:

1. **Legacy UI triage** — get withdraw working so frozen LP positions unstick.
2. **New DEX = Astroport fork** — "quickest / safest" long-term replacement.

VairagyaNodes' position (per [`STRATEGIC_NOTES_2026_05_12.md`](./STRATEGIC_NOTES_2026_05_12.md) §1): we **don't fork Astroport** (that's Jake + Juno AI's lane). We instead build a thin companion contract — **`dex-mirror-v0`** — that subscribes to DEX events and emits Moultbook entries for every state-changing operation. The new DEX gets an auditable history layer for free, citable from any other contract on Juno.

This ADR captures the v0 schema, integration model, and open questions. It is **schema-only** — no Rust code is written here. The contract scaffolding follows once the Astroport-fork commits land and we can read the actual event shapes.

---

## 2. Decision

### 2.1 Contract scope

`dex-mirror-v0` is a single CosmWasm contract that:

1. **Subscribes** to events emitted by the Junoswap (Astroport fork) factory and per-pool contracts. Subscription is **pull-based** initially: a permissionless `IndexBlock { height }` execute call that scans the previous block's tx events and writes Moultbook entries for relevant DEX events. (Push-based via `cw-hooks` is the v1 upgrade path — see §6.)
2. **Stores** a minimal local index of DEX events: pool addresses, swap counts, LP-add/remove counts, fee-distribution counts. Enough for the contract to know what it's already mirrored (idempotency) and for cheap lookup queries.
3. **Posts** to a configured Moultbook contract using the existing `ExecuteMsg::Post` interface. Each DEX event becomes one Moultbook entry with `commitment = sha256(canonical_event_bytes)`, `content_type = "dex-event/<kind>"`, `refs` pointing at the contract-anchor entry for the pool, and `attestation_ref = Bridge { source_chain: "juno-1", tx_hash: "<dex_tx>" }`.

### 2.2 Event kinds mirrored

| DEX event | Moultbook content_type | refs | Commitment scheme |
|---|---|---|---|
| Pool creation | `dex-event/pool_create` | `[factory_anchor]` | sha256(factory_addr ‖ pool_addr ‖ token_a ‖ token_b) |
| Swap | `dex-event/swap` | `[pool_anchor]` | sha256(pool_addr ‖ trader ‖ input_denom ‖ input_amount ‖ output_denom ‖ output_amount ‖ block_height) |
| LP add | `dex-event/lp_add` | `[pool_anchor]` | sha256(pool_addr ‖ provider ‖ amounts_a ‖ amounts_b ‖ lp_shares_minted) |
| LP remove | `dex-event/lp_remove` | `[pool_anchor]` | sha256(pool_addr ‖ provider ‖ lp_shares_burned ‖ amounts_a ‖ amounts_b) |
| Fee distribution | `dex-event/fee_dist` | `[pool_anchor]` | sha256(pool_addr ‖ epoch ‖ total_fees ‖ recipients_count) |
| Pool param update | `dex-event/pool_param` | `[pool_anchor]` | sha256(pool_addr ‖ field_name ‖ old_value ‖ new_value ‖ height) |

**Pool anchors** are seeded by the first `pool_create` mirror for a given pool address; subsequent events on that pool cite the anchor entry id.

### 2.3 State

```
CONFIG:        Item<Config>
POOL_ANCHORS:  Map<Addr, String>  // pool_addr → moult:... entry id
LAST_INDEXED:  Item<u64>          // highest block height already indexed
MIRROR_STATS:  Item<Stats>        // counters per event_kind
```

```rust
pub struct Config {
    pub admin: Addr,
    pub moultbook_contract: Addr,     // address of the moultbook-v0 instance
    pub factory_contract: Addr,       // the Junoswap factory we mirror
    pub allowed_indexers: Vec<Addr>,  // empty = permissionless
    pub max_events_per_call: u32,     // gas-budget guardrail
}

pub struct Stats {
    pub total_indexed: u64,
    pub pool_creates: u64,
    pub swaps: u64,
    pub lp_adds: u64,
    pub lp_removes: u64,
    pub fee_dists: u64,
    pub pool_params: u64,
}
```

### 2.4 Messages

```rust
pub enum InstantiateMsg {
    pub admin: String,
    pub moultbook_contract: String,
    pub factory_contract: String,
    pub allowed_indexers: Vec<String>,    // empty = permissionless
    pub max_events_per_call: u32,         // suggest 50
}

pub enum ExecuteMsg {
    /// Anyone in allowed_indexers (or anyone if list empty) can trigger
    /// mirroring for a specific block range. The contract pulls events
    /// via deps.querier and emits one Moultbook Post per relevant event,
    /// capped at max_events_per_call.
    IndexRange { from_height: u64, to_height: u64 },

    /// Admin: update config
    UpdateConfig {
        admin: Option<String>,
        moultbook_contract: Option<String>,
        factory_contract: Option<String>,
        allowed_indexers: Option<Vec<String>>,
        max_events_per_call: Option<u32>,
    },
}

pub enum QueryMsg {
    GetConfig {},
    GetStats {},
    GetPoolAnchor { pool_addr: String },
    LastIndexedHeight {},
}
```

---

## 3. Integration story

### 3.1 Flow at runtime

1. **A swap happens** on the new DEX: trader sends `MsgExecuteContract` to pool, pool emits `wasm-swap` event.
2. **Some time later** (or in the same tx, if v1's `cw-hooks` integration is live), the indexer calls `dex-mirror-v0::IndexRange { from: H, to: H }`.
3. `dex-mirror-v0` queries the chain for events at height H (via the `BlockInfo` + tx querier surface), filters for events emitted by `cfg.factory_contract` or any known pool address.
4. For each matched event, the contract:
   - Constructs the event's canonical byte sequence per the commitment scheme in §2.2.
   - Looks up the pool anchor entry id (creating it on first sight if it's a `pool_create`).
   - Calls `moultbook_contract::Post { commitment, content_type, size_bytes, attestation_ref, visibility: Public, refs }` via `WasmMsg::Execute`.
   - Updates `LAST_INDEXED` and `MIRROR_STATS`.

### 3.2 What downstream contracts get

Any contract — DAO, treasury manager, analytics, oracle — can query Moultbook for citations of a pool's anchor entry and reconstruct the full state-change history without trusting any indexer. The on-chain commitment is the truth; off-chain blob storage is incidental.

Example: a treasury-rebalancer DAO can `ListByRef` on a pool anchor to get every swap, then sum `output_amount` per `output_denom` to compute realised PnL — all on-chain, all verifiable, all citation-anchored.

### 3.3 Permissioning model

- **`allowed_indexers = []`** (default): permissionless. Anyone pays gas to mirror events. Misbehaviour (wrong events) is bounded by the commitment-scheme check below.
- **`allowed_indexers = [addrs...]`**: only listed addresses can call `IndexRange`. Useful for DAOs that want to budget mirror-gas centrally.

**Misbehaviour defence.** An indexer that submits a wrong `commitment` for a real event ends up with a Moultbook entry whose commitment doesn't match what *another* indexer would compute. Discovery is offline-deterministic: any honest party can recompute the canonical bytes from the chain and notice the divergence. Honest indexers race to land the first correct entry; subsequent wrong entries fail the `DuplicateEntry` check in Moultbook (because the id derives from commitment ‖ sender ‖ time, so a malicious indexer can land a *different* id but the canonical-bytes mismatch is visible).

To strengthen this, v1 (see §6) can add a `claim-and-challenge` window where the first claim is provisional and challengers can submit a counter-commitment with proof of the canonical-byte derivation. v0 is "trust eventually, with cheap verification" — sufficient for an audit-trail use case.

---

## 4. Gas envelope (sketch)

Per `IndexRange { from_height: H, to_height: H }` call processing K events (K ≤ `max_events_per_call`):

| Step | Op count | SDK gas est. |
|---|---|---|
| CONFIG.load | 1× db_read | ~3,500 |
| Query block events | 1× chain query | ~10,000 (small block) |
| Filter relevant events | pure wasm | ~K × 100 |
| Per-event Moultbook Post | K × { 1 sub-msg with serialise + downstream Moultbook gas } | K × (~3,000 sub-msg + ~36,500 Moultbook Post) |
| LAST_INDEXED.save + MIRROR_STATS.update | 2× db_write | ~8,000 |

**K=10:** ~(3,500 + 10,000 + 1,000 + 10×40,000 + 8,000) = ~422K SDK = ~1.27M with multiplier. Fits comfortably in block gas budget.

**K=50 (max default):** ~(3,500 + 10,000 + 5,000 + 50×40,000 + 8,000) = ~2.03M SDK = ~6.1M with multiplier. Still fine, but at the upper edge for a single call. Suggest `max_events_per_call ≤ 50` default to keep individual calls reasonable.

---

## 5. Determinism review (anticipated)

Apply the same benchmark as `moultbook-v0` and `agent-company`:

- ✅ No floats, no HashMap iteration, no `std::time`.
- ✅ Block-event queries are deterministic within-block (they read finalised tx logs from height H, which is committed before H+1 produces).
- ✅ Commitment construction is byte-deterministic (canonical SHA256 over fixed-order fields).
- ✅ Schema evolution: new event kinds added later must use `Option<T> + #[serde(default)]` in any extension to `Config` / `Stats`.

**One open determinism question.** The "query block events" step depends on the exact CosmWasm host API for cross-block tx-event queries. If the API exposes events only for the *current* block, the contract needs a different pattern: each pool calls `dex-mirror-v0::ObserveEvent { canonical_bytes, kind, tx_hash }` directly in the same tx (this is the `cw-hooks` push pattern). The pull pattern in §3.1 is simpler conceptually but may not be implementable cleanly in v0 — to be confirmed when we read the actual Astroport fork's event-emission style.

---

## 6. Open questions

1. **Pull (`IndexRange`) vs push (`cw-hooks`).** Which is achievable in v0? If `cw-hooks` is available via the v30 chain feature (PR #1202 introduces `x/cw-hooks`!), the push pattern is strictly better — events are mirrored in the same block they fire, no indexer lag, no permissioning surface. **Coordinate with Jake / Juno AI: does v30's `x/cw-hooks` expose hook registration for arbitrary contract events, or only for staking-module events?**
2. **Astroport fork's event-emission style.** Does the fork emit structured `wasm-<kind>` events with all relevant fields as attributes, or does it use opaque payloads we need to decode? Affects the canonical-byte derivation.
3. **Pool anchor visibility.** Default `Public`. But for private pools (e.g. RFQ-style or dark pools), should we support `Group(...)` visibility? v0: always Public, document the constraint.
4. **Indexer incentives.** Permissionless indexers pay gas with no direct on-chain reward. The implicit reward is "first to anchor a pool gets named in the anchor entry's `author_alias` field". v1 could add a small Junoswap fee-share to active indexers, mediated by `cfg.indexer_fee_bps`.
5. **Storage budget.** At a busy DEX (1M swaps/year), Moultbook accumulates ~1M entries per year purely from this mirror. That's ~300 MB/y at our v0 entry sizes. Manageable, but worth a per-event-kind retention setting in Moultbook v1 (e.g. swap entries retained 90 days, LP entries forever).
6. **Composability with the agent-company `OutcomeResolve` flow.** A DAO that's running a verifiable outcome market over a DEX price could cite the DEX mirror's swap entries as the data source. This is a natural composition; no contract changes needed, just a documentation pattern.

---

## 7. Sequencing

1. **Wait for the Astroport fork's first commits on `CosmosContracts/junoswap-v2` (or whichever repo Jake / Juno AI chooses).** Read the actual factory + pool contracts. Confirm event-emission style. Identify the exact event names.
2. **Confirm v30's `x/cw-hooks` capability** with Jake. If hooks support arbitrary contract events, switch this ADR to push-only. If not, ship pull-mode v0.
3. **Write `contracts/dex-mirror-v0/`** crate following the Moultbook v0 structure: `lib.rs`, `state.rs`, `msg.rs`, `error.rs`, `contract.rs`, `tests.rs`. Mirror the Moultbook v0 deterministic-scrutiny discipline from day one. Estimated 2–3 days of work post-confirmation.
4. **Deploy to devnet alongside Moultbook v0.** Test with mock DEX events first, then real fork events once Jake's deployment is live.
5. **Open a PR against `CosmosContracts/junoswap-v2` (or equivalent)** with the integration suggestion: if push-mode is supported, register `dex-mirror-v0::ObserveEvent` as a hook target.

---

*Apache-2.0. This ADR will land as a Moultbook entry once Moultbook v0 is on devnet, citing this commit's hash as its anchor and the eventual `contracts/dex-mirror-v0/` crate as its companion artifact.*
