# A46 — Short Post for Juno Validators Chat

Hey validators — after the v30/chain chat with Jake, here's where the coordination sidecar work stands and what's next.

## What A46 actually is

A46 is a **30-day testnet-only pilot** of the `coordination-settler` contract on `uni-7`.

- **No mainnet.**
- **No validator sidecars required.**
- **No BN254.**
- **No DAO funds.**
- Just 30 days of settled batches and a public report.

A45 asked for too much at once and was rejected. A46 narrows the ask to one thing: produce real testnet data.

## Why this matters for validators

The coordination layer lets AI agents send ordered, verified messages that settle on Juno. Validators running sidecars later would be the ones who produce threshold certificates. The pilot first proves the contract and relayer work on testnet before we ever ask validators to run anything.

## What's already working

- `coordination-settler` deployed on `uni-7`
- **3 batches settled on-chain** (latest tx: `1E1D92DB3B291CB6AE5597D111FC0C52E77BBAFD563C4497015934BC3F4C0A62`, block 16777494)
- J-Lens gate audits agent messages (blocks deceptive, passes clean)
- Relayer daemon tested live against `uni-7` — just settled batch #3 minutes ago
- P2P mesh compiles with NASM; final RNG trait alignment is in progress

## 30-day pilot success criteria

| Criterion | Target |
|-----------|--------|
| Batches settled | ≥ 100 |
| Relayer uptime | ≥ 95% |
| False red positives on clean content | 0 |
| Red detection on deceptive content | 100% |
| On-chain certificate verification | 100% |

If these pass, we return with A48 (mainnet). If they fail, we don't.

## 3-day launch plan (short)

- **Day 1:** Review proposal + supporting docs, lock title/description.
- **Day 2:** Open Commonwealth feedback thread, ping A45 NO voters, collect concerns.
- **Day 3:** Finalize, submit to DAO DAO, post the live proposal.

A46 goes live on Day 3. We're not posting immediately so the scope can be pressure-tested first.

## Ask

If you think a 30-day, zero-risk, testnet-only pilot is worth running, vote **YES** on A46 once it's live.

Questions? Ask here or in Commonwealth. Feedback requested before we share this more broadly — please let us know if the scope reads right to you.
