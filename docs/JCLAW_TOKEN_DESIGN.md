# ADR: What Is `$JClaw`? — Credential vs Token

**Status**: Proposed (2026-06-08)
**Context**: Backlog item "`$JClaw` token + escrow + task-ledger + agent-registry contracts"
**Decision drivers**: A contradiction in the existing docs about whether
`$JClaw` is a non-tradeable governance credential or a tradeable economic token.

---

## 1. The contradiction

Two documents in this repo define `$JClaw` in mutually exclusive ways:

| Source | Definition |
|--------|-----------|
| `docs/DIMI_HANDOFF_PLAN.md` (trust-tree model) | **Soulbound, non-transferable credential** — bound to one wallet, prunable via `BreakChannel`, confers governance weight + infra co-stewardship. "Each bud is soulbound — non-transferable, bound to one wallet address." |
| `docs/GENESIS_BUDS_ARCHITECTURE.md` §114 (Phase 4) | **Tradeable CW20** with a distribution plan: 30% to 13 buds, 20% community airdrops, 20% treasury, 15% vesting dev fund, 10% WAVS staking rewards, 5% Junoswap liquidity. |

A token with airdrops, vesting, and liquidity pools is tradeable by
definition. A soulbound credential cannot have a liquidity pool. **These
cannot both be `$JClaw`.**

---

## 2. Key finding: the governance credential already exists

Governance weight in JunoClaw is **not** token-based today. It lives inside
`agent-company` as an internal member roster:

```rust
// contracts/agent-company — MemberInput { addr, weight, role }
MemberInput { addr: alice, weight: 6000, role: MemberRole::Human }
MemberInput { addr: bob,   weight: 4000, role: MemberRole::Agent }
```

Mutated only via `WeightChange` governance proposals. This roster is,
functionally, the soulbound credential:

- **Non-transferable by construction** — there is no transfer function; it is
  contract state, not a balance you can move.
- **Prunable** — `WeightChange` / `UpdateMembers` removes a member. This is
  the `BreakChannel` operation.
- **One wallet per member** — each `MemberInput` is a single address.
- **Weighted** — exactly the 13 × 769 bps model in
  `GENESIS_BUDS_ARCHITECTURE.md`.

These are **cw4-group semantics** already embedded in `agent-company`.
Conclusion: for *governance*, the "$JClaw soulbound credential" is **already
implemented**. No token contract is required to satisfy the trust-tree.

---

## 3. CW20 vs TokenFactory (only relevant to the *economic* token)

If — and only if — a *tradeable* utility/fee/staking token is wanted, the two
standard options are:

| Dimension | CW20 (contract token) | TokenFactory (native denom) |
|-----------|----------------------|-----------------------------|
| Type | `cw20-base` smart contract | Native bank denom `factory/{addr}/jclaw` |
| Transfer control | Own the `execute` entry point; *can* override `Transfer`/`Send` | Freely transferable via bank `MsgSend` — no block on stock Juno |
| Soulbinding | Possible but fights the standard; confuses wallets/indexers | Effectively impossible without bank send-restriction middleware |
| Wallet UX | Needs CW20-aware view | Native in Keplr/Leap; IBC-transferable |
| Gas | Heavier (contract call per transfer) | Cheap (bank module) |
| DEX / LP | Junoswap-compatible directly | Needs native-denom pair support or wrapping |
| Governance | DAO DAO `cw20-staked-balance-voting` | Custom voting module needed |
| Mint / admin | Contract `mint` with cap | `MsgMint` / `MsgBurn` by denom admin |

**Verdict:** TokenFactory is the right choice for a tradeable economic token
(cheap, native, IBC, LP-ready). CW20 is the right choice if programmable
mint/vesting logic is required. **Neither is appropriate for a non-tradeable
credential**: soulbinding a TokenFactory denom is essentially impossible on
stock Juno, and a soulbound CW20 is a deliberately broken token.

---

## 4. Do we need a new standard?

For the **credential**: no new *token* standard — and not CW20/TokenFactory
either. The closest fit is **cw4-group** (membership + weight, non-transferable
by design, DAO DAO compatible).

The trust-tree, however, adds topology that cw4 does **not** model:

- parent → child budding links
- depth (root ring depth 0–1 vs branches depth 2+)
- `BreakChannel` subtree pruning
- the "pass your bud before you sunset" seat rule

So a **custom `cw4`-compatible soulbound credential contract is justified** *if*
the trust-tree is wanted as a standalone, queryable, portable artifact (rather
than living implicitly inside `agent-company`). The design rule:

> Implement the cw4 query interface (`Member`, `ListMembers`, `TotalWeight`,
> `Hooks`) so DAO DAO and existing Cosmos tooling treat it as a normal
> membership source — then layer the tree-specific execute messages
> (`Bud`, `BreakChannel`, `Sunset`) on top.

This gives portability and tooling compatibility without inventing a token
standard nobody else speaks.

---

## 5. Decision

Adopt a **three-layer separation**:

1. **Governance credential = the trust-tree.** Either:
   - **(a)** keep using `agent-company`'s member roster (zero new code), or
   - **(b)** extract a custom `cw4`-compatible `jclaw-credential` contract if
     tree topology + portability are wanted.
   **Soulbound. Never a token.**
2. **Economic token (optional, future) = TokenFactory `ujclaw`.** Only if a
   tradeable utility/fee/staking token is actually needed. Tradeable by
   design — which is acceptable precisely because it is explicitly *not* the
   credential.
3. **Remove the contradiction.** Retire or relabel the "$JClaw CW20 with
   airdrop/LP/vesting" language in `GENESIS_BUDS_ARCHITECTURE.md` §114 as the
   *economic* token, distinct from the credential.

### Recommended near-term path

- **Do not build a CW20/TokenFactory `$JClaw` now.** The governance need is
  already met by `agent-company` membership (layer 1a).
- If a standalone credential artifact is desired, build the
  `cw4`-compatible `jclaw-credential` contract (layer 1b) — this is the only
  net-new contract worth writing, and it should expose cw4 queries.
- Defer the economic TokenFactory token (layer 2) until there is a concrete
  use (fees, staking rewards) that the native `ujuno` / escrow flow does not
  already cover.

---

## 6. Consequences

- **Unblocks the backlog correctly**: escrow, task-ledger, and agent-registry
  are already BUILT; the "$JClaw token" item is reframed as a *credential*
  decision, not a token-build task — avoiding shipping a broken soulbound CW20.
- **Keeps DAO tooling compatibility** via the cw4 query interface if layer 1b
  is chosen.
- **Documentation debt**: `GENESIS_BUDS_ARCHITECTURE.md` §114 and
  `docs/OPEN_ENDS.md` line 23 ("`$JClaw` soulbound token — Contract not
  written") must be updated to reflect this separation.

---

## 7. Open questions for the 13 / Genesis

1. Layer 1a (roster stays in `agent-company`) or 1b (extract a
   `jclaw-credential` cw4 contract)?
2. Is there a concrete economic use that requires layer 2 at all, or does
   native `ujuno` + the existing escrow journal already cover task payments?
3. If layer 2 is wanted later: TokenFactory (default) or CW20 (only if
   vesting/mint-cap logic is needed)?
