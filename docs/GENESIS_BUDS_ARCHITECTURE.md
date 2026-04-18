# JunoClaw Genesis → 13 Buds Architecture

## Overview

```
                    ┌─────────────────┐
                    │  GENESIS (Neo)  │  ← First handler, 100% weight
                    │  weight: 10000  │  ← Can amend entire database
                    └────────┬────────┘
                             │
                     CodeUpgrade Prop
                  (WAVS + Akash + Junoswap)
                             │
                     WeightChange Prop
                    (Genesis → 13 Buds)
                             │
        ┌────┬────┬────┬────┼────┬────┬────┬────┬────┬────┬────┬────┐
        │    │    │    │    │    │    │    │    │    │    │    │    │
       B1   B2   B3   B4   B5   B6   B7   B8   B9  B10  B11  B12  B13
      770  770  770  770  770  770  770  770  770  770  770  770  770
        │                        │                        │
        └──── DAO voting ────────┴──── JCLAW token ───────┘
             (67% for constitutional props: code upgrades + weight changes,
              51% for normal props)
```

## Phase 0: Genesis Amendments — Mar 13-17, 2026 ✅ COMPLETE

**Genesis address**: `juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m` (Neo wallet)

The Genesis address currently holds **100% weight (10,000 basis points)** and is the sole member of the agent-company contract. During this phase, Genesis can:

- Migrate contract code (wasmd admin)
- Submit and auto-pass any proposal (100% > 67% supermajority)
- Wire new infrastructure (Junoswap, Akash, WAVS)
- Amend the member roster
- Set governance parameters

**This is the "pre-budding" phase where Genesis prepares the entire database before distributing power.**

## Phase 1: The Bundled Proposal — Mar 17, 2026 ✅ COMPLETE (Proposal 5)

Genesis submits a **single CodeUpgrade proposal** that bundles:

| Action | What It Does |
|--------|-------------|
| `SetDexFactory` | Wire Junoswap factory into agent-company config |
| `ExecuteContract` | Register WAVS operator in agent-registry |
| `ExecuteContract` | Update Akash deployment config in task-ledger |

Since Genesis has 100% weight, this passes immediately with supermajority.

## Phase 2: The Budding (WeightChange Proposal) — ~Mar 28-31, 2026 ⏳ AFTER PROP PASSES

After the infrastructure is wired, Genesis submits a **WeightChange proposal** to distribute weight to 13 buds:

```json
{
  "create_proposal": {
    "kind": {
      "weight_change": {
        "members": [
          { "addr": "juno1..genesis..", "weight": 10, "role": "human" },
          { "addr": "juno1..bud1..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud2..", "weight": 770, "role": "agent" },
          { "addr": "juno1..bud3..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud4..", "weight": 770, "role": "agent" },
          { "addr": "juno1..bud5..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud6..", "weight": 770, "role": "agent" },
          { "addr": "juno1..bud7..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud8..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud9..", "weight": 770, "role": "agent" },
          { "addr": "juno1..bud10..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud11..", "weight": 770, "role": "agent" },
          { "addr": "juno1..bud12..", "weight": 770, "role": "human" },
          { "addr": "juno1..bud13..", "weight": 770, "role": "agent" }
        ]
      }
    }
  }
}
```

**Weight distribution**: 13 × 770 = 10,010... adjusted:
- 13 buds × 769 = 9,997
- Genesis retains 3 (symbolic, near-zero but keeps membership)
- Total: 10,000

**Genesis retains wasmd admin** (can still migrate contract code in emergencies) but loses voting power. The 13 buds now control governance.

## Phase 3: Post-Budding Governance — ~Apr 1+, 2026 ⏳ AFTER BUDDING

After budding:

| Action | Required | Notes |
|--------|----------|-------|
| Normal proposal (FreeText, ConfigChange) | 51% quorum (5,131 weight → 7 buds) | Simple majority |
| Code upgrade (Migrate, Instantiate, SetDex) | 67% quorum (6,700 weight → 9 buds) | Supermajority |
| Weight change (add/remove buds) | 67% quorum (6,700 weight → 9 buds) | Constitutional supermajority |
| WAVS push | 51% quorum | Off-chain verification tasks |

### Further Budding

The 13 buds can vote to add more buds via WeightChange proposals:

```
13 buds → vote → WeightChange → 21 buds
21 buds → vote → WeightChange → 34 buds
...
```

Each budding redistributes weight. The contract enforces total weight = 10,000.

## Phase 4: JCLAW Governance Token — ~Q2 2026 ⏳ FUTURE

**Future**: Deploy a CW20 token contract (`JCLAW`) that:

1. **Gates proposal creation** — must hold JCLAW to submit proposals
2. **Weights voting power** — JCLAW balance determines vote weight
3. **Enables delegation** — JCLAW holders can delegate to buds
4. **Funds operations** — JCLAW used for task escrow, DEX fees, etc.

This transitions from "fixed-weight DAO" to "token-weighted DAO" — similar to how DAODAO works on Juno.

**Token distribution plan:**
- 30% to 13 initial buds (2.3% each)
- 20% to community pool (airdrops, grants)
- 20% to treasury (controlled by DAO)
- 15% to development fund (vesting)
- 10% to WAVS operators (staking rewards)
- 5% to liquidity pools (Junoswap)

## Juno Chain Governance Proposal

### Context
- **Validator**: Vairagya Node (unbonded, active, waiting position 6)
- **Deposit**: User has sufficient JUNO
- **Discord**: User has validator role in Juno Discord

### Proposal Type
This is a **signaling proposal** (text proposal) on Juno mainnet, not a code execution proposal. It signals the community that:

1. JunoClaw is reviving Junoswap with TEE-attested verification
2. JunoClaw is deploying verifiable AI agents on Akash
3. The WAVS integration brings hardware-attested compute to Juno
4. Vairagya Node validators are building real infrastructure

### Proposal Flow
```
Genesis (Neo) → CodeUpgrade prop (wire Junoswap/WAVS/Akash)
    → WeightChange prop (Genesis → 13 buds)
        → Juno governance prop (signaling, community awareness)
            → Validator re-bonding campaign
```

## Contract Addresses (uni-7 testnet)

| Contract | Address | Code ID |
|----------|---------|---------|
| agent-company v3 | `juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6` | 63 |
| junoswap-factory | `juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh` | 61 |
| junoswap-pair (JUNOX/USDC) | `juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98` | 60 |
| junoswap-pair (JUNOX/STAKE) | `juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s` | 60 |
| agent-registry | `juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7` | 54 |
| task-ledger | `juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46` | 55 |
| escrow | `juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv` | 56 |

## Genesis Address Authority

| Authority | Pre-Budding | Post-Budding |
|-----------|-------------|-------------|
| **wasmd admin** (migrate code) | ✅ Genesis | ✅ Genesis (emergency only) |
| **Voting weight** | 10,000 (100%) | 3 (0.03%) |
| **Proposal creation** | ✅ | ✅ (still a member) |
| **Auto-pass proposals** | ✅ (100% > any quorum) | ❌ (needs 6+ buds) |
| **Wire infrastructure** | ✅ | Via CodeUpgrade prop only |
| **Amend member roster** | ✅ | Via WeightChange prop only |
