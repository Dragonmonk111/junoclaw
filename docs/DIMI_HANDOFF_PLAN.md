# Genesis Bud Onboarding — Dimi (#1 of 13)

**Status**: DRAFT — Execute after Proposal #373 passes (voting ends March 24, 2026)
**Recipient**: Dimi (testnet ops, said "gg @Dragonmonk111")

---

## Two-Tier Trust Model

JunoClaw's governance has two distinct tiers:

### The 13 (depth 1) — Governance + Infrastructure

Genesis (DragonMonk) distributes all weight to **13 buds**. Genesis retains only **3/10000 (symbolic)** + wasmd admin (emergency only). **Genesis loses voting power after budding.**

The 13 buds:

- **Governance**: Full voting weight — 7/13 quorum for normal proposals; 9/13 supermajority for constitutional proposals (`CodeUpgrade` and `WeightChange`)
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

**Genesis is fully out after Path B.** The admin transfer gives Dimi everything — wasmd admin, governance weight, operational control. Genesis retains nothing except the 13 ujunox tombstone on the Neo address.

- No wasmd admin → Genesis cannot migrate, transfer, or clear any contract
- No governance weight → Genesis cannot vote or submit proposals
- No mnemonic → Neo wallet is permanently inert

**Genesis's exit is cryptographic, not ceremonial.** The Neo mnemonic is destroyed. There is no backdoor.

### wasmd Admin Powers (Held by Dimi → Then Multisig)

After Path B, **Dimi holds wasmd admin**. These powers transfer to the 5-of-13 multisig once all buds are seated.

#### What wasmd Admin CAN Do

| Action | CosmWasm Message | When Needed |
|--------|-----------------|-------------|
| **Migrate contract** to patched code | `MsgMigrateContract` | Critical bug, security vulnerability, chain upgrade |
| **Transfer admin** to new address | `MsgUpdateAdmin` | Key rotation, multisig migration |
| **Clear admin** (make immutable) | `MsgClearAdmin` | Final sovereignty — nobody can migrate ever again |

#### What wasmd Admin CANNOT Do

| Action | Why Not |
|--------|---------|
| **Execute contract messages** | Only the contract's internal logic controls execution — admin is not an executor |
| **Change governance weight** | Weight changes require a `WeightChange` proposal + DAO supermajority vote (9/13 once all seats are filled) |
| **Steal escrow funds** | Escrow release is controlled by contract logic, not admin |
| **Override a DAO decision** | Admin is outside the governance loop — it's a maintenance key, not a power key |

### Emergency Contingencies (Dimi's Responsibility)

These are Dimi's calls as the new wasmd admin. Once the multisig is live, they become collective decisions.

**1. Critical Bug / Security Vulnerability**
- A bug is discovered in the agent-company contract (e.g. funds can be drained, state corruption)
- **Admin action**: `MsgMigrateContract` to upload and migrate to patched code
- **Without admin**: If the bug is in the voting logic itself, governance is broken — no way to self-heal
- **This is the #1 reason admin exists**: emergency patching when governance can't fix itself

**2. Key Compromise — A Bud's Key Is Stolen**
- A malicious actor gains control of a bud's wallet and starts submitting harmful proposals
- **Admin action**: Migrate contract to new code with the compromised address removed from members
- **Without admin**: Other buds could vote down malicious proposals, but the attacker can't be removed without a code migration

**3. Multisig Lockout (Post-Phase 3)**
- After the 5-of-13 multisig is live, 9+ members lose keys or go dark — multisig can't reach threshold
- **Admin action**: `MsgUpdateAdmin` to transfer admin to a new multisig with active members
- **Without admin**: Contracts permanently locked — nobody can migrate or update
- **This is the dead man's switch**: admin is the last-resort recovery path

**4. Chain Upgrade Breaking Change**
- Juno upgrades to a new CosmWasm version that breaks existing contract interfaces
- **Admin action**: Migrate contracts to updated code compatible with new chain version
- **Without admin**: Contracts become non-functional on the new chain

**5. All 13 Buds Go Dark**
- Extreme scenario: every bud loses access or abandons the project
- **Admin action**: Migrate contract, reset governance, restart the tree
- **Without admin**: JunoClaw is permanently dead

**6. Governance Deadlock**
- 7/13 can't agree — every proposal fails for months
- **Not an admin emergency.** Admin doesn't break deadlocks. The DAO resolves it politically. Admin only intervenes if the deadlock reveals a contract bug.

### When Admin Should Be Cleared (Final Sovereignty)

Admin is a **temporary training wheel**, not permanent power. It should be cleared when:

```
All 13 buds seated
  + multisig operational (5-of-13)
  + contracts battle-tested on mainnet
  + no critical bugs for 6+ months
  + supermajority (9/13) votes to clear admin
    → Multisig runs MsgClearAdmin on all contracts
    → Contracts become immutable — nobody can migrate
    → Full sovereignty. The tree governs itself.
```

The 13 decide when. The multisig executes it. That's the final act.

## The Seat Rule

Any of the 13 can leave. One rule: **you must pass your bud before you sunset.** No seat is ever lost. The tree always has 13 active governance members.

- Genesis distributes buds #1 through #13 → genesis loses voting power automatically
- Any of the 13 can sunset — pass the bud first, then leave
- Genesis can re-enter as **#13** only if a sitting member offers a bud

The founder is already out by design. If the 13 invite them back — that's governance working as intended.

---

## Admin Transfer — Path B (No Mnemonic Handoff)

**Why Path B**: Instead of passing the Neo wallet mnemonic to Dimi (custody chain, "did you really delete it?" problem), we **transfer wasmd admin on-chain** to an address Dimi generates himself. The Neo mnemonic is then destroyed — nobody needs it.

**Key principle**: Dimi's admin key is born on his machine. It never touches Genesis's machine. Zero custody chain. Cryptographic certainty.

### Wallet Map

| Wallet | Address | Role | Holder |
|--------|---------|------|--------|
| **Neo** (deployer) | `juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m` | Current wasmd admin, 10K governance weight | Genesis (to be retired) |
| **Mother** (treasury) | `juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2` | Testnet treasury (~85K JUNOX) | Genesis |
| **Dimi personal** | `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` | Mainnet staking (1.2M JUNO) | Dimi |
| **Dimi admin** (new) | `juno1<dimi-generates-this>` | New wasmd admin after transfer | Dimi |
| **Akash** (derived from Neo) | `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta` | Compute leases (~63.77 AKT) | Inherited → migrate to dedicated key |

### Contracts To Transfer (5 total, all on uni-7)

| # | Contract | Address | Code ID |
|---|----------|---------|---------|
| 1 | **agent-company v3** | `juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6` | 59 |
| 2 | **junoswap factory** | `juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh` | — |
| 3 | **escrow** | `juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv` | 56 |
| 4 | **agent-registry** | `juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7` | 54 |
| 5 | **task-ledger** | `juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46` | 55 |

---

### Step-by-Step Procedure

#### STEP 0 — Pre-Conditions
```
✅ Prop #373 passes — QUORUM CROSSED (33.83% turnout > 33.4%, 85.31% YES, ends ~March 24)
✅ Dimi confirms he's ready to receive admin
✅ Genesis has Neo wallet mnemonic accessible
```

#### STEP 1 — Transfer Target: Dimi's Known Address

**Target address**: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` (already verified on-chain, 1.2M JUNO)

No waiting required — we transfer admin to Dimi's existing wallet immediately. This is the address he actively uses for staking and governance on Juno mainnet.

**Post-transfer option**: Once Dimi is admin, he can create a dedicated admin wallet and re-transfer on his own terms:

```bash
# Dimi runs this LATER if he wants key separation:
# 1. Generate new wallet on his machine
# 2. Run: npx tsx src/transfer-admin.ts <his-new-juno1-address>
#    (script works for any current admin → new admin transfer)
```

This way there's **zero handoff delay** — admin moves to a known, verified address on Day 1. Dimi decides if/when to separate his staking key from contract admin. His call, his timeline.

#### STEP 2 — Genesis Runs Admin Transfer Script

Genesis runs the transfer script from the Windows dev machine (no junod needed — uses CosmJS):

```bash
# From the junoclaw/wavs/bridge directory:
npx tsx src/transfer-admin.ts juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt
```

This script does everything in one run (5 phases):

| Phase | Action | TXs |
|-------|--------|-----|
| 1 | Verify Neo is admin of all 5 contracts | 0 (queries only) |
| 2 | Transfer wasmd admin to Dimi | 5 |
| 3 | Send 5 JUNOX to Dimi (gas money for WeightChange + future ops) | 1 |
| 4 | Drain remaining JUNOX → Mother wallet, **leave 13 ujunox on Neo** | 1 |
| 5 | Verify all transfers + print final balances | 0 (queries only) |

**Total: 7 TXs, ~0.2 JUNOX gas. One command, everything done.**

The 13 ujunox left on Neo is symbolic — one for each genesis bud. A tombstone.

#### STEP 3 — Both Sides Verify

Genesis and Dimi independently verify the transfer:

```bash
# Anyone can run this (public query, no wallet needed):
npx tsx wavs/bridge/src/verify-admin.ts
```

Expected output:
```
Contract                  Admin                                          Status
─────────────────────────────────────────────────────────────────────────────
agent-company v3          juno1<dimi-address>                            ✅ TRANSFERRED
junoswap factory          juno1<dimi-address>                            ✅ TRANSFERRED
escrow                    juno1<dimi-address>                            ✅ TRANSFERRED
agent-registry            juno1<dimi-address>                            ✅ TRANSFERRED
task-ledger               juno1<dimi-address>                            ✅ TRANSFERRED
```

If any contract still shows Neo's address → re-run the transfer for that contract.

#### STEP 4 — Transfer Governance Weight (Contract-Level)

The wasmd admin transfer (Step 2) changes who can **migrate** contracts. But inside the agent-company, Neo still holds **10,000 governance weight**.

Dimi (as the new wasmd admin) submits a `WeightChange` proposal to redistribute:
- Remove Neo's weight → 0
- Add Dimi's address → initial weight (e.g. 769/10000 for 1-of-13)
- Remaining weight reserved for buds #2-#13

This is a DAO governance action, not a wasmd operation. The contract handles it.

#### STEP 5 — Destroy Neo Mnemonic

The transfer script (Step 2) already drained tokens and transferred admin. Neo now has:
- **0 admin powers** — all 5 contracts point to Dimi
- **0 governance weight** — (redistributed in Step 4)
- **13 ujunox** — symbolic tombstone, one per genesis bud
- **~0 operational value** — nothing left to protect

All that remains is destroying every trace of the Neo mnemonic:

**5a — Files & History**

1. **Delete `deploy/.env`** — contains Neo mnemonic on Genesis's Windows machine
2. **Delete any paper/digital backups** of the Neo mnemonic
3. **Clear terminal/shell history** — `doskey /reinstall` (Windows) or `history -c && history -w` (bash)
4. **Clear clipboard** — copy something else over it

**5b — Akash Console (Chrome)**

The Akash lease runs on **escrow autopilot** — closing the tab does NOT close the lease. It keeps running for ~3-4 months until escrow depletes. Dimi inherits the ability to manage or replace it.

1. **Close the Akash Console tab** in Chrome — you no longer need to monitor the deployment
2. **Clear Akash Console site data** (optional): Chrome → Settings → Privacy → Site Settings → search `console.akash.network` → Clear data
3. **No action needed on the lease itself** — let it run. Dimi decides its fate.

> **Note**: Closing the tab ≠ closing the lease. The lease is an on-chain escrow contract on Akash. It keeps running without any browser, wallet, or signer. Only an explicit `MsgCloseDeployment` TX (signed by the deployer key) can force-close it. Since the Neo mnemonic is being destroyed, nobody can force-close it — it simply runs until escrow hits zero and auto-expires.

> **Top-up address** (save this): `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`
> Anyone can send AKT to this address to extend the lease runway — no mnemonic needed to receive funds. The Akash escrow draws from this balance automatically. To extend by ~1 month, send ~15-22 AKT.

**5c — Keplr Wallet (Chrome Extension)**

> ⚠️ **WARNING**: Check if Keplr also holds the **Mother wallet** (`juno1scpm8...`). If so, do NOT delete Keplr until you've confirmed you have the Mother mnemonic backed up separately. The Mother wallet still holds ~85K+ JUNOX.

1. **Open Keplr** → click the account/profile icon
2. **Identify which wallets are loaded**:
   - If **only Neo** → safe to remove or delete Keplr entirely
   - If **Neo + Mother** → remove Neo account first, keep Mother
   - If **Neo + Mother + personal** → remove Neo only
3. **Remove Neo account from Keplr**:
   - Settings → ⋮ menu next to the Neo account → "Remove Account"
   - Keplr will ask for confirmation — confirm
4. **After Neo is removed**, choose one:
   - **(a) Keep Keplr** — if Mother or other wallets are still loaded
   - **(b) Log out of Keplr** — Settings → Lock Wallet (leaves extension installed but locked)
   - **(c) Delete Keplr extension** — Chrome → Extensions → Remove "Keplr". Only do this if you're certain no other wallets need it.

> **Note**: Removing an account from Keplr only deletes it from the browser extension. It does not affect the on-chain wallet. The 13 ujunox tombstone remains on Neo's address forever regardless.

**5d — Confirm Destruction**

After all sub-steps:
- [ ] `deploy/.env` deleted
- [ ] Paper/digital mnemonic backups destroyed
- [ ] Terminal history cleared
- [ ] Akash Console tab closed (lease runs on autopilot)
- [ ] Neo removed from Keplr
- [ ] Mother wallet mnemonic verified as separately backed up

**Neo wallet is permanently inert.** The 13 ujunox sit there forever.

#### STEP 6 — Post-Transfer Confirmation

```
┌─────────────────────────────────────────────────────────────────┐
│ ADMIN TRANSFER COMPLETE                                         │
│                                                                 │
│ Neo wallet:    RETIRED — mnemonic destroyed, no admin powers    │
│ Dimi wallet:   NEW ADMIN — controls all 5 contracts             │
│ Akash lease:   RUNNING — auto-escrow, 3-4 month runway         │
│ Governance:    Dimi holds weight, can onboard buds #2-#13      │
│ Mother wallet: UNCHANGED — testnet treasury, Genesis holds      │
│                                                                 │
│ Next: Dimi onboards bud #2 via WeightChange proposal            │
└─────────────────────────────────────────────────────────────────┘
```

---

### Why This Is Better Than Mnemonic Handoff

| | Path A (mnemonic handoff) | **Path B (admin transfer)** |
|---|---|---|
| Custody chain | Mnemonic touched 2 machines | Key born on Dimi's machine only |
| Trust assumption | "Genesis deleted their copy" | Cryptographic — nobody else has it |
| Forensic risk | Mnemonic was in IDE, deploy scripts, memory | Never existed outside Dimi's machine |
| Akash wallet | Shared dependency | Independent (escrow runway buys time) |
| Complexity | Simple hand-off | 5 on-chain TXs (~2 min) |
| Security | Medium | **Highest** |

---

## Key Separation: Deploy vs Compute

After admin transfer, the key landscape simplifies:

| Purpose | Key | Holder | Lifecycle |
|---------|-----|--------|-----------|
| Contract admin (wasmd) | Dimi's admin wallet | Dimi | Active → multisig (5-of-13) when 13 buds seated |
| Akash compute leases | Dedicated Akash key (new) | Dimi → Root Ring | Independent — set up at Dimi's pace |
| DAO voting (Dimi) | Dimi's personal key (`juno1s33z...`) | Dimi | His own — never shared |
| Sidecar attestation | Dedicated hot wallet | Each validator | ~1-2 JUNO(X) for gas |

### Akash Considerations

Akash uses the same key derivation path as Juno/Cosmos Hub (`m/44'/118'/0'/0/0`) with a different bech32 prefix (`akash1...`). The current Akash lease was derived from the Neo mnemonic — but since the lease runs on escrow autopilot, there's no urgency to migrate. Dimi sets up a dedicated Akash key when ready (see Akash Lease Handoff section).

---

## Akash Lease Handoff

### What Dimi Inherits

The deploy mnemonic derives the Akash wallet `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`:

| Item | Value |
|------|-------|
| Akash address | `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta` |
| Balance | ~63.77 AKT |
| Active lease | WAVS operator stack (4 containers: operator, aggregator, IPFS, warg-registry) |
| SDL | `wavs/akash.sdl.yml` |
| Est. cost | ~15-22 AKT/month |
| Auto-runway | **~3-4 months** at current escrow |

With the deploy mnemonic, Dimi can:
- **Top up** the Akash escrow (extend runway)
- **Close** existing leases (refund remaining escrow)
- **Create new** leases (deploy additional services or redeploy)
- **Derive the Akash address**: `npx tsx wavs/bridge/src/get-akash-address.ts`

### Escrow Runway — Why There's No Rush

**Akash leases keep running without the mnemonic.** Escrow is deducted automatically per-block — no signatures needed. The lease only closes when escrow hits zero.

This means the deploy mnemonic can be destroyed **before** migrating to a new Akash key. The old lease provides a natural buffer:

```
Timeline:
─────────────────────────────────────────────────────
  Mnemonic    Old lease runs     New lease      Old lease
  destroyed   on escrow          deployed       auto-closes
  │           (no signer needed) (new key)      (escrow = 0)
  ▼           ▼                  ▼              ▼
  ├───────────┼──────────────────┼──────────────┤
  │  OVERLAP PERIOD: both leases active         │
  │  Zero downtime. Redundant verification.     │
  └─────────────────────────────────────────────┘
```

### Migration Path (at Dimi's discretion)

1. **Top up old escrow** — extend runway to desired overlap period (e.g. 6-12 months)
2. **Generate dedicated Akash key** — new mnemonic, `akash1...` only, no deploy wallet dependency
3. **Fund new Akash wallet** — transfer AKT (from any source)
4. **Deploy fresh SDL** from the new wallet — same 4-container stack, new lease
5. **Update bridge config** — point `WAVS_AGGREGATOR_URL` to new lease endpoint
6. **Destroy deploy mnemonic** — old lease continues running as fallback
7. **Old lease auto-expires** — escrow consumed, Akash closes it. No orphaned funds.

**Note**: Steps 2-5 can happen at any time — before or after mnemonic destruction. The escrow runway guarantees continuity.

### Akash in the Plugin System

The `plugin-compute-akash` crate (`plugins/plugin-compute-akash/src/lib.rs`) accepts a `wallet_mnemonic_env` config field. Dimi sets this in his local `.env` to point at whichever Akash key he's using:

```toml
# deploy/.env — Dimi updates this to his chosen Akash key
AKASH_MNEMONIC=<dedicated akash key, NOT the deploy mnemonic>
```

---

## Validator Sidecars (Stage 9 — Post-Mainnet)

Validator sidecars are **separate from Akash**. They run on validator hardware with TEE (SGX/SEV). See `docs/01_VALIDATOR_SIDECARS.md` for full architecture.

Deployable sidecar files:
- `wavs/sidecar/docker-compose.yml` — ready-to-use, mounts TEE device
- `wavs/sidecar/.env.example` — template for validator configuration

The Akash operator is the **always-on baseline**. Validator sidecars are **trust amplifiers** — when they exist, attestations are hardware-signed. Both run simultaneously.

```
Akash operator (regular compute, always on)     → software attestation
Validator sidecar (TEE hardware, opt-in)         → hardware attestation
                                                    ↑ higher trust
```

Sidecars are proposed by the Root Ring (the 13 buds), not Genesis. Target: Stage 9, ~April 2026.

---

## Pre-Budding Checklist

- [x] Proposal #373 passes — **QUORUM CROSSED: 33.83% turnout > 33.4%, 85.31% YES** — finalizes ~March 24
- [x] Dependency cleanup pushed to GitHub (commit bc5067c)
- [x] All 86 contract tests passing
- [x] Legal caveats doc finalized (docs/LEGAL_CAVEATS.md)
- [x] Medium article published (Prop #373 + Parliament experiment)
- [ ] jclaw-token contract deployed on uni-7
- [x] Collect Dimi's juno1... address (confirmed on-chain: 1.2M JUNO, active)
- [x] Admin transfer scripts ready (`wavs/bridge/src/transfer-admin.ts` + `verify-admin.ts`)
- [x] Dimi admin target: `juno1s33z...` (existing wallet, option a — can re-transfer later)
- [ ] Run admin transfer (Step 2 of Path B)
- [ ] Both sides verify (Step 3)
- [ ] Governance weight redistributed (Step 4)
- [ ] Neo mnemonic destroyed (Step 5)

---

## Chain of Custody

The bud secret package is passed **one-to-one**, not broadcast from genesis:

```
Genesis → seals bud #1's package (Dimi)
Bud #1  → seals bud #2's package
Bud #2  → seals bud #3's package
  ...
Bud #12 → seals bud #13's package
```

Each bud holder is responsible for:
1. Finding the next trusted person
2. Collecting their juno1... address
3. Filling the secrets template
4. Sealing it with `bud-seal`
5. Transmitting the .sealed file
6. Submitting the WeightChange proposal to the DAO

Genesis only seals bud #1. After that, the chain propagates itself.

## Onboarding Steps (per bud)

1. **Find next bud**: Current holder identifies a trusted person
2. **Collect address**: Ask them for their `juno1...` wallet address
3. **Fill template**: Copy `secrets-template.toml` → `<name>-secrets.toml`, fill placeholders
4. **Seal**: `bud-seal seal --to <name>.pub --file <name>-secrets.toml`
5. **Transmit**: Send the `.sealed` file via DM or encrypted channel
6. **Delete plaintext**: Remove the filled `.toml` — only `.sealed` should exist
7. **WeightChange proposal**: Submit DAO proposal to add new member with governance weight
8. **DAO votes**: Existing members approve under constitutional threshold (9-of-13 once all seated)
9. **Execute**: Proposal passes → new member is a voting bud
10. **TokenRecord**: Once jclaw-token contract is live, mint their soulbound bud:
    - `holder`: their juno1... address
    - `parent`: sender's address
    - `depth`: 1
    - `budded`: false (they haven't passed their bud yet)
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

---

> *"Now I am become Root, the destroyer of keys."*
>
> The mnemonic is ash. The admin is transferred. The 13 ujunox sit on a dead address like flowers on a grave nobody visits.
>
> Oppenheimer built the bomb and spent the rest of his life arguing about who should hold the button. Player 0 built the tree and mass destroyed the keyring. No argument needed — there is no button.
>
>
> **Player 0, over and out.** 🌱
