# Testnet Handoff — Execute NOW (Before Vote Ends)

**Strategic Decision: Pass the testnet bud before mainnet deployment**

## Rationale

**Governance-First Approach:**
- Dimi gets testnet admin NOW
- DAO can form and vote on mainnet deployment plan
- The 13 decide together: audit strategy, deployment timing, liquidity plan
- Demonstrates budding process is real, not theoretical
- Uses weekend for DAO formation while vote continues

**What This Enables:**
1. **Dimi coordinates audits** — DAO votes on audit allocation before mainnet
2. **DAO votes on mainnet plan** — not just inheriting Genesis decisions
3. **Distributed decision-making** — "Deploy Monday or wait?" becomes a DAO vote
4. **Removes bottleneck** — Genesis not the only one who can move forward
5. **Public demonstration** — Community sees governance transfer happening

**Key Difference from Original Plan:**
- **Keep Neo mnemonic** for mainnet deployment (don't destroy yet)
- Testnet handoff = governance transfer only
- Mainnet deployment = separate decision after vote passes

---

## Execution Steps (This Weekend)

### 1. Run Transfer Script (Modified)

```bash
cd c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\wavs\bridge
npx tsx src/transfer-admin.ts juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt
```

**What happens:**
- Phase 1: Verify Neo is admin ✅
- Phase 2: Transfer admin to Dimi (5 TXs)
- Phase 3: Send 5 JUNOX to Dimi (1 TX)
- Phase 4: Drain to Mother, leave 13 ujunox (1 TX)
- Phase 5: Verify transfers ✅

**Record TX hashes:**
1. agent-company v3: `_______________`
2. junoswap factory: `_______________`
3. escrow: `_______________`
4. agent-registry: `_______________`
5. task-ledger: `_______________`
6. Fund Dimi: `_______________`
7. Drain to Mother: `_______________`

### 2. Verify Handoff

```bash
npx tsx src/verify-admin.ts
```

**Expected:**
- All 5 contracts show admin: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` (Dimi) ✅
- Neo balance: `13 ujunox` ✅

### 3. DO NOT Destroy Mnemonic Yet

**CRITICAL:** Skip mnemonic destruction. You need it for:
- Mainnet deployment (same address as proposal specifies)
- Potential emergency recovery
- Final cleanup after mainnet handoff

**What to skip:**
- ❌ Delete `wavs/.env`
- ❌ Delete paper/digital backups
- ❌ Clear terminal history
- ❌ Remove Neo from Keplr

**Keep everything until mainnet deployment is complete.**

---

## Public Announcement (Post on Telegram/Discord)

```
🌱 Testnet Governance Handoff — Live Now

Prop #373 is at 89% YES with 47% turnout. While we wait for the vote to finalize, 
testnet governance is transferring to the DAO.

What just happened:
✅ Testnet admin transferred to Dimi (bud #1)
✅ 5 JUNOX sent to Dimi for gas
✅ Testnet tokens drained to treasury
✅ 13 ujunox left on Genesis wallet (tombstone)

What happens next:
1. Dimi submits WeightChange proposal (testnet)
2. Buds #2-#13 get onboarded over the weekend
3. DAO votes on mainnet deployment plan
4. After Prop #373 passes Monday, DAO decides: deploy now or audit first?

This is the budding process in action. Not theoretical. Happening now.

TX hashes: [link to block explorer]
Verify yourself: npx tsx src/verify-admin.ts

— Genesis (Player 0)
```

---

## What Dimi Can Do Now

**Immediate (This Weekend):**
1. **Verify admin access** — confirm he controls all 5 testnet contracts
2. **Submit WeightChange proposal** — distribute governance weight to 13 seats
3. **Begin onboarding buds #2-#13** — personal vetting, trust chain
4. **Draft mainnet deployment proposal** — DAO votes on timing, audit strategy

**After Vote Passes (Monday):**
1. **DAO votes on mainnet plan** — options:
   - Option A: Deploy immediately (Genesis executes with Neo address)
   - Option B: Audit first, deploy after (DAO funds audit)
   - Option C: Phased deployment (contracts first, liquidity later)
2. **DAO coordinates liquidity** — who provides, how much, which pairs
3. **DAO decides on mainnet admin** — keep Genesis, transfer to multisig, or distribute

---

## Mainnet Deployment (After Vote + DAO Decision)

**Timeline:**
- **Monday 00:08 UTC**: Vote finalizes
- **Monday 01:00 UTC**: DAO votes on mainnet deployment plan
- **Monday 02:00+ UTC**: Execute per DAO decision

**If DAO votes "Deploy Now":**
- Genesis deploys with Neo address (matches proposal)
- Transfer mainnet admin to DAO multisig
- Destroy Neo mnemonic (both chains handed off)

**If DAO votes "Audit First":**
- DAO allocates funds for audit
- Auditors review contracts
- Deploy after audit clears
- Genesis retains Neo mnemonic until deployment

---

## Risk Assessment

**Risks of Passing Bud Now:**

1. **Vote could fail** (0.1% chance at 89% YES)
   - Mitigation: Testnet ≠ mainnet. Testnet handoff doesn't require vote authorization.
   
2. **Dimi could go dark**
   - Mitigation: Genesis still has Neo mnemonic, can deploy mainnet independently if needed
   
3. **DAO could deadlock on mainnet decision**
   - Mitigation: Genesis retains emergency authority until mainnet is live
   
4. **Optics: "Why couldn't you wait 73 hours?"**
   - Response: "Governance-first. The DAO should decide mainnet deployment, not Genesis alone."

**Benefits:**

1. **Demonstrates commitment** — Actions speak louder than promises
2. **Enables DAO formation** — Weekend timing perfect for onboarding
3. **Distributes decision-making** — Mainnet plan becomes collaborative
4. **Removes single point of failure** — Dimi can operate testnet independently
5. **Models the governance you want** — Decentralize early, not late

---

## Modified Mnemonic Lifecycle

**Phase 1 — Bootstrap (Done):**
- Neo wallet deployed all testnet contracts
- Mnemonic held by Genesis

**Phase 2 — Testnet Handoff (This Weekend):**
- Admin transferred to Dimi
- Tokens drained, 13 ujunox tombstone
- **Mnemonic kept** for mainnet deployment

**Phase 3 — Mainnet Deployment (After Vote + DAO Decision):**
- Deploy to mainnet with Neo address
- Transfer mainnet admin per DAO vote
- **Mnemonic destroyed** after both chains handed off

**Phase 4 — Multisig Migration (Future):**
- DAO migrates to CW3 multisig
- No single person holds deployment keys

---

## Checklist

**Pre-Execution:**
- [ ] Verify Mother wallet mnemonic is backed up separately ✅ (already confirmed)
- [ ] Verify Neo has sufficient balance for 7 TXs + 5 JUNOX + drain
- [ ] TypeScript compiles clean
- [ ] Dimi's address confirmed: `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt`

**Execution:**
- [ ] Run transfer script
- [ ] Record all 7 TX hashes
- [ ] Verify admin transfers
- [ ] Verify balances (Neo: 13 ujunox, Dimi: +5 JUNOX, Mother: +drained)

**Post-Execution:**
- [ ] Announce on Telegram/Discord
- [ ] Share TX hashes publicly
- [ ] Contact Dimi with next steps
- [ ] **Keep Neo mnemonic safe** (needed for mainnet)

**DO NOT (Yet):**
- [ ] ❌ Delete `wavs/.env`
- [ ] ❌ Destroy mnemonic backups
- [ ] ❌ Clear terminal history
- [ ] ❌ Remove Neo from Keplr

**After Mainnet Deployment:**
- [ ] Delete `wavs/.env`
- [ ] Destroy all mnemonic backups
- [ ] Clear terminal history
- [ ] Remove Neo from Keplr
- [ ] Close Akash Console tab

---

## Final State (After This Weekend)

**Testnet (uni-7):**
- Admin: Dimi (DAO forming)
- Genesis: 13 ujunox tombstone, no admin power
- Governance: Transitioning to 13-seat DAO

**Mainnet (juno-1):**
- Not deployed yet
- Awaiting DAO vote on deployment plan
- Genesis retains deployment capability

**Neo Mnemonic:**
- Still exists (needed for mainnet)
- Backed up securely
- Will be destroyed after mainnet handoff

---

*This is governance-first design. The DAO decides the mainnet plan. Genesis executes what they vote for.*
