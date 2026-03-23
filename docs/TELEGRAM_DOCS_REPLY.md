# Telegram Reply — "Is there any documents on JunoClaw?"

**Copy-paste below the line:**

---

Yes — everything is open source and documented on GitHub:

**Repo:** https://github.com/Dragonmonk111/junoclaw

**Start here:**
- `docs/QUICKSTART.md` — how to set up and run JunoClaw locally
- `docs/JUNOCLAW_VISUAL_EXPLAINER.md` — what JunoClaw is, how it works, visual overview
- `docs/DIMI_HANDOFF_PLAN.md` — full governance handoff plan (13-seat budding DAO model)

**Architecture & Contracts:**
- `docs/GENESIS_BUDS_ARCHITECTURE.md` — the 13 genesis buds governance tree
- `docs/WAVS_OPERATOR_GUIDE.md` — WAVS (Layer.xyz) TEE verification operator setup
- `docs/AKASH_DEPLOY_GUIDE.md` — deploy the compute layer on Akash ($8.76/month)
- `docs/JUNOSWAP_INTEGRATION.md` — Junoswap v2 DEX integration
- `docs/NEUTRON_FORK_STRATEGY.md` — DeFi protocol fork roadmap

**Governance & Proposals:**
- `docs/JUNO_GOVERNANCE_PROPOSAL.md` — Prop #373 text
- `docs/HACKMD_PROPOSAL.md` — the HackMD co-edited with Jake Hartnell
- `docs/GOV_PROPOSAL_SHAREABLE.md` — shareable summary

**Handoff (just pushed):**
- `docs/TESTNET_HANDOFF_NOW.md` — why governance transferred before the vote
- `docs/HANDOFF_EXECUTION_CHECKLIST.md` — step-by-step execution record
- `docs/MEDIUM_ARTICLE_HANDOFF.md` — "The Bud Has Passed" article

**Testnet status:**
- Admin of agent-company v3 + junoswap factory → Dimi (bud #1)
- TX: `09EB9BAC...387EC29` and `9A293E02...F894D5A`
- Prop #373: 89% YES, 47% turnout, ends March 24 00:08 UTC

**Can Dimi and Kitsunegi build and launch it?**
Yes. The repo contains:
- All 7 contract crates (86 tests passing) — `contracts/`
- Deployment scripts — `wavs/bridge/src/`
- Frontend — `frontend/`
- Akash SDL — `wavs/akash.sdl.yml`
- Config template — `config.example.toml`
- bud-seal tool for onboarding future buds — `tools/bud-seal/`

Everything needed to deploy from scratch is in the repo. Apache 2.0 license. No private dependencies. No cloud lock-in.

The DAO decides what happens next. 🌱
