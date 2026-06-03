# Deterministic Audit — `junoswap-factory`

*Apache-2.0. Methodology: [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md). Sister audits: [`junoswap-pair/DETERMINISTIC_AUDIT.md`](../junoswap-pair/DETERMINISTIC_AUDIT.md), [`agent-registry/DETERMINISTIC_AUDIT.md`](../agent-registry/DETERMINISTIC_AUDIT.md), [`zk-verifier/DETERMINISTIC_AUDIT.md`](../zk-verifier/DETERMINISTIC_AUDIT.md). Anchor: contract source at `contracts/junoswap-factory/src/` as of 2026-05-14.*

## Summary (3 lines)

`junoswap-factory` is the registry contract that spawns AMM pair contracts via `WasmMsg::Instantiate`. The contract is **not production-functional in its current state**: `CreatePair` emits an event and submits the instantiate sub-message, but **never writes to `PAIRS` or `ALL_PAIRS` storage** (a code comment at `contract.rs:114-115` acknowledges this). The duplicate-pair guard is therefore a no-op, queries return empty results, and `PAIR_COUNT` drifts ahead of actual pair existence. Five additional lower-severity findings noted below.

## Resolution (2026-06-02)

| ID | Status | Resolution |
|----|--------|------------|
| F1 | **FIXED** | `CreatePair` now uses `SubMsg::reply_on_success`; a new `reply` entry_point parses `_contract_address` from the instantiate event and writes `PAIRS` + `ALL_PAIRS`. `PENDING_PAIRS` map stashes in-flight metadata. Tests added: `test_create_pair_registers_pair_via_reply`, `test_duplicate_pair_rejected_after_registration`, `test_reply_unknown_id_rejected`. |
| F3 | **FIXED** | `AllPairs` now uses `Bound::exclusive(start_after)` — O(limit) instead of O(n). |
| F4 | **FIXED** | `new_id` uses `checked_add(1)` → `ContractError::Overflow`. |
| F5 | **FIXED** | Empty `migrate` entry_point added (`MigrateMsg`); `test_migrate_ok`. |
| F2 | deferred | Fee ceiling left at 10000 — `test_fee_boundary_10000_valid` asserts it as a valid boundary; tightening to a lower `MAX_FEE_BPS` is a separate policy change. |
| F6 | deferred | `cw2` introspection of `junoclaw_contract` — optional polish, deferred to v0.2. |

Factory suite: **17 tests passing.** Findings below are retained as the original 2026-05-14 audit record.

## Methodology

Same 4-axis deterministic-scrutiny benchmark used on the other six audited contracts:

1. **Authority surface** — who can do what, and is least-privilege respected?
2. **State integrity** — does every storage mutation match the intended observable effect?
3. **Failure determinism** — can a single tx leave the contract in an inconsistent state (partial-write, post-handler unwind, async reply)?
4. **Resource bounds** — gas, recursion, iteration, integer overflow.

Findings ranked by exploitability × blast-radius. HIGH = correctness-affecting in normal operation. MEDIUM = correctness-affecting under specific conditions or design-choice. LOW = operational quality / cosmetic / dead surface.

## Findings

| ID | Severity | Title | Affected |
|----|----------|-------|----------|
| F1 | **HIGH** | `CreatePair` never writes the pair to `PAIRS` / `ALL_PAIRS` maps — registry is non-functional | `contract.rs:70-130` |
| F2 | MEDIUM | `default_fee_bps` ceiling is 10000 (100%); typical AMM caps are 30-100 bps | `contract.rs:30, 90, 148` |
| F3 | LOW | `AllPairs` query scans all entries instead of using `Bound::inclusive(start)` | `contract.rs:185-201` |
| F4 | LOW | `count + 1` uses unchecked arithmetic on u64 (practically unreachable but stylistically inconsistent with `checked_add` elsewhere in the codebase) | `contract.rs:95` |
| F5 | LOW | No `migrate` entry_point; state evolution requires redeploy + state reconstruction | `contract.rs` |
| F6 | LOW | `junoclaw_contract` is stored in config and forwarded to `PairInstantiateMsg` but factory itself never interacts with it; dead surface from the factory's perspective | `contract.rs:34-44, 103, 153-155` |

---

## F1 — `CreatePair` never writes the pair to `PAIRS` / `ALL_PAIRS` maps (HIGH)

### Observation

`state.rs` declares two index maps:

```rust
pub const PAIRS: Map<(&str, &str), Addr> = Map::new("pairs");
pub const ALL_PAIRS: Map<u64, PairRecord> = Map::new("all_pairs");
```

`contract.rs:execute_create_pair` reads `PAIRS` for the duplicate check:

```rust
let (key_a, key_b) = sort_assets(&token_a, &token_b);
if PAIRS.has(deps.storage, (&key_a, &key_b)) {
    return Err(ContractError::PairExists {});
}
```

It then:
- Computes `new_id = count + 1`
- Builds `pair_instantiate_msg` and `instantiate_msg`
- Saves `PAIR_COUNT.save(deps.storage, &new_id)?;`
- Emits the `wasm-create_pair` event
- Returns the `Response` with `add_message(instantiate_msg).add_event(event)`

Search for `PAIRS.save` or `ALL_PAIRS.save` across the entire contract: **zero hits.** The code comment explicitly flags this:

```rust
// For now, store a placeholder — the pair address will be set via reply or manual registration
// In production, use Reply to capture the instantiated address
PAIR_COUNT.save(deps.storage, &new_id)?;
```

There is also **no `reply` entry_point** anywhere in `contract.rs`.

### Failure modes

1. **`Pair { token_a, token_b }` query always fails** — `PAIRS.load(...)` returns `StdError::NotFound` for every pair the factory has ever "created."
2. **`AllPairs { ... }` query always returns `[]`** — `ALL_PAIRS.range(...)` is empty.
3. **`PairCount {}` query returns `N` while `Pair`/`AllPairs` say zero pairs exist** — `PAIR_COUNT` increments on every `CreatePair` regardless of whether the child instantiate succeeds, so the count drifts ahead of any reality.
4. **Duplicate-pair guard is a no-op.** `PAIRS.has(deps.storage, (&key_a, &key_b))` always returns `false` because nothing is ever stored. A user can call `CreatePair { token_a: ujuno, token_b: uusdc, ... }` an unbounded number of times; each call spawns a new pair contract (real cost, real chain state), and the factory considers each one a fresh creation. There is no on-chain way to deduplicate.
5. **Child-instantiate failure is invisible.** The `instantiate_msg` is added via `Response::add_message(...)` — i.e. a non-reply sub-message. If the pair-code-id is missing, malformed, or rejects the init, the parent transaction unwinds atomically (good), but the factory still has no way to detect a *successful* child for later registration.

### Severity rationale — HIGH

Three of four query endpoints (`Pair`, `AllPairs`, `PairCount`-vs-reality) are broken in normal operation, not under attacker pressure. Duplicate-pair protection is advertised by the `PairExists` error variant in `error.rs` but architecturally cannot fire. Any frontend or off-chain indexer that calls `AllPairs` to enumerate the AMM has no usable answer.

This is not a security finding in the "attacker-extracts-funds" sense — the factory holds no funds, takes no funds as fees, and its only side effect (sub-message spawn) still works. But it is a **correctness** finding at the level of "the contract does not do what its interface claims," which is the bar for HIGH per our benchmark.

### Suggested fix

Two paths, both standard CosmWasm patterns:

**Option A — Reply-based (production shape).** Convert the `add_message` to a tracked sub-message and add a `reply` handler:

```rust
const CREATE_PAIR_REPLY_ID: u64 = 1;

let sub = SubMsg::reply_on_success(instantiate_msg, CREATE_PAIR_REPLY_ID);

// Stash the pending pair info keyed by reply_id so the reply handler can finalize it.
PENDING_PAIRS.save(deps.storage, CREATE_PAIR_REPLY_ID, &PendingPair { ... })?;

Response::new().add_submessage(sub) ...

// New entry point:
#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        CREATE_PAIR_REPLY_ID => {
            let pair_addr = parse_instantiate_addr_from_reply(reply.result)?;
            let pending = PENDING_PAIRS.load(deps.storage, CREATE_PAIR_REPLY_ID)?;
            let key_a = pending.token_a.denom_key();
            let key_b = pending.token_b.denom_key();
            PAIRS.save(deps.storage, (&key_a, &key_b), &pair_addr)?;
            ALL_PAIRS.save(deps.storage, pending.id, &PairRecord {
                id: pending.id,
                pair_addr,
                token_a: pending.token_a,
                token_b: pending.token_b,
                created_at: env.block.height,
            })?;
            PENDING_PAIRS.remove(deps.storage, CREATE_PAIR_REPLY_ID);
            Ok(Response::new().add_attribute("registered_pair", pair_addr))
        }
        _ => Err(ContractError::UnknownReplyId {}),
    }
}
```

Cost: ~80-100 LoC of new code (handler + state struct + parsing helper) + 1-2 unit tests.

**Option B — Manual-register (lighter-weight).** Add an `ExecuteMsg::RegisterPair { pair_addr, token_a, token_b }` that the off-chain indexer (or the pair contract itself, via a callback) submits after instantiation succeeds. Authority gate: only callable by the address that called `CreatePair` (track in a per-id pending map) or by `config.owner`.

Option A is the production shape — it preserves atomicity and removes the off-chain dependency. Option B is the quick-fix if reply-handler complexity isn't justified for v0.1.

### Test coverage gap

`tests.rs:test_create_pair_emits_event` asserts the event is emitted but never re-queries `Pair` or `AllPairs` to confirm registration. Add:

```rust
#[test]
fn test_create_pair_registers_pair() {
    // ... after CreatePair execute ...
    let res: PairResponse = from_json(
        query(deps.as_ref(), mock_env(), QueryMsg::Pair { token_a, token_b }).unwrap()
    ).unwrap();
    assert_eq!(res.pair_addr, expected_instantiated_addr);
}

#[test]
fn test_duplicate_pair_rejected() {
    // ... first CreatePair succeeds ...
    let err = execute(deps.as_mut(), env, info, same_msg).unwrap_err();
    assert!(matches!(err, ContractError::PairExists {}));
}
```

Both tests would currently fail. Adding them would have caught F1 at PR time.

---

## F2 — `default_fee_bps` ceiling is 10000 (100%) (MEDIUM)

### Observation

Three places enforce `fee_bps <= 10000`:

- `contract.rs:30` (instantiate)
- `contract.rs:90` (execute_create_pair)
- `contract.rs:148` (execute_update_config)

10000 bps = 100%, meaning a swap of any amount would be entirely consumed as fee. This is technically a valid configuration but produces a non-functional AMM.

Typical real-world AMM fee ranges:
- Uniswap v2: 30 bps (0.3%)
- Uniswap v3 tiers: 1 / 5 / 30 / 100 bps
- Curve: 4-40 bps
- Sushi: 25-30 bps
- Pancakeswap: 25 bps

A reasonable contract-enforced ceiling for an AMM factory would be `300` (3%) or `1000` (10%). Anything higher is operator footgun territory.

### Severity rationale — MEDIUM

Not a security bug; the owner can already grief their own AMM (e.g. setting fee to 9999 bps). But the `> 10000` check reads as defensive programming that doesn't actually defend. Tightening it to `> 1000` would (a) match real-world AMM expectations, (b) preserve the headroom for legitimate experimentation, (c) not break any current caller (`tests.rs` uses 30 and 50 bps).

### Suggested fix

```rust
const MAX_FEE_BPS: u16 = 1000;  // 10% — generous upper bound for legitimate AMMs
// ... change `> 10000` to `> MAX_FEE_BPS` in all three places
```

Add a `MaxFeeBpsResponse` query or expose `MAX_FEE_BPS` via the `Config` query so frontends know the limit.

---

## F3 — `AllPairs` query scans all entries instead of using `Bound::inclusive(start)` (LOW)

### Observation

`contract.rs:185-201`:

```rust
let start = start_after.map(|s| s + 1).unwrap_or(1);
let pairs: Vec<PairResponse> = ALL_PAIRS
    .range(deps.storage, None, None, Order::Ascending)
    .filter(|r| r.as_ref().map(|(k, _)| *k >= start).unwrap_or(false))
    .take(limit)
    .map(|r| { /* ... */ })
    .collect::<StdResult<_>>()?;
```

The `range(deps.storage, None, None, ...)` scans every key in `ALL_PAIRS` and the `.filter(...)` discards keys below `start` in memory. For a factory with thousands of pairs, the gas cost scales with `n` (total pairs), not with `limit`.

The idiomatic `cw-storage-plus` form is:

```rust
use cw_storage_plus::Bound;

let start_bound = start_after.map(Bound::exclusive);
let pairs: Vec<PairResponse> = ALL_PAIRS
    .range(deps.storage, start_bound, None, Order::Ascending)
    .take(limit)
    .map(|r| { /* ... */ })
    .collect::<StdResult<_>>()?;
```

This skips directly to the start key at the storage layer, making the gas cost `O(limit)`.

### Severity rationale — LOW

Today this is invisible because F1 means `ALL_PAIRS` is always empty. Once F1 is fixed and the factory accumulates pairs (which it will, given it's a permissionless `CreatePair`), the inefficiency starts mattering. Even at 100 pairs, the scan-and-filter form is ~100 storage reads per query rather than `limit`. By 1000 pairs the query becomes prohibitive.

Same pattern appears in `agent-registry` query handlers (per audit finding there) — would be worth surfacing as a cross-cutting pattern in [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md).

---

## F4 — `count + 1` unchecked arithmetic (LOW)

`contract.rs:95`:

```rust
let new_id = count + 1;
```

`count` is `u64`. Overflow is unreachable in any practical scenario. The codebase elsewhere uses `checked_add` for similar increments (e.g., in `agent-registry` and `zk-verifier`). Stylistic inconsistency, not a bug. Suggested change:

```rust
let new_id = count.checked_add(1).ok_or(ContractError::Overflow {})?;
```

Add the variant to `error.rs`:

```rust
#[error("Overflow")]
Overflow {},
```

---

## F5 — No `migrate` entry_point (LOW)

No `#[entry_point] pub fn migrate(...)` in `contract.rs`. If state schema evolves (e.g., F1 fix changes `PairRecord` shape), the only path is redeploy + state reconstruction by the off-chain owner. For a registry contract this is awkward because the natural fix for F1 introduces a new state variant (`PENDING_PAIRS` map).

Recommend adding an empty `migrate` now so future schema changes have an in-place migration path:

```rust
#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "migrate"))
}
```

---

## F6 — `junoclaw_contract` is dead surface from the factory's perspective (LOW)

`config.junoclaw_contract` is:
- Validated in `instantiate` (line 34-37)
- Stored in `Config` (line 43)
- Forwarded to `PairInstantiateMsg` (line 103)
- Updated by `execute_update_config` (line 153-155)

It is **never** queried, dispatched to, or otherwise used by the factory. The actual `junoclaw_contract` interaction lives in the child pair contract (per `junoswap-pair/contract.rs` referenced in the sister audit).

This is fine for the factory — it's just plumbing. Worth flagging as LOW because:

1. The factory has no way to validate that the address is actually a JunoClaw contract (e.g., via `cw2::query_contract_info`). A misconfigured owner could pass an arbitrary address and the pairs would silently malfunction.
2. The factory could expose a config-level health check (e.g., `QueryMsg::JunoclawContractHealth {}` that runs `cw2` introspection on the configured address).

Suggested fix (optional polish): in `instantiate` and `execute_update_config`, after `addr_validate`, optionally also `cw2::query_contract_info` to confirm it's a JunoClaw-shaped contract:

```rust
if let Some(addr) = msg.junoclaw_contract {
    let addr = deps.api.addr_validate(&addr)?;
    let info: cw2::ContractVersion = deps.querier.query_wasm_smart(
        &addr,
        &cw2::CW2QueryMsg::GetContractInfo {},
    )?;
    if !info.contract.contains("junoclaw") {
        return Err(ContractError::InvalidJunoclawContract {});
    }
    Some(addr)
} else {
    None
}
```

This adds a query on instantiate/update but catches misconfiguration at the point of action rather than at pair-execute-time.

---

## Severity-weighted summary

- **HIGH (1):** F1. **Blocks v0.1 production use.** Either Option A (reply handler) or Option B (manual-register endpoint) is required before this contract can be considered functional.
- **MEDIUM (1):** F2. Tighten before mainnet.
- **LOW (4):** F3-F6. Polish before mainnet but no block.

## Recommended sprint sequencing

1. **F1 fix (Option A: reply handler)** — 80-100 LoC + 2 unit tests covering the now-real registration path. Confirms duplicate-pair-rejection works once `PAIRS` is populated. **Highest priority.**
2. **F3 fix (Bound::inclusive in AllPairs)** — 5 LoC change. Confirms gas profile is `O(limit)` not `O(n)`. **Fold into the F1 PR** since both touch query paths.
3. **F2 ceiling tightening** — 3 LoC + new constant. Bundle with the F1 PR.
4. **F4 `checked_add`** — 3 LoC + new error variant. Bundle.
5. **F5 `migrate`** — 5 LoC empty stub. Bundle.
6. **F6 `cw2` introspection** — 15 LoC. Defer to v0.2 if the F1 PR is already large.

All six findings can land in one PR alongside an F1-driven test expansion. Sister contracts (`junoswap-pair`) are clean from the equivalent reply-handler perspective per [`junoswap-pair/DETERMINISTIC_AUDIT.md`](../junoswap-pair/DETERMINISTIC_AUDIT.md).

## Cross-references

- [`junoswap-pair/DETERMINISTIC_AUDIT.md`](../junoswap-pair/DETERMINISTIC_AUDIT.md) — the spawned-by-factory contract; reply-vs-add_message pattern already correctly used there.
- [`agent-registry/DETERMINISTIC_AUDIT.md`](../agent-registry/DETERMINISTIC_AUDIT.md) — same `range`-without-`Bound` pattern flagged in that contract's audit (F3 cross-cutting).
- [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md) — methodology + cross-contract finding index.

---

*Audited 2026-05-14 by Cascade/VairagyaNodes deterministic-scrutiny pass. Code anchor: `contracts/junoswap-factory/src/{contract,state,msg,error,tests}.rs`. Apache-2.0.*
