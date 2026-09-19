# JunoClaw Production Subscription Ladder

*Drafted Sep 2, 2026. Not yet a committed plan — for discussion.*

## Context

JunoClaw is currently fully open-source with on-chain skill registry. No SaaS,
no paywall, no subscription. This document proposes a production subscription
ladder that funds development without breaking the open-source core.

## Principle: Open Core, Paid Infrastructure

The software is and remains open source (MIT/Apache-2.0). What you charge for
is **hosted infrastructure** and **convenience** — the same model as GitLab,
Plausible, or Supabase. Anyone can self-host the full stack for free. Most
people won't want to.

## Subscription Tiers

### Tier 1: Hobbyist (Free, forever)
- Self-host everything: ROS2 bridge, browser viewer, training scripts, ONNX export
- Full access to open-source repos (junoclaw-physics, skill-registry, bridge)
- Train policies locally, export to ONNX, register on-chain yourself
- Buzz relay: connect to any public relay node
- **Target**: Makers, students, individual robot owners
- **Cost to us**: Zero (they run their own infra)
- **Value to us**: Community, bug reports, skill ecosystem growth

### Tier 2: Builder ($20/month)
- Hosted Buzz relay node (we run the relay, they connect)
- Skill-registry: one-click publish with ONNX hash auto-computed
- HuggingFace Hub auto-upload from export_onnx.py (--push flag)
- Training dashboard: web UI to start/monitor/evaluate RL training runs
- 1 robot identity on-chain
- Email support
- **Target**: Robot builders, small labs, indie developers
- **Cost to us**: Relay hosting (~$5/mo), HF Hub storage (free tier), dashboard server (~$10/mo)
- **Margin**: ~$5-10/mo per user

### Tier 3: Fleet ($200/month)
- Everything in Builder, plus:
- Multi-robot fleet management (up to 20 robots)
- Cross-fleet memory sync (hosted fleet.rs backend)
- SkillGate cloud inference (offload safety checks to our server)
- Priority relay (dedicated bandwidth, lower latency)
- Training compute credits (10M timesteps/month on our GPU instances)
- 5 robot identities on-chain
- Discord support
- **Target**: Research labs, robotics startups, small fleets
- **Cost to us**: GPU compute (~$100/mo), relay+infra (~$30/mo)
- **Margin**: ~$70/mo per user

### Tier 4: Enterprise (Custom pricing)
- Everything in Fleet, plus:
- TEE-attested inference (Akash Confidential Compute integration)
- On-prem relay deployment (we help them run their own)
- SLA (99.9% uptime for relay + registry)
- Custom skill auditing (we review their skills for safety)
- DAO governance integration (their org gets a seat)
- Unlimited robot identities
- Dedicated support engineer
- **Target**: Industrial deployments, defense, logistics
- **Cost to us**: Varies (TEE GPU rental, engineering time)
- **Margin**: High (services-heavy)

## Additional Revenue Streams

### Per-Skill Execution Micropayments
- x402 payment protocol (learned from OpenGradient)
- Robot pays $0.001 per skill execution to the skill creator
- We take 5% platform fee
- Enables the "robot earns tokens by doing tasks" loop (learned from RODEO)

### Skill Marketplace Transaction Fees
- When a skill is sold (not shared freely), we take 2-3% of the transaction
- Smart contract handles escrow and settlement
- Skills can be priced in $JUNO or stablecoins

### Training Compute Marketplace
- Rent GPU time for RL training (Akash or own infrastructure)
- $0.50/hour for H100, $0.20/hour for A100
- Users submit training configs, we run them and return checkpoints + ONNX
- Competes with vast.ai, runpod.io but integrated with the skill lifecycle

### Staking-Based Skill Quality Signals
- Skill creators stake tokens to back their skill's quality
- If a skill causes falls/safety violations, stake is slashed
- Higher stake = higher trust score = more visibility in the skill browser
- Learned from peaq's machine credit ratings

## What Stays Free No Matter What

1. **The open-source code** — junoclaw-physics, skill-registry contract, bridge, training scripts. MIT/Apache-2.0. Fork it, self-host it, never pay us.
2. **The on-chain skill registry** — it's a CosmWasm contract on Juno. Anyone can read/write. No gatekeeper.
3. **The Buzz relay protocol** — Nostr-based, open. Anyone can run a relay.
4. **ONNX export** — export_onnx.py is open source. The hash computation is open. No paywall on trust.

## Implementation Priority

1. **HuggingFace Hub integration** (already planned, low effort) — foundation for Builder tier
2. **Training dashboard** (medium effort) — web UI wrapping the existing training scripts
3. **Hosted relay** (low effort) — run a Buzz relay node on a VPS
4. **Per-skill micropayments** (high effort, needs x402 or similar) — research phase
5. **TEE-attested inference** (already planned via A040) — Enterprise tier enabler

## Open Questions

- Should we use TokenFactory ujclaw for subscription payments, or stablecoins?
- How does the DAO governance interact with subscription revenue?
- Should the skill marketplace be a separate contract or extend skill-registry?
- What's the minimum viable product for the Builder tier? (Probably: hosted relay + HF Hub + simple dashboard)
