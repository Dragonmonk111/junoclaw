---
title: FAQ
description: Short answers to common questions about JunoClaw.
---

**Is JunoClaw a Cosmos SDK chain?**
No. It is a Rust chain built on Commonware primitives. It exposes Cosmos-style gRPC and runs CosmWasm, so Cosmos tooling and habits mostly carry over.

**Is it related to Juno?**
Yes. It is built by the Juno Agents DAO and came out of the Juno ecosystem. Juno governance props #373 and #374 passed.

**Is there a token?**
The chain's denom is `ujclaw` and is used for gas. No token sale exists, and none is announced. Testnets are unpaid. We don't discuss price.

**Why "post-quantum" if it still uses BLS?**
Certificates need a quorum of BLS **and** MAYO2 signatures. A future quantum attacker who breaks BLS would still have to forge MAYO2.

**How fast is it?**
On the G2 public testnet blocks finalize at ~0.36 s on average across external operators (measured over ~18,000 blocks on 25 Oct 2026); the G1 soak's best sustained hour was 0.11 s — the certificate covers the executed state either way. Speed is not our pitch, though. The combination of fast finality, PQ certificates and a light-client path is.

**Can I run a validator?**
G2 is onboarding external operators in batched set-change windows. See the [validator call](/docs/onboarding/validators/).

**I don't have a static IP. Can I still join?**
Yes. Peers dial each other at a fixed `IP:7001`, so you need a stable public address, but it doesn't have to be at home. A small cloud VM works, or a cheap VPS that forwards TCP `7001` to your home node over WireGuard. Cloudflare Tunnel is not a good fit for P2P traffic.

**Is there slashing?**
Not at the consensus layer. Stake and slashing live in the truth-market contracts.

**Where is the code?**
Chain: [`Dragonmonk111/layer-sdk` @ `commonware`](https://github.com/Dragonmonk111/layer-sdk/tree/commonware). Contracts and agent stack: [`Dragonmonk111/junoclaw`](https://github.com/Dragonmonk111/junoclaw). Dual-licensed MIT OR Apache-2.0.
