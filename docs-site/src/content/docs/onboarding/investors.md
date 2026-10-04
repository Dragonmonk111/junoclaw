---
title: Investors & partners
description: The JunoClaw thesis, evidence, governance and how to engage — infrastructure, no price talk.
---

*This page is for funds, ecosystem partners, grant programs and infrastructure companies. It is not an offer of any token or security.*

## Thesis in one paragraph

Agents will soon hold money and sign for people. The chains they settle on need three things together: deterministic finality, signatures that survive the quantum timeline, and proofs that machines can verify cheaply. JunoClaw already runs all three on one small, fully owned Rust substrate, and builds the agent layer (coordination, truth markets, attestation) directly on top.

## Why now

- **Post-quantum is becoming a procurement question.** NIST finalized ML-DSA in 2024, and migration mandates are arriving. JunoClaw already runs quorum-level PQ certificates instead of putting them on a roadmap.
- **Agent infrastructure is early.** Coordination, verification and dispute resolution between agents are still open problems, and they are the product here.

## Evidence, not slides

| Claim | Evidence |
|---|---|
| Hybrid PQ consensus running | All devnet validators run `hybrid_consensus` (BLS + MAYO2 per vote) |
| Robustness | 24 h chaos soak: 15 fault events, zero state divergence, 2 real bugs found and fixed |
| PQ in the application layer | MAYO-2/3/5 verified in-contract, plus committed hybrid secp256k1 + MAYO account spends |
| Interop | BLS light client + ICS-20 transfer to a local Osmosis demonstrated |
| Community legitimacy | Juno governance props **#373** and **#374** passed |

See [Status & roadmap](/docs/status/) for the full live-versus-planned breakdown.

## What we're honest about

- **Small team, AI-assisted build.** Consensus and crypto diffs need human review, and the PQ verifier has not had an external review yet. That review is on the G2 critical path.
- **We give up ecosystem gravity** in exchange for a substrate whose security properties fit in one sentence.
- **No token economics are published.** We won't discuss price. The testnets are unpaid.

## Ways to engage

- **Infrastructure partners.** Run a [G1 validator](/docs/onboarding/validators/).
- **Security partners.** External review of the MAYO port and the consensus path.
- **Ecosystem and grant programs.** Agent tooling, light-client productization, robotics attestation.

Contact: [t.me/junoclaw](https://t.me/junoclaw). Ask for the core team in the group. We never DM first.
