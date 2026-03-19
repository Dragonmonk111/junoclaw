# JunoClaw — Open Ends Inventory

*Last updated: March 18, 2026 (post Qu-Zeno Portal, pre-proposal submission)*

---

## Status Key
- **LIVE** = deployed and working
- **BUILT** = code exists, not yet deployed/running
- **MOCK** = UI renders with fake data, real integration pending
- **TODO** = not started

---

## 1. Smart Contracts

| Component | Status | Notes |
|-----------|--------|-------|
| agent-company v3 | LIVE (uni-7) | Code ID 63, 5 proposals executed |
| Junoswap factory | LIVE (uni-7) | Code ID 61 |
| Junoswap pair (JUNOX/USDC) | LIVE (uni-7) | Code ID 60, no real liquidity |
| Junoswap pair (JUNOX/STAKE) | LIVE (uni-7) | Code ID 60, no real liquidity |
| $JClaw soulbound token | TODO | Contract not written. Trust-tree budding model designed but not implemented |
| Escrow contract | TODO | Referenced in proposal but not yet written |
| Task-ledger contract | TODO | Referenced in proposal but not yet written |
| Agent-registry contract | TODO | Referenced in proposal but not yet written |

### Open ends:
- **Mainnet deployment**: All contracts need redeployment on juno-1 after proposal passes
- **Liquidity**: Junoswap pairs have zero real liquidity — needs community LP provision post-mainnet
- **$JClaw token**: Soulbound token contract needs design + implementation
- **Missing contracts**: escrow, task-ledger, agent-registry are mentioned in proposal roadmap but don't exist yet

---

## 2. Frontend (React/Vite)

| Component | Status | Live Data? | Notes |
|-----------|--------|------------|-------|
| ChatPanel | BUILT | Mock | Agent chat uses local store, no backend LLM |
| DaoPanel | LIVE | Yes | Keplr + CosmJS, live proposals from uni-7, vote/execute/propose TX |
| DexPanel — Swap | BUILT | Partial | dex-queries + dex-execute wired, falls back to mock if chain unreachable |
| DexPanel — Pool | BUILT | Partial | Shows real reserves if chain connected, mock otherwise |
| DexPanel — Liquidity | BUILT | Partial | Provide/withdraw buttons wired to real TX, mock LP position |
| DexPanel — Attestations | MOCK | No | Renders mock attestation data, no WebSocket feed connected |
| IntelPanel (Qu-Zeno) — Overview | BUILT | Partial | WebSocket to ws://localhost:7778, mock data when offline |
| IntelPanel — Gov Watch | MOCK | No | Renders mock governance events |
| IntelPanel — Migrations | MOCK | No | Renders mock migration events |
| IntelPanel — Whale Alert | MOCK | No | Renders mock whale events |
| IntelPanel — IBC Health | MOCK | No | Renders mock IBC events |
| UpdatesPanel | MOCK | No | Background event log, local store only |
| Sidebar | BUILT | Partial | Agent list from store, no real backend |
| StatusBar | BUILT | Partial | Shows chain connection status |

### Open ends:
- **ChatPanel backend**: No actual LLM inference — needs a backend service or direct Akash endpoint
- **DexPanel attestations**: Needs WebSocket feed from chain-watcher to show real swap attestations
- **Qu-Zeno live feed**: Chain-watcher (port 7778) needs to be running for real events
- **Qu-Zeno data**: All 4 intel sub-views show mock data; need chain-watcher to emit typed events
- **UpdatesPanel**: Entirely mock — could be wired to chain-watcher feed
- **TypeScript**: Compiles clean (`tsc --noEmit` = 0 errors)

---

## 3. WAVS / Chain-Watcher

| Component | Status | Notes |
|-----------|--------|-------|
| WASI component (494KB) | BUILT | 10 verification workflows compiled |
| Chain-watcher (EventWatcher) | BUILT | `wavs/chain-watcher/src/` — watches Juno events |
| Chain-watcher (Verifier) | BUILT | Runs WASI component locally |
| Chain-watcher (Attestor) | BUILT | Submits attestation TX to chain |
| Chain-watcher (FeedServer) | BUILT | WebSocket on port 7778 for frontend |
| Bridge (local-compute) | BUILT | For local testing |

### Open ends:
- **Chain-watcher not running**: Needs `npm start` in wavs/chain-watcher/ for Qu-Zeno live data
- **Akash deployment**: Operator stack deployed but may need SDL update for chain-watcher
- **warg-registry**: Self-publishing confirmed, but in-memory storage resets on restart
- **Multi-operator**: Architecture supports it, but only 1 operator instance exists

---

## 4. Governance / DAO

| Item | Status | Notes |
|------|--------|-------|
| 13 bud addresses | TODO | Need to collect real addresses from community members |
| WeightChange proposal | TODO | Requires 13 addresses, then submit via CLI or UI |
| $JClaw distribution | TODO | Depends on $JClaw token contract |
| Mainnet agent-company | TODO | Deploy after proposal passes |
| CodeUpgrade quorum (67%) | BUILT | Contract supports it, tested |

### Open ends:
- **Bud recruitment**: Need 13 real community members willing to participate
- **Weight distribution**: Genesis holds 100% until WeightChange proposal executes
- **Mainnet deployment sequence**: agent-company → factory → pairs → escrow → task-ledger → registry

---

## 5. Infrastructure

| Item | Status | Notes |
|------|--------|-------|
| Akash SDL (4 containers) | LIVE | ~$8.76/month, 63.77 AKT funded |
| warg-registry on Akash | LIVE | Self-publishing junoclaw:verifier v0.1.0 |
| Validator sidecar docker-compose | BUILT | Ready but no validators running it yet |
| Juno validator node (VirtualBox) | LIVE | Unbonded, position 6 |
| GitHub repo | LIVE | https://github.com/Dragonmonk111/junoclaw |
| HackMD proposal | LIVE | https://hackmd.io/s/HyZu6qv5Zl (Jake co-editing) |

### Open ends:
- **Validator sidecar adoption**: Need validators to opt in post-proposal
- **Akash TEE**: Akash doesn't offer TEE instances — hardware attestation depends on validators
- **AKT budget**: 63.77 AKT covers 3-5 months; need plan for sustained funding

---

## 6. Documentation / Community

| Item | Status | Notes |
|------|--------|-------|
| GOV_PROP_COPYPASTE.md | DONE | Ready for ping.pub submission |
| HACKMD_PROPOSAL.md | DONE | Live, Jake amended |
| ART_PROMPTS.md | DONE | 8 Ghibli prompts |
| FINAL_UPDATE_ARTICLE.md | DONE | Medium article |
| TEE_MILESTONE_ARTICLE.md | DONE | TEE proof article |
| SUBMISSION_CHECKLIST.md | DONE | Tonight's plan |

### Open ends:
- **Check Jake's HackMD edits**: Sync any changes back to GOV_PROP_COPYPASTE.md before submitting
- **Post-submission announcements**: Telegram, Discord, Twitter
- **Medium update**: Add proposal number + voting link after submission

---

## Priority Queue (What To Do After Proposal)

### Immediate (this week)
1. Submit proposal tonight ← **YOU ARE HERE**
2. Announce on Telegram + Discord + Twitter
3. Recruit 13 bud addresses from community respondents
4. Start chain-watcher for Qu-Zeno live demo

### Short-term (next 2 weeks)
5. Write $JClaw soulbound token contract
6. Write escrow + task-ledger + agent-registry contracts
7. Wire DexPanel attestations to chain-watcher WebSocket
8. Wire Qu-Zeno to chain-watcher typed events
9. Deploy mainnet contracts (if proposal passes)

### Medium-term (month 2)
10. Provide initial liquidity to Junoswap pairs
11. Execute WeightChange → distribute governance to 13 buds
12. Onboard validators for TEE sidecars
13. Security audit (community-funded)
14. ChatPanel backend (LLM integration)

---

*"What is watched cannot decay unnoticed." — Qu-Zeno Portal*
