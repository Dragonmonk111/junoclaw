# Genesis Bud Onboarding — Dimi (#1 of 13)

**Status**: DRAFT — Execute after Proposal #373 passes (voting ends March 24, 2026)
**Recipient**: Dimi (testnet ops, said "gg @Dragonmonk111")

---

## Two-Tier Trust Model

JunoClaw's governance has two distinct tiers:

### The 13 (depth 1) — Governance + Infrastructure

Genesis (DragonMonk) distributes all weight to **13 buds**. Genesis retains only **3/10000 (symbolic)** + wasmd admin (emergency only). **Genesis loses voting power after budding.**

The 13 buds:

- **Governance**: Full voting weight — 7/13 quorum, 9/13 supermajority for code upgrades
- **Infra co-stewardship**: Shared access to deploy tooling, testnet ops, server infrastructure
- **Each bud is soulbound** — non-transferable, bound to one wallet address
- **Prunable** — DAO can `BreakChannel` to revoke a branch if trust breaks

The 13 are the governance. Not 14. Genesis is out.

**Multisig**: 5-of-13 threshold for all deploy, upgrade, and fund operations.

### Tier 2 — The Branches (depth 2+)

Once the 13 genesis buds are filled, the tree **stops being linear and starts branching**:

- Each depth-1 bud can bud once → their bud becomes a **branch root**
- Branch roots can bud further → recursive tree growth
- Branches get **governance weight only** — no infra access by default
- Infra access for branch members requires a DAO vote from the Root Ring

---

## Trust-Tree Structure

```
═══════════════════════════════════════════════════
  ROOT RING (depth 0-1) — infra + governance
═══════════════════════════════════════════════════

  DragonMonk (genesis, depth 0)  ← 3/10000 symbolic after budding
  ├── Dimi (bud #1)      ← testnet builder, first genesis bud
  ├── [bud #2]           ← TBD
  ├── [bud #3]           ← TBD
  │   ...
  ├── [bud #12]          ← TBD
  └── [bud #13]          ← genesis can re-enter here if invited by the 13

═══════════════════════════════════════════════════
  BRANCHES (depth 2+) — governance only
═══════════════════════════════════════════════════

  Dimi (depth 1)
  └── Dimi's bud (depth 2)       ← governance weight
      └── their bud (depth 3)    ← governance weight
          └── ...                ← tree grows

  [bud #2] (depth 1)
  └── their bud (depth 2)        ← governance weight
      └── ...
```

The Root Ring is the **hub**. After depth 1, each bud branches independently — no longer linear.

---

## Genesis Sunset

**Genesis loses voting power after budding.** This is automatic, not optional. The moment 13 buds are distributed, genesis drops to 3/10000 symbolic weight. The 13 govern.

Genesis retains wasmd admin for true emergencies — revocable by supermajority (9 of 13).

## The Seat Rule

Any of the 13 can leave. One rule: **you must pass your bud before you sunset.** No seat is ever lost. The tree always has 13 active governance members.

- Genesis distributes buds #1 through #13 → genesis loses voting power automatically
- Any of the 13 can sunset — pass the bud first, then leave
- Genesis can re-enter as **#13** only if a sitting member offers a bud

The founder is already out by design. If the 13 invite them back — that's governance working as intended.

---

## Mnemonic Lifecycle

**Phase 1 — Bootstrap (now):**
Root 0 holds the deploy wallet mnemonic. Sealed backup delivered to Bud #1 (Dimi) via `bud-seal` (X25519 + ChaCha20-Poly1305).

**Phase 2 — Multisig Migration (after 13 buds filled):**
Deploy wallet drained into CW3 multisig (5-of-13). No single mnemonic holder.

**Phase 3 — Mnemonic Destruction:**
Original mnemonic destroyed. All ops go through multisig governance.

---

## Pre-Budding Checklist

- [x] Proposal #373 passes with YES majority
- [x] Dependency cleanup pushed to GitHub (commit bc5067c)
- [x] All 86 contract tests passing
- [x] Legal caveats doc finalized (docs/LEGAL_CAVEATS.md)
- [ ] Medium article published
- [ ] jclaw-token contract deployed on uni-7
- [ ] Collect Dimi's juno1... address

---

## Onboarding Steps

1. **Collect address**: Ask Dimi for his `juno1...` wallet address
2. **WeightChange proposal**: Submit DAO proposal to add Dimi with governance weight
3. **DAO votes**: Existing members (initially just root) approve
4. **Execute**: Proposal passes → Dimi is now a voting member
5. **TokenRecord**: Once jclaw-token contract is live, mint his soulbound bud:
   - `holder`: Dimi's juno1... address
   - `parent`: DragonMonk's address (root)
   - `depth`: 1
   - `budded`: false (he hasn't passed his bud yet)
   - `revoked`: false

---

## What a Genesis Bud Gets (Depth 1 — The 13)

- **Vote** on all DAO proposals (config changes, weight changes, code upgrades, sortition, payments)
- **Propose** new actions to the DAO
- **Bud once** — invite one trusted person into the tree (creates a branch)
- **GitHub collaborator** access
- **Testnet deploy access** — shared deploy wallet for uni-7 ops
- **Server access** — dedicated SSH user on infrastructure nodes
- **Infra modulation** — can participate in deploy, upgrade, and operational decisions

## What a Branch Bud Gets (Depth 2+ — Branches)

- **Vote** on DAO proposals (governance weight)
- **Propose** actions to the DAO
- **Bud once** — extend their branch
- **No infra access** by default — must be granted by Root Ring vote

The Root Ring shares the weight of running the project. Branches extend the governance reach without diluting operational control.

---

## Genesis Bud Address Collection

| # | Name | Address | Status |
|---|------|---------|--------|
| 1 | Dimi | `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` | Found — uni-7 genesis (same key as mainnet valoper) |
| 2 | | | |
| 3 | | | |
| 4 | | | |
| 5 | | | |
| 6 | | | |
| 7 | | | |
| 8 | | | |
| 9 | | | |
| 10 | | | |
| 11 | | | |
| 12 | | | |
| 13 | | | |

---

*No seat dies. Pass the bud before you leave. The tree governs itself.*
