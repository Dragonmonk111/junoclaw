# Junoswap fork — audit findings report

*Drafted 2026-05-15 from the deterministic-audit pass on the JunoClaw fork of Junoswap. Apache-2.0. Suitable to share with Jake Hartnell (who mentioned vibecoding a Junoswap replacement) or to attach to a follow-up DM. Source-of-truth for full detail: [`contracts/junoswap-factory/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-factory/DETERMINISTIC_AUDIT.md) and [`contracts/junoswap-pair/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-pair/DETERMINISTIC_AUDIT.md).*

## TL;DR (read this paragraph)

We audited two Junoswap contracts (`junoswap-factory` + `junoswap-pair`) under the deterministic-scrutiny benchmark and found **two HIGH-severity bugs that would brick the AMM in production**, plus one MEDIUM determinism violation:

1. **`junoswap-factory` F1 — registry is non-functional.** `CreatePair` increments `PAIR_COUNT` and emits the right event, but never writes to the `PAIRS` or `ALL_PAIRS` maps. The `AllPairs` query returns `[]` regardless. The duplicate-pair guard always reads "no duplicate" so users can spawn unbounded pair contracts for the same token couple.
2. **`junoswap-pair` F1 — Uniswap v2 first-depositor inflation attack.** No `MIN_LIQUIDITY` lockup on initial LP mint. An attacker who deposits 1 + 10000 first, mints 100 LP at 100% share, then watches a victim depositing 100 + 1000 mint only 10 LP — the attacker captures ~91% of pool value with the 1+10000 seed. Standard Uniswap v2 patch from a decade ago that this fork didn't re-apply.
3. **`junoswap-pair` F4 — `f64` arithmetic in the `Pool` query.** Float arithmetic is a CosmWasm determinism red flag: validators running on different machines can in principle disagree on the query response. Also loses precision past 2^53.

Total findings: 1 HIGH (factory) + 1 HIGH + 4 MEDIUM + 3 LOW (pair) = 9 findings across the two contracts. **None of the HIGH findings are stretches** — the factory bug has an in-source comment acknowledging the gap; the pair bug is the canonical Uniswap-v2 inflation attack with a worked-example-by-token-amounts in the audit. Anyone forking Junoswap should read these before relying on the contracts in production.

## How to use this report

Three reasonable consumer audiences:

- **If you're rebuilding Junoswap from scratch** — use this report's findings as the test plan for your replacement. Each HIGH finding maps to a regression test you want before commit 1.
- **If you're patching the existing Junoswap fork** — the `Suggested fix` sections in each per-contract audit doc are PR-ready (LoC counts and test asserts included).
- **If you're reviewing a third party's Junoswap port** — these are the questions to ask first.

We're happy to send PRs against either the existing fork or the replacement, whichever ships.

---

## F1 (junoswap-factory) — `CreatePair` never writes to the registry

**Severity:** HIGH
**Location:** `contracts/junoswap-factory/src/contract.rs:70-130`

`state.rs` declares the right maps:

```rust
pub const PAIRS: Map<(&str, &str), Addr> = Map::new("pairs");
pub const ALL_PAIRS: Map<u64, PairRecord> = Map::new("all_pairs");
```

But `execute_create_pair`:

1. Reads `PAIRS.has(deps.storage, (&key_a, &key_b))` for the duplicate check (always returns `false`).
2. Computes `new_id = count + 1`, builds the `PairInstantiateMsg` and `WasmMsg::Instantiate`.
3. Saves `PAIR_COUNT.save(...)`.
4. Emits `wasm-create_pair`.
5. Returns `Response::new().add_message(instantiate_msg).add_event(event)`.

There is **no `PAIRS.save(...)`, no `ALL_PAIRS.save(...)`, and no `reply` entry_point**. A code comment at line 114-115 acknowledges:

```rust
// For now, store a placeholder — the pair address will be set via reply or manual registration
// In production, use Reply to capture the instantiated address
```

### Failure modes

| Symptom | Effect |
|---|---|
| `Pair { token_a, token_b }` query | Always `StdError::NotFound`, regardless of how many pairs were "created" |
| `AllPairs { ... }` query | Always returns `[]` |
| `PairCount {}` query | Drifts ahead of reality — every successful `CreatePair` increments it, but the underlying pair has no on-chain registration entry |
| `PairExists` duplicate-pair guard | No-op — `PAIRS.has(...)` always `false` |
| Frontend / indexer integration | No way to enumerate the AMM through chain queries |

### Fix shape

Use a `Reply` sub-message to capture the instantiated pair address, then write to both maps in the reply handler:

```rust
const INSTANTIATE_PAIR_REPLY_ID: u64 = 1;

fn execute_create_pair(...) -> Result<Response, ContractError> {
    // ... existing validation ...
    
    let sub_msg = SubMsg::reply_on_success(
        WasmMsg::Instantiate { /* ... */ },
        INSTANTIATE_PAIR_REPLY_ID,
    );
    
    // Stash the pending pair info under PAIR_COUNT so reply can complete the write.
    PENDING_PAIR.save(deps.storage, &PendingPair { id: new_id, token_a, token_b, fee_bps })?;
    PAIR_COUNT.save(deps.storage, &new_id)?;
    
    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_event(event))
}

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_PAIR_REPLY_ID => {
            let pending = PENDING_PAIR.load(deps.storage)?;
            let pair_addr = parse_instantiate_address(&msg.result)?;
            let (key_a, key_b) = sort_assets(&pending.token_a, &pending.token_b);
            
            PAIRS.save(deps.storage, (&key_a, &key_b), &pair_addr)?;
            ALL_PAIRS.save(deps.storage, pending.id, &PairRecord { 
                pair_addr: pair_addr.clone(), 
                token_a: pending.token_a, 
                token_b: pending.token_b, 
                fee_bps: pending.fee_bps 
            })?;
            PENDING_PAIR.remove(deps.storage);
            
            Ok(Response::new()
                .add_attribute("action", "create_pair_reply")
                .add_attribute("pair_addr", pair_addr))
        }
        _ => Err(ContractError::UnknownReplyId { id: msg.id }),
    }
}
```

Plus a regression test that calls `CreatePair`, mocks the reply, then asserts `Pair { token_a, token_b }` returns the expected address. ~50 LoC + 1 test.

---

## F1 (junoswap-pair) — first-depositor inflation attack (no `MIN_LIQUIDITY`)

**Severity:** HIGH
**Location:** `contracts/junoswap-pair/src/contract.rs:78-87`

The classic Uniswap v2 first-depositor attack. Initial LP minting:

```rust
let lp_shares = if pool.total_lp_shares.is_zero() {
    let product = Uint256::from(amount_a) * Uint256::from(amount_b);
    isqrt_u256(product)  // Geometric mean — no MIN_LIQUIDITY subtraction
} else {
    let share_a = amount_a.multiply_ratio(pool.total_lp_shares, pool.reserve_a);
    let share_b = amount_b.multiply_ratio(pool.total_lp_shares, pool.reserve_b);
    share_a.min(share_b)
};
```

### Worked example

1. Attacker provides `1` of token_a and `10000` of token_b. Initial LP = `floor(sqrt(1*10000)) = 100`.
2. Reserves now `(1, 10000)` — heavily skewed. Attacker holds 100% of LP.
3. Victim wants to provide `100 + 1000` (intending equal-value at the going market rate). Computes:
   - `share_a = 100 * 100 / 1 = 10000`
   - `share_b = 1000 * 100 / 10000 = 10`
   - `min = 10`. Victim mints **10 LP shares** for a `100 + 1000` deposit.
4. Total LP now 110, reserves `(101, 11000)`. Victim's share = 10/110 = **9.1%**. Attacker's = 100/110 = **90.9%**.
5. Victim deposited 1100 worth (at the 1:10 ratio) but only got 9.1% of pool value. The attacker captured 90.9% of the value with their 1+10000 seed.

### Why the existing test suite missed it

`tests.rs` covers initial liquidity for symmetric small deposits (1+1, 100+100). The test plan didn't include "skewed initial deposit" because the attack only manifests when (a) the initial deposit is heavily ratio-skewed, and (b) the second depositor expects the AMM to mint shares proportional to value. The existing tests assert the math is correct *for the inputs provided* — which it is. The bug is that the math is exploitable when the inputs are adversarially chosen.

### Fix shape

Standard Uniswap v2 mitigation — subtract `MIN_LIQUIDITY = 1000` from the first LP mint and lock those shares forever:

```rust
const MIN_LIQUIDITY: u128 = 1000;

if pool.total_lp_shares.is_zero() {
    let product = Uint256::from(amount_a) * Uint256::from(amount_b);
    let raw_lp = isqrt_u256(product);
    if raw_lp <= Uint128::new(MIN_LIQUIDITY) {
        return Err(ContractError::InsufficientInitialLiquidity {});
    }
    let user_lp = raw_lp - Uint128::new(MIN_LIQUIDITY);
    pool.total_lp_shares = raw_lp;  // includes MIN_LIQUIDITY (locked, no address)
    LP_SHARES.save(deps.storage, &info.sender, &(prev + user_lp))?;
    // The MIN_LIQUIDITY stays in total_lp_shares but is never assigned — permanently locked.
}
```

~25 LoC + 2 tests:

- **Test A:** asserts MIN_LIQUIDITY is locked and not withdrawable.
- **Test B:** the worked-example attack now reduces the attacker's share to ~50% of fair value (the bound the lockup provides). Replays the 1+10000 then 100+1000 scenario and asserts victim's LP share is ≥45%.

---

## Other findings (compact form)

### `junoswap-factory`

- **F2 MEDIUM** — `default_fee_bps` ceiling is 10000 (100%); typical AMM caps are 30-100 bps. Fix: cap at `MAX_FEE_BPS = 100` (1%).
- **F3 LOW** — `AllPairs` query scans all entries; should use `Bound::exclusive(start)`.
- **F4 LOW** — `count + 1` unchecked arithmetic on u64.
- **F5 LOW** — no `migrate` entry_point.
- **F6 LOW** — `junoclaw_contract` is dead weight in this contract; metadata-only.

Full detail: [`contracts/junoswap-factory/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-factory/DETERMINISTIC_AUDIT.md).

### `junoswap-pair`

- **F2 MEDIUM** — no reconciliation between stored reserves and bank balance. Donations are locked but not exploitable.
- **F3 MEDIUM** — tiny-swap zero-fee exploit (fee floors to zero for `offer_amount < 334` at 30 bps). Fix: round fee UP, not DOWN.
- **F4 MEDIUM** — `f64` in `Pool` query. **Determinism red flag.** Replace with rational-number string formatting.
- **F5 LOW-MEDIUM** — `WithdrawLiquidity` doesn't read `info.funds`; attached funds are silently absorbed. Same shape as the v6 F4 fix that landed for `ProvideLiquidity`/`Swap` but didn't extend to `WithdrawLiquidity`.
- **F6 LOW** — `factory` and `junoclaw_contract` fields stored but never invoked.
- **F7 LOW** — no `migrate` entry_point.
- **F8 LOW** — `total_swaps: u64` overflow (academic only — 580 billion years at one swap/sec).

Full detail: [`contracts/junoswap-pair/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-pair/DETERMINISTIC_AUDIT.md).

## Summary table

| Contract | HIGH | MEDIUM | LOW | Total |
|---|---|---|---|---|
| `junoswap-factory` | 1 | 1 | 4 | 6 |
| `junoswap-pair` | 1 | 4 | 3 | 8 |
| **Total** | **2** | **5** | **7** | **14** |

Both HIGH findings are landable as 1 PR each (~50 LoC + tests for the factory; ~25 LoC + 2 tests for the pair). Combined PR effort: ~half a day if landing in the existing fork; ~1 day if integrated into a fresh rewrite (mainly because rewrites tend to do more polish than is needed for a pure fix-the-bugs PR).

## Suggested next step

If you're shipping a Junoswap replacement, the most useful next step is probably a 30-minute call (or async DM) where we walk through the worked-example attack on F1 (pair) and the architectural choice for the F1 (factory) reply-handler shape. Both are simple but have one or two design-axis decisions worth aligning on. Either of us can send the patches once the shape is agreed.

If you'd rather run with this report standalone, the per-contract audit docs have line-anchored fix snippets you can paste into PRs directly.

## Cross-references

- [`contracts/junoswap-factory/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-factory/DETERMINISTIC_AUDIT.md) — full factory audit, 6 findings.
- [`contracts/junoswap-pair/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-pair/DETERMINISTIC_AUDIT.md) — full pair audit, 8 findings.
- [`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md) — cross-contract methodology and the six recurring patterns extracted from the 9-contract sweep.
- [`docs/AUDIT_BOT_V2_DESIGN.md`](./AUDIT_BOT_V2_DESIGN.md) — the lint-bot proposal that would catch most of the LOW findings (Patterns C/D/E/F) automatically on PR.
- Uniswap v2 first-depositor inflation reference: [github.com/Uniswap/v2-core/issues/8](https://github.com/Uniswap/v2-core/issues/8) (closed 2018) — the original issue that motivated the `MIN_LIQUIDITY` patch in `UniswapV2Pair.sol`.

---

*Apache-2.0. Drafted by Cascade / VairagyaNodes. Anchor: contract source on `origin/main` as of 2026-05-14.*
