# Handoff Execution Checklist — D-Day

**Execute after Prop #373 voting period ends: March 24, 2026 at 00:08 UTC**

---

## Pre-Flight Checks (Do BEFORE execution)

### 1. Verify Vote Status
- [ ] Voting period has officially ended (March 24, 00:08 UTC)
- [ ] Final YES percentage: ______%
- [ ] Final turnout: ______% (must be > 33.4%)
- [ ] Proposal status: PASSED

### 2. Verify Wallet Safety
- [ ] Run wallet verification: `npx tsx src/verify-wallets.ts`
- [ ] Confirm: Mother wallet is on SEPARATE mnemonic ✅
- [ ] Confirm: Mother mnemonic is backed up separately
- [ ] Confirm: Neo mnemonic is accessible in `wavs/.env`

### 3. Verify Script Readiness
- [ ] TypeScript compiles clean: `npx tsc --noEmit --project tsconfig.json`
- [ ] Transfer script exists: `wavs/bridge/src/transfer-admin.ts`
- [ ] Verify script exists: `wavs/bridge/src/verify-admin.ts`
- [ ] Target address confirmed: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` (Dimi)

### 4. Check Neo Wallet Balance
- [ ] Query Neo balance on-chain
- [ ] Confirm sufficient JUNOX for 7 TXs (~0.2 JUNOX) + 5 JUNOX to Dimi + drain amount
- [ ] Expected balance: ~15,000 JUNOX (14,994 to drain + 5 to Dimi + 0.2 gas + 13 tombstone)

---

## Execution (One Command)

### Run Transfer Script

```bash
cd c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\wavs\bridge
npx tsx src/transfer-admin.ts juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt
```

**Expected output:**
- Phase 1: Verify Neo is admin (5 contracts) ✅
- Phase 2: Transfer admin to Dimi (5 TXs)
- Phase 3: Send 5 JUNOX to Dimi (1 TX)
- Phase 4: Drain to Mother, leave 13 ujunox (1 TX)
- Phase 5: Verify all transfers ✅

**Record TX hashes:**
1. agent-company v3: `_______________________________________________`
2. junoswap factory: `_______________________________________________`
3. escrow: `_______________________________________________`
4. agent-registry: `_______________________________________________`
5. task-ledger: `_______________________________________________`
6. Fund Dimi: `_______________________________________________`
7. Drain to Mother: `_______________________________________________`

**Record execution time:** `______:______ UTC`

---

## Post-Execution Verification

### 1. Verify Admin Transfers (Independent Check)

```bash
npx tsx src/verify-admin.ts
```

**Expected output:**
- All 5 contracts show admin: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` ✅

### 2. Verify Balances

Query on-chain:
- [ ] Neo balance: `13 ujunox` (tombstone) ✅
- [ ] Dimi balance: increased by ~5,000,000 ujunox ✅
- [ ] Mother balance: increased by ~14,994,000,000 ujunox ✅

### 3. Verify on Block Explorers

Check TXs on:
- Mintscan: https://testnet.mintscan.io/juno-testnet/address/juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m
- ping.pub: https://ping.pub/juno-testnet/account/juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m

---

## Mnemonic Destruction (CRITICAL — No Going Back)

### 5a — Files & History
- [ ] Delete `wavs/.env` (contains Neo mnemonic)
- [ ] Delete any paper/digital backups of Neo mnemonic
- [ ] Clear terminal history: `doskey /reinstall` (Windows)
- [ ] Clear clipboard: copy something else over it

### 5b — Akash Console (Chrome)
- [ ] Close Akash Console tab
- [ ] (Optional) Clear site data: Chrome → Settings → Privacy → Site Settings → `console.akash.network` → Clear data
- [ ] Note: Lease continues running on escrow autopilot (~3-4 months)

### 5c — Keplr Wallet (Chrome Extension)
- [ ] Open Keplr → click account/profile icon
- [ ] Identify which wallets are loaded (Neo + Mother + personal?)
- [ ] Remove Neo account: Settings → ⋮ menu → "Remove Account"
- [ ] Confirm Mother wallet is still accessible (separate mnemonic)
- [ ] Choose: (a) Keep Keplr, (b) Lock Wallet, or (c) Delete extension

### 5d — Confirm Destruction
- [ ] `wavs/.env` deleted
- [ ] Paper/digital mnemonic backups destroyed
- [ ] Terminal history cleared
- [ ] Akash Console tab closed
- [ ] Neo removed from Keplr
- [ ] Mother wallet mnemonic verified as separately backed up

**Neo wallet is permanently inert.** The 13 ujunox sit there forever.

---

## Medium Article Publication

### Fill in Placeholders

Edit `docs/MEDIUM_ARTICLE_VOTE_PASSED.md`:

1. Replace `[FINAL_YES_PCT]` with actual final YES percentage
2. Replace `[FINAL_TURNOUT]` with actual final turnout percentage
3. Replace `[FINAL_VETO]` with actual NoWithVeto percentage
4. Replace `[TIME]` with execution time (UTC)
5. Replace `[TX_HASH_1]` through `[TX_HASH_7]` with actual TX hashes
6. Replace `[MOTHER_FINAL_BALANCE]` with Mother's final JUNOX balance

### Publish

- [ ] Copy final article to Medium
- [ ] Add cover image (if available)
- [ ] Add tags: blockchain, governance, AI, decentralization, cosmos
- [ ] Publish
- [ ] Share link on Juno Discord, Twitter, etc.

---

## Notify Dimi

Send message to Dimi with:
- [ ] Handoff complete confirmation
- [ ] Link to verify-admin script output
- [ ] Link to TX hashes on block explorer
- [ ] Next step: Submit `WeightChange` proposal
- [ ] Reminder: Find bud #2

---

## Final Confirmation

- [ ] All 7 TXs confirmed on-chain
- [ ] All 5 contracts show Dimi as admin
- [ ] Neo wallet balance: 13 ujunox
- [ ] Neo mnemonic destroyed (no recovery possible)
- [ ] Mother wallet safe (separate mnemonic)
- [ ] Medium article published with TX proof
- [ ] Dimi notified

**Player 0 has left the building.** 🌱

---

## Emergency Rollback (ONLY if something goes catastrophically wrong)

**If the script fails mid-execution and contracts are in inconsistent state:**

1. **DO NOT destroy the mnemonic yet**
2. Check which contracts successfully transferred admin
3. For contracts still showing Neo as admin, re-run transfer manually:
   ```bash
   # Example for single contract
   npx tsx src/transfer-admin.ts juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt
   ```
4. Verify all 5 contracts before proceeding to mnemonic destruction

**If Dimi's address is wrong or compromised:**

1. **DO NOT destroy the mnemonic yet**
2. Generate new address for Dimi
3. Re-run transfer script with correct address
4. Verify before proceeding

**If Mother wallet is accidentally on same mnemonic:**

1. **STOP — DO NOT DESTROY MNEMONIC**
2. Create new Mother wallet (separate mnemonic)
3. Transfer all funds: Neo Mother → New Mother
4. Update all documentation with new Mother address
5. THEN destroy Neo mnemonic

---

*This checklist is a one-time execution document. After completion, it becomes a historical record of the handoff.*
