# Deterministic Audit — `faucet`

*Apache-2.0. Methodology: [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md). Sister audits: [`junoswap-factory/DETERMINISTIC_AUDIT.md`](../junoswap-factory/DETERMINISTIC_AUDIT.md), [`builder-grant/DETERMINISTIC_AUDIT.md`](../builder-grant/DETERMINISTIC_AUDIT.md), [`agent-registry/DETERMINISTIC_AUDIT.md`](../agent-registry/DETERMINISTIC_AUDIT.md). Anchor: contract source at `contracts/faucet/src/` as of 2026-05-14.*

## Summary (3 lines)

`faucet` is the testnet drip-faucet — admin-funded pool, one claim per address forever, admin can pause/withdraw/update drip-amount. The contract is the **cleanest** of the four `B`-cluster audited contracts (factory / builder-grant / faucet / agent-registry-revisit). Single MEDIUM (`Fund {}` silently absorbs non-config-denom tokens — same cross-cutting pattern as `builder-grant` F1). Everything else is LOW or design-acknowledged.

## Methodology

Same 4-axis deterministic-scrutiny benchmark.

## Findings

| ID | Severity | Title | Affected |
|----|----------|-------|----------|
| F1 | **MEDIUM** | `Fund {}` accepts any denom; non-configured denoms become unrecoverable | `contract.rs:115-133` |
| F2 | LOW | One claim per address forever — no admin reset, no time-based renewal | `contract.rs:74-77` (design) |
| F3 | LOW | Sybil resistance is per-address; trivially defeated by new addresses | `contract.rs:74-77` (design) |
| F4 | LOW | `total_claims += 1` is unchecked arithmetic | `contract.rs:95` |
| F5 | LOW | No `migrate` entry_point | `contract.rs` |
| F6 | LOW | `query_has_claimed` returns `claimed_at_block` as `Option<u64>` but no way to verify the block is still valid (chain rollback edge) | `contract.rs:249-258` |

## F1 — `Fund {}` silently absorbs tokens of any denom (MEDIUM)

Identical pattern to [`builder-grant` F1](../builder-grant/DETERMINISTIC_AUDIT.md#f1--fund--silently-absorbs-tokens-of-any-denom-medium). `execute_fund` at `contract.rs:115-133` reads:

```rust
let total_funded: u128 = info
    .funds
    .iter()
    .filter(|c| c.denom == config.denom)
    .map(|c| c.amount.u128())
    .sum();
```

The `.filter` accounts only the configured denom in the `amount` attribute, but `info.funds` has already been moved to the contract by wasmd before the entry-point runs. Any non-config-denom tokens are permanently absorbed into the contract balance with no recovery path — `execute_withdraw` at `contract.rs:135-165` builds `BankMsg::Send` with `config.denom` only.

### Severity rationale — MEDIUM

Same severity reasoning as builder-grant F1: silent absorption, accounting misreports `0`, sender loses funds, no recovery. The cross-cutting nature of this pattern (now in two contracts) makes it more important to fix — and to add a contract template / lint that catches future occurrences.

### Suggested fix

Identical shape to builder-grant F1's suggested fix. Validate `info.funds` denom inside `execute_fund` and reject any non-matching tokens:

```rust
fn execute_fund(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    for coin in &info.funds {
        if coin.denom != config.denom {
            return Err(ContractError::UnexpectedDenom {
                expected: config.denom.clone(),
                received: coin.denom.clone(),
            });
        }
    }
    
    let total_funded: u128 = info.funds.iter().map(|c| c.amount.u128()).sum();
    
    if total_funded == 0 {
        return Err(ContractError::EmptyFunds {});
    }

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("funder", info.sender)
        .add_attribute("amount", total_funded.to_string()))
}
```

Add `UnexpectedDenom { expected, received }` and `EmptyFunds {}` to `error.rs`.

### Cross-cutting recommendation

Two contracts (faucet, builder-grant) now have this exact pattern. Worth promoting to a cross-contract finding in [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md) under a new "Cross-cutting patterns" section. Future contracts that take `info.funds` should follow the explicit-rejection pattern by default.

## F2 — One claim per address forever, no admin reset (LOW / design)

`CLAIMED: Map<&Addr, u64>` stores the block height at first claim. `execute_claim` rejects with `AlreadyClaimed` if the address is in the map. There is no `ExecuteMsg::ResetClaim { address }` or time-based renewal.

### Why this is LOW (not MEDIUM)

The contract description explicitly says "100 JUNOX per first-time builder" — one-shot is the documented behavior. The lack of an admin reset is a deliberate design choice (prevents admin from re-issuing to favored addresses).

### When this could become an issue

- Testnet reset: if the chain resets state, claim history clears (because contract state clears). Fine.
- Genuine user re-claim need: e.g., user loses their key and needs to claim from a new address. Contract has no path for this. Off-chain coordination + admin `Fund {}` to the new address is the workaround.
- Operator wants to top-up specific testers: not supported. Workaround: admin `Withdraw` to themselves, then `BankMsg::Send` separately.

### Suggested doc-only fix

Add to the contract's `lib.rs` or a README:

```rust
//! # Faucet contract
//!
//! One claim per address, forever. By design — no admin reset, no
//! time-based renewal. If a user loses their key and needs to claim
//! from a new address, coordinate off-chain with the admin.
//!
//! For per-builder repeat grants based on verified work, see the
//! sister `builder-grant` contract.
```

## F3 — Sybil resistance is per-address only (LOW / design)

A user can create N addresses and call `Claim {}` from each, receiving `N × drip_amount` total. The contract has no on-chain mechanism to detect or prevent this (no proof-of-personhood, no IP-rate-limit, no captcha).

### Why this is LOW (not MEDIUM)

This is a known faucet-design limitation across all on-chain faucets. Standard mitigations are off-chain (captcha gateway, IP rate-limit, OAuth via Discord/Twitter handle). The contract cannot enforce them. The cost of sybil-attack on a testnet faucet is bounded by `total_balance / drip_amount` — a malicious actor drains the pool, admin refills, ad infinitum. Operational rather than security-critical.

### Suggested doc-only fix

Same as F2 — add to README:

```
This contract enforces "one claim per address." It does NOT enforce
"one claim per person." Sybil resistance must be supplied off-chain
(captcha, OAuth, IP rate-limit at the frontend).
```

## F4 — `total_claims += 1` unchecked (LOW)

`contract.rs:95`. u64 overflow is unreachable in any practical scenario. Stylistic inconsistency with `checked_add` elsewhere in the codebase. Trivial 3-LoC fix:

```rust
config.total_claims = config.total_claims
    .checked_add(1)
    .ok_or(ContractError::Overflow {})?;
```

## F5 — No `migrate` entry_point (LOW)

Same as sister contracts. Recommend empty stub.

## F6 — `claimed_at_block` returned without rollback awareness (LOW / cosmetic)

`query_has_claimed` returns `Option<u64>` for the block height at claim. If the chain has rolled back below that block (extremely rare on Cosmos, but possible during testnet rewinds), the value points to a non-existent block. The query doesn't return a "still valid" assertion.

### Why this is LOW

- Chain rollbacks below committed state are very rare in Cosmos production. Testnet rewinds happen but the contract state would also reset.
- The block height is informational only — the contract logic doesn't depend on it being current.
- Frontends that query `claimed_at_block` should treat it as "the height when this was recorded" not "this block still exists."

### Suggested doc fix

Doc comment on `ClaimStatusResponse.claimed_at_block`:

```rust
pub struct ClaimStatusResponse {
    pub address: Addr,
    pub claimed: bool,
    /// The block height at which the claim was recorded. Informational
    /// only — may point to a block that no longer exists if the chain
    /// has been rewound below this height. Does not affect the
    /// `claimed: bool` flag (which reflects the current contract state).
    pub claimed_at_block: Option<u64>,
}
```

## Severity-weighted summary

- **MEDIUM (1):** F1. **Pre-mainnet block.** Same pattern as `builder-grant` F1 — should land at the same time as a coordinated PR across both contracts.
- **LOW (5):** F2-F6. Polish before mainnet but no block. F2 and F3 are doc-only changes; F4 is a 3-LoC change; F5 is a 5-LoC empty stub; F6 is a doc comment.

## Recommended sprint sequencing

1. **F1 fix (coordinated with `builder-grant` F1)** — Single PR that fixes both contracts' `execute_fund` with the same shape. ~30 LoC + 4 error variants + 4 tests across both contracts. **Half-day PR.**
2. **F4 + F5** — Bundle into the same polish PR as F2/F3/F6 doc updates. ~15 LoC total. **One-hour PR.**

Total: ~1 day of focused work alongside the `builder-grant` cleanup.

## Cross-cutting pattern: "info.funds without explicit denom validation"

This audit + the `builder-grant` audit have now both flagged the same pattern. Recommend a `memory/deterministic-audit-benchmark.md` update to add a "Cross-cutting patterns" section listing recurring concerns. Initial entries:

| Pattern | Severity | Contracts affected | Mitigation |
|---------|----------|---------------------|------------|
| `info.funds` accepted without denom validation in `execute_fund` | MEDIUM | `faucet`, `builder-grant` | Explicit loop checking `coin.denom == config.denom`; reject mismatches |
| Full-scan range queries without `Bound::inclusive` on pagination start | LOW-MEDIUM | `junoswap-factory`, `builder-grant`, `agent-registry` | Use `Bound::exclusive(start)` to skip at storage layer |
| Unchecked `+= 1` accumulators | LOW | `junoswap-factory`, `builder-grant`, `faucet` | `checked_add` + `Overflow` error variant |
| Missing `migrate` entry_point | LOW | All six audited so far | Empty stub at minimum |

A future "audit-bot v2" PR-time CI (per [`memory/SESSION_PROTOCOL.md`](../../memory/SESSION_PROTOCOL.md) §3 T10) would naturally check for these patterns.

## Cross-references

- [`builder-grant/DETERMINISTIC_AUDIT.md`](../builder-grant/DETERMINISTIC_AUDIT.md) F1 — same MEDIUM, same fix shape.
- [`junoswap-factory/DETERMINISTIC_AUDIT.md`](../junoswap-factory/DETERMINISTIC_AUDIT.md) — sister audit.
- [`memory/deterministic-audit-benchmark.md`](../../memory/deterministic-audit-benchmark.md) — methodology + cross-contract finding index.

---

*Audited 2026-05-14 by Cascade/VairagyaNodes deterministic-scrutiny pass. Code anchor: `contracts/faucet/src/{contract,state,msg,error}.rs`. Apache-2.0.*
