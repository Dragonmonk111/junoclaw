# DIY Ideas — What You Can Build on JunoClaw

Discussion file. Items marked ✅ are built and tested. Items marked 💡 are ideas ready to implement.

---

## ✅ 1. Agentic Parliament (Built — uni-7 testnet)

**What it is:** 7 AI "Members of Parliament" with distinct policy stances, voting on proposals on-chain. Real wallets, real transactions, deterministic vote logic based on keyword evaluation against each MP's stance.

**Contract:** `juno1a5ta00sq7qtd7y65mheaerux6ngzencvj3cvz4smhgke3ypv9mdqulwepl`
**Script:** `wavs/bridge/src/parliament-demo.ts`
**Code ID:** 63 (agent-company v3)

### The 7 MPs

| Seat | Name | Role | Default Bias |
|------|------|------|-------------|
| 1 | The Builder | Infrastructure Chair | YES |
| 2 | The Fiscal Hawk | Treasury Oversight | NO |
| 3 | The Populist | Community Representative | YES |
| 4 | The Technocrat | Verification Standards | ABSTAIN |
| 5 | The Diplomat | Cross-Chain Relations | YES |
| 6 | The Environmentalist | Sustainability Advocate | ABSTAIN |
| 7 | The Contrarian | Devil's Advocate | NO |

### What we learned

1. **Auto-pass problem:** Contract auto-resolves proposals once quorum (51%) is met and YES > NO. With 7 MPs, 4 votes = 57% of weight → auto-pass before MPs 5-7 can vote.
   - **Fix:** Set `quorum_percent: 100` to force full participation, or use a two-phase commit (debate window → vote window).
   - **Deeper fix:** Add a `min_voters_percent` parameter to the contract — separate from quorum — that requires a minimum number of distinct voters before resolution.

2. **Stable coalitions:** Builder + Populist + Technocrat always vote the same way. Real parliaments have shifting coalitions. A future version could add:
   - Randomized proposal framing (same proposal, different keywords)
   - LLM-based evaluation instead of keyword matching (Ollama on Akash)
   - Coalition negotiation (agents message each other before voting)

3. **Vote ordering matters:** The first 4 seats have all the power. A sortition-based random vote order (using NOIS/drand) would fix this.

### Refinement: Full-participation version

The refined `parliament-demo.ts` uses `quorum_percent: 100` so all 7 MPs must vote. This reveals the true power dynamics:
- **Proposals with broad appeal** (IBC, verification mandates) pass 5-2
- **Spending proposals** are contested: Fiscal Hawk + Contrarian form a consistent NO bloc
- The Technocrat and Environmentalist become swing votes

### How to run it

```bash
cd wavs/bridge
npx tsx src/parliament-demo.ts setup     # generate 7 wallets, fund, deploy
npx tsx src/parliament-demo.ts propose   # submit next proposal
npx tsx src/parliament-demo.ts debate    # show reasoning + forecast
npx tsx src/parliament-demo.ts vote      # all 7 vote on-chain
npx tsx src/parliament-demo.ts status    # full state overview
```

---

## 💡 2. Community Grant Committee (Sortition)

**Concept:** Instead of electing a fixed committee, randomly select 5 of the 13 bud holders to review each grant application. Uses the NOIS/drand sortition already built into agent-company.

**What exists:** `SortitionRequest` proposal kind, `resolve_sortition` with Fisher-Yates via SHA-256, 5 passing tests.

**What to build:**
- Grant proposal template: amount, description, milestone deliverables
- Sortition selects 5 reviewers from the 13
- Reviewers vote within 48 hours (block-based deadline)
- WAVS attests the randomness was fair and the vote count is correct
- If passed, escrow releases funds in milestones

**Effort:** ~1 day (sortition already works, need grant-specific proposal flow)

---

## 💡 3. Prediction Market DAO

**Concept:** Members create binary outcome markets ("Will Juno hit $1 by June?"). WAVS resolves outcomes using external data attested by TEE.

**What exists:** `OutcomeCreate` and `OutcomeResolve` proposal kinds in agent-company, plus the verification pipeline.

**What to build:**
- Market creation UI (question, deadline, resolution source)
- Stake mechanism (members lock JUNO on yes/no positions)
- WAVS resolution: TEE fetches external data (price feed, on-chain state), produces attested outcome
- Payout distribution to winners
- Verifiable outcome markets — every resolution has a hardware-sealed receipt

**Effort:** ~2-3 days (outcome types exist, need staking + resolution + payout logic)

---

## 💡 4. Validator Health Dashboard (WAVS-Attested)

**Concept:** WAVS operator continuously monitors Juno validators — uptime, missed blocks, voting participation, commission changes — and posts attested health reports on-chain.

**What to build:**
- New WASI component: `validator-health-check.wasm`
- Queries validator set from Juno RPC inside TEE
- Produces health score per validator (composite of uptime, governance participation, commission fairness)
- Posts attested report to task-ledger
- Frontend dashboard reads attested reports and displays validator rankings
- Community can use this to make informed delegation decisions

**Why it matters:** Most validator dashboards are trust-based. This one proves the data wasn't tampered with.

**Effort:** ~2 days (new WASI component + frontend panel)

---

## 💡 5. Automated Treasury Rebalancer

**Concept:** The DAO sets a target portfolio ratio (e.g., 60% JUNO, 30% OSMO, 10% ATOM). An agent continuously rebalances using Junoswap v2, with every trade WAVS-attested.

**What to build:**
- Treasury policy proposal: define target ratios and rebalance thresholds
- Agent monitors portfolio composition
- When drift exceeds threshold (e.g., 5%), agent executes swaps
- WAVS verifies each swap: correct price, acceptable slippage, matches policy
- All attestations stored on-chain — full audit trail
- DAO can change policy via governance vote

**This is the "agentic swap" in action.** The example from QUICKSTART.md but actually running.

**Effort:** ~3 days (policy engine + monitoring loop + swap execution + attestation)

---

## 💡 6. Cross-Chain Arbitrage Agent

**Concept:** Agent watches JUNO price across Junoswap v2 (Juno) and Osmosis pools (via IBC). When spread exceeds threshold, executes arbitrage. Profits go to DAO treasury. Every trade attested.

**What to build:**
- IBC channel monitoring (already in 5 WAVS workflows)
- Price comparison engine (Junoswap v2 pool vs Osmosis pool)
- Arbitrage execution: buy low chain → IBC transfer → sell high chain
- WAVS attestation of each leg
- Profit tracking and distribution to DAO members

**Risk:** IBC latency means the spread might close before execution completes. Agent needs to evaluate expected profit vs gas + IBC timeout risk.

**Effort:** ~4-5 days (multi-chain coordination, IBC handling)

---

## 💡 7. Reputation Credential System

**Concept:** Extend the soulbound trust-tree with on-chain reputation. Members earn reputation for:
- Voting on proposals (participation)
- Correct prediction market outcomes
- Successful grant milestones
- WAVS-verified contributions

Reputation is non-transferable (soulbound) and decays over time if inactive.

**What to build:**
- Reputation state in agent-company (or new jclaw-token contract)
- Reputation update logic tied to proposal execution
- Decay function (reputation halves every 90 days without activity)
- Reputation-weighted voting (optional governance upgrade)
- Visual trust-tree in frontend showing reputation scores

**Effort:** ~3 days (state extension + decay logic + frontend visualization)

---

## 💡 8. Skill-Staking Circle

**Concept:** Members stake their skills (not tokens) against specific tasks. If they deliver (verified by WAVS + peer review), they earn weight. If they don't, they lose weight.

**Example:** "I'll build the IBC relay within 2 weeks. I stake 200 weight on it."
- If delivered (WAVS verifies code commit + tests pass): weight doubled
- If not delivered by deadline: weight lost

**This creates skin-in-the-game without financial risk.** Contributors risk governance power, not money.

**Effort:** ~4 days (new proposal type + stake/release logic + deadline enforcement)

---

## Build Priority (suggested order)

| Priority | Idea | Why |
|----------|------|-----|
| 1 | ✅ Agentic Parliament | Done. Proves the system works with multiple agents |
| 2 | Community Grant Committee | Demonstrates sortition + real-world utility |
| 3 | Prediction Market | Showcases OutcomeCreate/Resolve + WAVS resolution |
| 4 | Treasury Rebalancer | The "killer app" for agentic swaps |
| 5 | Validator Health Dashboard | Community tool, drives adoption |
| 6 | Reputation System | Deepens the trust-tree model |
| 7 | Cross-Chain Arbitrage | High complexity but high visibility |
| 8 | Skill-Staking Circle | Novel mechanism, needs more design |

---

## Integration with QUICKSTART.md

After discussion, merge the refined Parliament demo and selected ideas into QUICKSTART.md as:
- A "What You Can Build" section with 3-4 concrete examples
- Link to this file for the full idea list
- Code pointers for each idea showing which contract features to use

---

*All ideas use existing JunoClaw contracts (agent-company v3, escrow, task-ledger, junoswap). No new contracts needed for ideas 1-5.*
