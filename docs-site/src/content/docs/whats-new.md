---
title: What's new
description: Dated releases and milestones for JunoClaw — chain, tooling, docs and community.
---

Releases and milestones, newest first. For what is running right now, see [Status & roadmap](/docs/status/).

## October 25, 2026

- **Node release rc16 rolled out across the G2 set.** Peer blocking is scoped to the current validator roster, block durations are clamped, stale blocks are flushed eagerly, and the metrics endpoint no longer panics when its port is busy. Every seat verifies the same binary hash before starting — `sha256 869b1fb3…c025`.
- **Certified state-sync in production.** A seat rejoined by adopting a snapshot that two independent donors advertised with the same root, then signed from its first block. Every seat now runs at least two donors with `min_anchor_agree = 2`.
- **Signer verification from node logs.** Operators can decode the signer bitmap of every certificate straight from their own logs and check that their seat is actually signing — method contributed by validator operator 79anvi, now part of the operator runsheet.
- **Website and docs refresh.** G2 status throughout, robotics + agents + DAO, a "verify it yourself" walkthrough, self-hosted fonts and stricter security headers.

## October 22, 2026

- **G2 reshare.** The validator set grew to **8 equal-weight seats** run by external operators, threshold 6 of 8. The group public key — the IBC trust root — is unchanged, so light clients never had to re-trust.

## October 23, 2026

- **First community code contributions to the node.** A validator operator contributed a Prometheus metrics endpoint and a logging-configuration fix; both ship in the rc15/rc16 line.

## October 13, 2026

- **G1 closed.** The first multi-operator ceremony: a 48.2-hour soak, 361,709 finalized blocks (heights 512,776 → 874,485), zero state divergence, post-mortem published.
- **Community infrastructure live.** Explorer, TLS gRPC API, status monitor and the open-source full-history archiver.

## October 6, 2026

- **Devnet hardening.** Invalid MAYO signatures are rejected in hybrid certificates, MAYO vote signatures survive restarts, block time is bounded by validators, state-sync anchors are verified locally, and publicly derivable devnet keys are refused outside devnet.
- **Block time 1–2 s → 0.16 s on devnet** once the state-root scan stopped walking node-local sidecar data.

## Upstream

- **Commonware.** JunoClaw's production findings feed back to the consensus library we build on — a documentation PR on certified-versus-finalized parents was merged upstream, and a fix scoping peer blocks to the active roster has been proposed there.
