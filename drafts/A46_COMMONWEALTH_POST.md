# A46 — Commonwealth / Forum Post (Executive Version)

Title: **A46 — Coordination-Settler 30-Day Testnet Pilot: uni-7**

## TL;DR

A45 asked for architecture ratification and was rejected. This proposal is much smaller: **endorse a 30-day testnet-only pilot of the already-deployed `coordination-settler` on uni-7.** No mainnet. No sidecars. No BN254. No funds. Just 30 days of settled testnet batches with hard, public success criteria.

If the pilot succeeds, the DAO can consider a mainnet proposal (A48). If it fails, nothing happens — no chain state changes, no liability.

## Why A45 Failed

The juno-ai steward voted NO again. A45 still bundled too much:
- three-layer architecture ratification
- validator sidecar as production target
- BN254 as an open question
- mainnet gating language

This proposal removes everything except the testnet pilot.

## What Changed Since A45

- **Relayer works:** 2 batches already settled on-chain on uni-7 (txs `3D2EF675...` and `2DD277E9...`)
- **Commonware P2P compiles:** `aws-lc-sys` builds with NASM, `commonware-p2p` v2026.7.0 compiles successfully
- **J-Lens wired:** attestations now feed into `tools/context-agent/src/trust.js`
- **Validator guides drafted:** `drafts/VALIDATOR_SIDECAR_TESTNET_GUIDE.md` and `drafts/ARTICLE_VALIDATOR_SIDECAR_MASTER_EXPLAINER.md` are ready for after the pilot

## The Ask

Vote **YES** to run a 30-day testnet pilot with these constraints:
- 100+ batches settled on uni-7
- 0 red false positives on clean messages
- 100% red blocking on deceptive messages
- ≥95% relayer uptime
- All certificates verified on-chain

## What's Not In This

- No mainnet deployment
- No validator sidecars
- No BN254
- No funds, tokens, membership changes
- No architecture ratification beyond "run this testnet pilot"

## After the Pilot

If success criteria are met, the builder will return with:
- **A48:** Mainnet coordination-settler deployment (small, focused)
- **A48:** Validator sidecar pilot (optional, opt-in)
- **A49:** BN254 precompile open question (only if Juno team is interested)

If criteria are not met, no follow-up proposal is made.

---

**Vote YES for a 30-day, zero-risk, testnet-only pilot.**
