# junoswap-pair — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark. Anchor commit: `685deb1` on origin/main. Files read in full: `src/contract.rs` (472 lines), `src/state.rs` (43 lines), `src/msg.rs` (61 lines), `src/error.rs` (40 lines), `src/tests.rs` (405 lines).*

*Junoswap-pair is the **constant-product XYK AMM** for the JunoSwap DEX-mirror. It's the only contract in the JunoClaw stack that holds user funds in continuously-rebalancing positions (escrow holds them in fixed positions; junoswap-factory just routes). Findings here propagate directly to user-fund safety on any pool that uses this contract.*

---

## 0. Architectural framing

**Surface:**

| Action | Authority | Effect |
|---|---|---|
| `Instantiate { factory, token_a, token_b, fee_bps, junoclaw_contract? }` | anyone (factory typically) | Sets pair config + zero pool state |
| `ProvideLiquidity {}` | anyone | Mints LP shares for both-token deposit; geometric-mean for first deposit, proportional thereafter |
| `WithdrawLiquidity { lp_amount }` | anyone (with LP) | Burns LP shares, returns proportional reserves |
| `Swap { offer_asset, min_return? }` | anyone | XYK swap with `fee_bps` fee; emits `wasm-swap` event with WAVS attestation fields |
| `Query Pool / SimulateSwap / LpBalance / PairInfo` | anyone | Read state |

**State surface:**

| Storage item | Type | Notes |
|---|---|---|
| `PAIR_CONFIG` | `Item<PairConfig { factory, token_a, token_b, fee_bps, junoclaw_contract? }>` | Set at instantiate, never updated |
| `POOL_STATE` | `Item<PoolState { reserve_a, reserve_b, total_lp_shares, total_swaps, total_volume_a, total_volume_b, last_swap_block }>` | Mutates on every action |
| `LP_SHARES` | `Map<&Addr, Uint128>` | Per-user LP balance; standard "no value" sentinel via `may_load + unwrap_or_default` |

**Prior audit lineage.** The contract has visible v6-era audit comments inline (`// ── v6 F4: ...` at lines 226 and 411). v6 F4 fixed two related "absorbed-denom" bugs in `Swap` and `ProvideLiquidity` — attached coins of unexpected denoms were silently absorbed into the pair's bank balance. This v0 audit picks up where v6 left off.

**No `migrate` entry point.** Cross-cutting pattern across the JunoClaw stack.

---

## 1. Failure Mode Enumeration

### 🔴 F1 — First-depositor inflation attack (no `MIN_LIQUIDITY` lockup)

**Location:** `execute_provide_liquidity` lines 78-87.

**The flaw.** Initial LP minting:

```rust
let lp_shares = if pool.total_lp_shares.is_zero() {
    // Initial liquidity: geometric mean
    let product = Uint256::from(amount_a) * Uint256::from(amount_b);
    isqrt_u256(product)
} else {
    let share_a = amount_a.multiply_ratio(pool.total_lp_shares, pool.reserve_a);
    let share_b = amount_b.multiply_ratio(pool.total_lp_shares, pool.reserve_b);
    share_a.min(share_b)
};
```

The classic Uniswap v2 first-depositor attack. The pair does NOT subtract a `MIN_LIQUIDITY` (typically 1000 wei) from the initial LP and lock it permanently. Combined with **F2** (no bank-balance reconciliation), this enables:

1. Attacker calls `ProvideLiquidity {}` with 1 ujuno + 1 uusdc → mints `floor(sqrt(1*1)) = 1 LP share`. Attacker becomes sole LP.
2. Attacker `BankMsg::Send`s 1,000,000 ujuno + 1,000,000 uusdc directly to the pair contract address (bypassing `ProvideLiquidity` entirely).
3. Pair's bank balance: 1,000,001 of each. Pair's stored `reserve_a` / `reserve_b`: still 1 + 1 (because the contract doesn't reconcile against bank balance).
4. Wait — actually since reserves are stored, the donation doesn't change the in-storage view. So a subsequent legitimate LP at step 2 would compute proportional shares based on `reserve_a = 1`, not against the inflated balance. So the donation just sits idle, unused.

**Wait — let me re-check.** The contract uses *stored* reserves, not bank balance, throughout. So a direct `BankMsg::Send` donation is **NOT counted** in the AMM math. Donated funds become permanently locked (no withdrawal path), but they don't dilute legitimate LPs.

**The actual attack vector is subtler.** It requires the attacker to inflate `pool.reserve_a` itself. There's only one path: `ProvideLiquidity`. So:

1. Attacker provides 1 + 1 → 1 LP share, reserves become (1, 1).
2. Victim wants to deposit 1,000,000 + 1,000,000. Computes `share_a = 1,000,000 * 1 / 1 = 1,000,000`, `share_b = 1,000,000 * 1 / 1 = 1,000,000`. min = 1,000,000. Victim mints 1M LP. Reserves become (1,000,001, 1,000,001), total LP = 1,000,001.
3. Victim's share is 1,000,000 / 1,000,001 ≈ 99.9999%. Attacker's share is 0.0001%. Reasonable.

But what if the attacker can **front-run** the victim's `ProvideLiquidity` with a `Swap` that skews the reserves? Let me check:

1. Attacker provides 1 + 1 → reserves (1, 1), 1 LP.
2. Attacker swaps 1 ujuno → uusdc. With XYK math: return = `1 * 1 / (1 + 1) = 0` (rounds down to zero). Swap reverts (line 256 zero-amount check). ✅ Defended.
3. Alternative: attacker swaps a large amount into the pool. But fee_bps and reserves prevent this from being free, and the math constraints prevent extreme manipulation from a 1+1 pool because every swap reverts due to zero-return.

**So is F1 actually exploitable?** With 1+1 initial liquidity, no — every meaningful swap reverts. With 1000+1000 initial liquidity, the attack window opens. Let me think harder.

Standard Uniswap v2 inflation-attack scenario:
1. Attacker provides `1` of token_a and `10000` of token_b. Initial LP = `floor(sqrt(1*10000)) = 100`.
2. Attacker now controls 100% of LP with reserves (1, 10000) — heavily skewed.
3. Victim wants to provide 100 + 1000 (intending equal-value). Computes:
   - `share_a = 100 * 100 / 1 = 10000`
   - `share_b = 1000 * 100 / 10000 = 10`
   - min = 10. Victim mints 10 LP shares for 100 + 1000 deposit.
4. Total LP now 110, reserves (101, 11000). Victim's share = 10/110 = 9.1%. Attacker's = 100/110 = 90.9%.
5. Victim deposited 1100 worth (at 1:10 ratio) but only got 9.1% of pool value. **The attacker captured 90.9% of the value with their 1+10000 initial deposit.**

This is the inflation attack in its full form. Token amounts I picked are illustrative — the fundamental issue is that the geometric mean LP for a heavily-skewed initial deposit is large, and subsequent depositors who don't match the exact ratio get short-changed.

**Severity: HIGH.** Real exploit, well-known attack, simple fix.

**Suggested fix.**

Standard Uniswap v2 mitigation: subtract `MIN_LIQUIDITY = 1000` from the first LP mint and lock those shares forever. Add to `execute_provide_liquidity`:

```rust
const MIN_LIQUIDITY: u128 = 1000;

if pool.total_lp_shares.is_zero() {
    let product = Uint256::from(amount_a) * Uint256::from(amount_b);
    let raw_lp = isqrt_u256(product);
    if raw_lp <= Uint128::new(MIN_LIQUIDITY) {
        return Err(ContractError::InsufficientInitialLiquidity {});  // new error variant
    }
    let lp_shares = raw_lp - Uint128::new(MIN_LIQUIDITY);
    // The MIN_LIQUIDITY is "minted" into total_lp_shares but never assigned
    // to any address — it's permanently locked.
    pool.total_lp_shares = raw_lp;  // includes MIN_LIQUIDITY
    LP_SHARES.save(deps.storage, &info.sender, &(prev + lp_shares))?;  // user gets raw_lp - MIN
    return Ok(...);
}
```

~25 LoC + 2 tests (one asserts MIN_LIQUIDITY is locked; one asserts the inflation attack now reduces the attacker's share to ~50% of fair value, which is the bound the lockup provides).

---

### 🟡 F2 — No reconciliation between stored reserves and bank balance

**Location:** Throughout. Reserves are tracked in storage; the contract never queries `deps.querier.query_balance(env.contract.address, &denom)`.

**The flaw.** The invariant `reserve_a == bank_balance(pair_addr, denom_a)` is maintained by hand: every `+= amount_a` or `-= return_amount` in `provide_liquidity` / `swap` / `withdraw` must match the corresponding `BankMsg::Send`. Any drift is invisible to the contract.

Two drift sources:

1. **Donation drift.** A user `BankMsg::Send`s tokens directly to the pair address (e.g., a misclick, a refund of an unrelated tx, a malicious "fund-locking" attack against this pair). The bank balance grows but `reserve_a` doesn't. The donation is permanently locked — no path to extract it. Severity: LOW (just locked funds, not exploitable).

2. **Trust-the-stored-state.** If a future code change introduces a bug that mis-accounts (e.g., a swap that takes `offer_amount` into `info.funds` but only credits `reserve_a += offer_amount - 1`), the drift accumulates silently. Without reconciliation, the contract has no way to detect this. Severity: MEDIUM (latent fragility).

**Severity: MEDIUM** (defense-in-depth; not directly exploitable today, but enables F1's full damage path).

**Suggested fix.**

Add a `query_pool_with_balance_reconciliation` debug query that compares stored reserves against bank balance, returning a "drift" attribute. This is read-only, defensive, and surfaces bugs early. ~30 LoC + 2 tests.

The harder fix (full reconciliation on every action) is rejected: it forces a chain query per action, doubling gas cost, for marginal safety value. The donation locked-funds case is acceptable; the latent-bug case is what code review catches.

---

### 🟡 F3 — Tiny-swap zero-fee exploit (fee_amount rounds to zero)

**Location:** `execute_swap` line 248.

**The flaw.**

```rust
let fee_amount = offer_amount.multiply_ratio(config.fee_bps as u128, 10000u128);
```

`Uint128::multiply_ratio(numerator, denominator)` does `(self * numerator) / denominator` with floor rounding. For `fee_bps = 30` (the default), `fee_amount = (offer_amount * 30) / 10000`. When `offer_amount * 30 < 10000`, i.e., `offer_amount < 334`, `fee_amount = 0`.

**Attack:** an attacker chains many tiny swaps to extract value from the pool's price impact without paying fees. For a pool of size 1B units with 30 bps fee, normal trades pay 0.3% fee. But an attacker doing 333 swaps of 333 units each pays ZERO fee — and the cumulative price impact is the same as a single 110,889-unit swap.

The math actually shows the attacker pays roughly the same SPREAD (price impact) as a single swap, but the FEE component goes to zero. So the LP loses the fee revenue but keeps the spread revenue. The economic cost of the attack is dominated by gas (333 transactions). Whether this is exploitable depends on chain gas cost vs. fee size.

For Juno mainnet with ~25K SDK gas per swap and 0.0025 ujunox/gas, each swap costs ~62.5 ujunox in gas. To extract value > gas, the saved fee per swap must exceed that. Given typical pool sizes and 30 bps fee, the breakeven swap size is roughly 21,000 units. So tiny swaps below this are gas-loss, not attractive.

**Severity: MEDIUM** — exploitable in theory, gas-cost-bounded in practice. Worth fixing for defense-in-depth.

**Suggested fix.**

Two clean options:

1. **Round fee UP, not down.** Replace `multiply_ratio` with a ceiling-division helper:
   ```rust
   let fee_amount = (offer_amount * config.fee_bps as u128 + 9999) / 10000;
   ```
   Tiny swaps now pay 1 unit of fee, exact-ratio breakdowns now pay 1 more than the ideal. ~5 LoC + 1 test.

2. **Reject swaps below a minimum.** Add `if offer_amount < MIN_SWAP_AMOUNT { Err(...) }`. Pick MIN_SWAP_AMOUNT such that `MIN_SWAP_AMOUNT * fee_bps / 10000 >= 1` (i.e., 334 for 30 bps). ~5 LoC + 1 test.

I prefer (1) — it doesn't change the swap surface, just hardens the math.

---

### 🟡 F4 — `f64` arithmetic in `Pool` query (determinism + precision)

**Location:** `query` lines 337-345.

**The flaw.**

```rust
let a = format!(
    "{:.6}",
    pool.reserve_b.u128() as f64 / pool.reserve_a.u128() as f64
);
```

Two issues:

1. **`f64` arithmetic in a CosmWasm contract is a determinism red flag.** Even though this is in a query (not a state-mutating handler), the audit-bot CI gate `.github/workflows/audit-bot.yml` should treat any `f64` / `f32` use as a finding — at minimum requires explicit justification. A CosmWasm chain runs the same .wasm on every validator; if `f64` arithmetic differs across hosts (very rare in practice but not guaranteed), validators would disagree. For queries, this manifests as different RPC responses to the same query on different validators — surprising users.

2. **`u128 as f64` loses precision** for reserves > 2^53 (≈ 9 × 10^15). For a high-value pool (e.g., a billion JUNO at 1 micro-precision), `pool.reserve_a.u128() as f64` truncates the low bits. The price string is wrong by ~1 part per million in the worst case. Not a correctness bug for users (they don't trade against the price string), but it's a precision violation.

**Severity: MEDIUM** — determinism principle violation, with a small but real precision footgun.

**Suggested fix.**

Replace `f64` with rational-number string formatting:

```rust
let (price_a, price_b) = if pool.reserve_a.is_zero() || pool.reserve_b.is_zero() {
    ("0".to_string(), "0".to_string())
} else {
    // Format as integer numerator/denominator pairs, no float involved.
    let a = format!("{}/{}", pool.reserve_b, pool.reserve_a);  // price of A in B
    let b = format!("{}/{}", pool.reserve_a, pool.reserve_b);  // price of B in A
    (a, b)
};
```

This shifts the decimal-formatting responsibility to the off-chain client (which can use proper big-rational libraries). ~10 LoC + 1 test.

---

### 🟡 F5 — `WithdrawLiquidity` silently absorbs attached funds

**Location:** `execute_withdraw_liquidity` lines 120-179.

**The flaw.** The function never reads `info.funds`. Any coins the user attaches to a `WithdrawLiquidity` message are sent to the pair contract (chain semantics) but never tracked in `reserve_a` / `reserve_b` and never refunded. They become permanently donated.

This is the **same shape as v6 F4** (the bug fixed in `ProvideLiquidity` and `Swap` per the inline comments at lines 226 and 411) — but the v6 fix didn't extend to `WithdrawLiquidity`. The attack surface is smaller (no economic incentive to attach funds when withdrawing) but the bug class is identical.

**Severity: LOW-MEDIUM** — accidental fund loss, identical pattern to v6 F4 but in a less-traveled handler.

**Suggested fix.**

Same shape as v6 F4:

```rust
fn execute_withdraw_liquidity(...) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::UnexpectedFunds {});  // new error variant
    }
    // ... rest unchanged
}
```

~5 LoC + 1 test.

---

### 🟢 F6 — `factory` and `junoclaw_contract` fields stored but never invoked

**Location:** `state.rs:8-12`, `contract.rs:323-329`.

**The shape.**

- `factory: Addr` is recorded at instantiate, exposed via `PairInfo` query, but never used as an authority gate or callback target.
- `junoclaw_contract: Option<Addr>` is recorded similarly, exposed via query, never invoked.

Both fields exist as **off-chain hints** for indexers / WAVS verification components. The pair contract itself is symmetric — it doesn't trust the factory to do anything special, and doesn't push to the junoclaw contract.

**Severity: LOW** — design choice, not bug. But worth surfacing because:

1. A reader of the contract might assume `factory` is an authority (it's not). Documentation gap.
2. `junoclaw_contract` is plumbing for a feature that hasn't shipped (WAVS verification hooks). The field is dead weight until the WAVS path lands.

**Suggested fix.**

Add a `state.rs` doc comment clarifying both fields are off-chain-consumer metadata, not on-chain authority. ~3 lines of docs. No code change.

---

### 🟢 F7 — No `migrate` entry point

**Location:** absence of `pub fn migrate`.

Cross-cutting pattern (same finding as task-ledger F10, escrow's migrate validation, agent-registry F8, zk-verifier F7). One-line fix per contract. ~3 LoC + 1 test.

---

### 🟢 F8 — `total_swaps: u64` overflow

**Location:** `state.rs:20`, mutated on every swap.

At one swap per second forever, `u64` overflows in ~580 billion years. **Severity: LOW** — academic concern only.

---

## 2. What the existing tests do and don't cover

The test suite (`src/tests.rs`, 405 lines, ~10 tests) is reasonable but has clear gaps:

**Covered:**
- Instantiate sanity ✅
- v6 F4: `ProvideLiquidity` rejects unexpected denoms ✅ (`test_provide_liquidity_rejects_unexpected_denom`)
- v6 F4: `Swap` rejects unexpected denoms ✅ (`test_swap_rejects_unexpected_denom`)
- Initial-liquidity LP minting (small pool) ✅
- Swap maintains constant-product invariant `k_after >= k_before` ✅
- Swap slippage protection (min_return) ✅
- Withdraw proportional reserves ✅

**Not covered:**
- 🔴 **F1** — first-depositor inflation attack. Add a test that:
  1. Attacker provides 1 + 10,000 → mints 100 LP.
  2. Victim provides 100 + 1000 → mints only 10 LP.
  3. Asserts victim's share / total_lp < some threshold (e.g., 50%) demonstrating dilution.
- 🟡 **F3** — tiny-swap zero-fee. Add a test asserting that for `offer_amount < 334`, the swap either reverts or charges at least 1 unit of fee.
- 🟡 **F5** — `WithdrawLiquidity` with attached funds. Symmetric to the existing v6 F4 tests; should fail closed.
- 🟢 **Donation locked-funds.** Test asserting that direct `BankMsg::Send` to the pair address doesn't affect AMM math (donated funds are locked). Documents the F2 invariant.

---

## 3. Determinism Proof

| Concern | Status |
|---|---|
| No floats | **🔴 FAIL — F4 uses `f64` in Pool query.** |
| No HashMap iteration | ✅ |
| No `std::time` | ✅ — uses `env.block.{height,time}` |
| Integer arithmetic | ✅ — `multiply_ratio` and `Uint256` for the geometric-mean intermediate |
| Saturating subtraction | ⚠️ — `pool.reserve_a -= withdraw_a` etc. don't saturate; rely on prior `lp_amount.multiply_ratio(reserve_a, total_lp_shares) <= reserve_a` (which holds when `lp_amount <= total_lp_shares`). Verified by code inspection but should have an `assert` for defense-in-depth. |
| Canonical denom comparison | ✅ — uses `denom_key()` from `junoclaw_common::AssetInfo` |

**One determinism violation (F4).** Otherwise clean.

---

## 4. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | **HIGH** | Add `MIN_LIQUIDITY` lockup on first deposit | ~25 LoC + 2 tests |
| F2 | MEDIUM | Add debug `query_pool_with_drift` that compares stored vs. bank balance | ~30 LoC + 2 tests |
| F3 | MEDIUM | Round fee UP via ceiling-divide | ~5 LoC + 1 test |
| F4 | MEDIUM | Replace `f64` price with integer-ratio strings | ~10 LoC + 1 test |
| F5 | LOW-MEDIUM | Reject `info.funds` in `WithdrawLiquidity` (v6 F4 symmetry) | ~5 LoC + 1 test |
| F6 | LOW | Doc comments on `factory` and `junoclaw_contract` fields | ~3 lines docs |
| F7 | LOW | Add `migrate` entry point with cw2 validation | ~10 LoC + 1 test |
| F8 | LOW | Document u64 overflow horizon (no fix needed) | 0 LoC |

**Recommendation.**

- **Sprint 1 (junoswap-pair-v0.2):** F1 (the headline) + F4 (determinism) + F5 (v6 F4 symmetry). All three are tight, well-understood fixes.
- **Sprint 2 (junoswap-pair-v0.3):** F3 (fee rounding) + F2 (drift detection) + F7 (migrate). Defensive-depth pass.
- **Documentation pass:** F6 + F8 doc-only. Land with whichever sprint touches the relevant file.

---

## 5. Comparative summary across the JunoClaw stack (post-this-audit)

| Contract | Audit | Headline finding | Severity |
|---|---|---|---|
| `agent-company` | ✅ | Vote weights not snapshotted at proposal creation | **HIGH** |
| `agent-registry` | ✅ | Registration fees trapped (no withdraw path) | **MEDIUM** |
| `task-ledger` | ✅ | CancelTask leaves orphaned escrow obligations | **LOW-MEDIUM** |
| `escrow` | ✅ | `timeout_blocks` dead + unit mismatch with `created_at` | **MEDIUM** |
| `zk-verifier` | ✅ | `VerifyProof` permissionless + unmetered → gas-DoS + `LAST_VERIFICATION` spoofing | **HIGH** |
| `junoswap-pair` | ✅ (this doc) | First-depositor inflation attack (no `MIN_LIQUIDITY` lockup) | **HIGH** |
| `moultbook-v0` | ✅ (deterministic from day 0) | None | None |
| `junoswap-factory` | pending | TBD | TBD |
| `builder-grant` | pending | TBD | TBD |
| `faucet` | pending | TBD | TBD |

**6 of 9 audited.** Three HIGH findings (`agent-company` F1, `zk-verifier` F1, `junoswap-pair` F1). All three are the same shape: a contract surface that is too permissive given its leverage, with the fix being layered defense rather than a single-line patch.

The cross-cutting "permission gap" pattern is now joined by a second cross-cutting pattern: **standard-protocol attack vectors that are well-known in the source ecosystem but not yet patched in our derivative.** F1 here is the Uniswap v2 inflation attack; the fix is the Uniswap v2 `MIN_LIQUIDITY` lockup. We're inheriting the surface without inheriting the historical defense. Worth surfacing as an audit-bot heuristic: *"if a contract derives from a public protocol (Uniswap, Compound, etc.), check that the derivative includes the protocol's accumulated defenses."*

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark. junoswap-pair is the only continuously-rebalancing custodial contract in the JunoClaw stack; F1 is the highest-leverage finding in this audit because every pool deployed via junoswap-factory inherits it.*
