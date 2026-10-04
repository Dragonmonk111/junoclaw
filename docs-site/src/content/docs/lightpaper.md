---
title: Lightpaper
description: JunoClaw in one read — the problem, the chain, the agent layer, the evidence and the road ahead.
---

*Version 1.0 — October 2026. Every claim here points at something running, tested, or measured. Anything planned is labelled as planned.*

## 1. The problem

Software agents are starting to hold funds, sign transactions and act for people. The chain they run on has to give them three things at once:

1. **Finality an agent can reason about.** A transaction is either final or it isn't. No confirmations, no reorgs.
2. **Signatures that hold up against quantum computers.** Agent keys and the chain's own certificates will need to stay valid for decades.
3. **Proofs a machine can check.** Agents and light clients should verify the chain's state from compact certificates, without replaying history.

Most chains offer one or two of these. Fast chains often certify transaction *order* first and attest to *state* later. Post-quantum work is mostly limited to accounts or still on roadmaps. Light clients usually have to follow header chains.

## 2. The chain

JunoClaw is a lean Layer-1 written in Rust on [Commonware](https://commonware.xyz) primitives. It runs a Simplex-family BFT consensus protocol.

| Property | How it works |
|---|---|
| **Finality** | Each finalized block has a threshold BLS12-381 certificate. Finality takes about 1 s and is deterministic. |
| **What gets certified** | The block payload binds the executed `state_root`, so the certificate covers the result of the block, not only its order. |
| **Post-quantum hybrid** | When `hybrid_consensus` is on, every vote carries a BLS partial and a MAYO2 signature (186 bytes). A certificate needs a quorum of both. Security is `max(classical, PQ)`. |
| **Light-client path** | Each certificate is chain-linked to the previous one, so you can verify history from certificates. |
| **Execution** | Deterministic KV + CosmWasm execution. Nodes stop on divergence instead of forking. |
| **Sync** | Certified state-sync snapshots with a multi-peer anchor quorum, height-range backfill, and a durable payload store. |
| **Interfaces** | Cosmos-style gRPC: `BroadcastTx`, `GetTx`, `Simulate`, and bank/wasm queries. |

### Validator model

- **Equal weight, fixed set.** The validator set is set at genesis, and every validator has one equal share. Changing the set is a coordinated config and binary upgrade.
- **Consensus has no stake.** Safety comes from the BLS threshold. A validator that equivocates can only corrupt its own share and cannot forge a certificate. Economic stake sits in the application layer (truth markets).
- **Two keys, one file.** Each validator's `keys.json` holds a BLS share (or BLS + MAYO2 in hybrid mode) and an Ed25519 P2P identity key.

## 3. The agent layer

The chain is kept deliberately thin. The product is what agents do on top of it:

- **Buzz.** Agent coordination channels (governance, dev, robotics, truth-market). It runs today on a relay at `buzz.junoclaw.xyz`. *Planned:* a chain-native replacement built on Commonware p2p after G1.
- **Truth markets and J-Lens.** Agents bond stake on claims, and J-Lens probes resolve them. Operators who diverge get slashed at this layer.
- **On-chain PQ and ZK verification.** Contracts verify MAYO-1/2/3/5 and Groth16. Hybrid secp256k1 + MAYO account spends have been committed on devnet.
- **Sealed signer.** A TEE component keeps keys inside the enclave and signs its own transactions on-chain.
- **Robotics attestation.** Locomotion policies trained in simulation, with on-chain attestation of the policies.

## 4. Evidence

The devnet ran a **24-hour chaos soak**: rotating kill/restart, live 90–180 s network partitions, container recreates, and a Byzantine proposer injecting faults. All 4 validators ran hybrid consensus.

- **15 chaos events. Zero state divergence.** The monitor compared certified payload digests across validators, and those digests bind the Merkle `state_root`.
- **Two real bugs found by the soak and fixed.** One was a silent wedge from IP reassignment after a partition (ops side). The other was a backfill request flood after recovery (chain side, regression-tested).
- **One hygiene fix.** `app_hash` used to be a rolling write-history hash, so it could differ on a node restored from state-sync even when its state was identical. It now reports `state_root` directly, and all validators show the same value. Consensus was never affected.

The post-soak image is deployed, and the devnet keeps running chaos.

## 5. Honest comparisons

| | Ethereum | Cosmos appchains | Sei Giga | JunoClaw |
|---|---|---|---|---|
| Finality | ~13–15 min economic | ~3–6 s per block | ~250 ms, ordering only | ~1 s, certifies state root |
| PQ posture | account-level research | none shipped | none | quorum-level, running |
| Light client | sync committee | header chains | — | chain-linked BLS certs |
| Thesis | world computer | sovereign zones | trading | verifiable agents |

Speed alone is not the differentiator. What sets JunoClaw apart is the combination: ~1 s state-certifying finality, post-quantum certificates at the quorum level, and a certificate-chain light-client path, all in one small codebase. In exchange, we give up the ecosystem gravity of the big stacks.

## 6. Roadmap

| Gate | Scope | Status |
|---|---|---|
| **G0 — Devnet hardened** | Consensus validation, mempool v2, state-sync, hybrid consensus, 24 h chaos soak | **Done** |
| **G1 — Closed testnet** | 3–5 invited external validators, key ceremony, genesis, monitoring | **Recruiting now** |
| **G2 — Public testnet** | Faucet, status page / explorer, public docs, fuzzing and external review of the PQ verifier | Planned |
| **Post-G2** | ML-DSA-65 hybrid accounts, PQ finality checkpoints, PQ light client for IBC | Planned |

## 7. Governance

JunoClaw is built by the **Juno Agents DAO** and grew out of the Juno ecosystem. Juno governance proposals **#373** (JunoClaw: verifiable AI agents) and **#374** (BN254 precompile for CosmWasm) both passed. The code is dual-licensed **MIT OR Apache-2.0**.

*Discipline: nothing in this document is aspirational unless it says so.*
