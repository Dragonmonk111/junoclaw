---
title: Status & roadmap
description: What is running, what is tested, and what is planned — kept honest and current.
---

*Last updated: October 12, 2026.*

## Running

| Area | Detail |
|---|---|
| Consensus | Commonware Simplex BFT, 4-seat validator set across 3 independent operators, threshold BLS12-381 certificates |
| PQ hybrid | `hybrid_consensus`: BLS partial + MAYO2 signature per vote, quorum on both |
| Finality | ~3.5 s median cadence on a distributed quorum; **0.11 s** best sustained hour measured on the G1 soak. The certificate binds the executed `state_root`. A finality record (certificate, proposal, timestamp) is stored per height |
| Validation | Proposals checked for height, parent, `state_root`, size and wall-clock timestamp (after the parent, at most 2 s ahead). Byzantine proposer tested |
| Mempool | Dedupe, recheck after commit, strict sequence admission |
| Sync | Height-range backfill, certified state-sync with a multi-peer anchor quorum, locally verified anchor certificate and a capped download size |
| Storage | Durable payload store, tx index, role-tier pruning: validators keep ~24 h (540,000 heights) of payloads and finality records, RPC nodes ~30 days, archives everything. Peers backfill across the whole window |
| gRPC | `BroadcastTx`, `GetTx`, `Simulate`, bank/wasm queries |
| Contracts | CosmWasm, in-contract MAYO-1/2/3/5 + Groth16 verification |
| Accounts | secp256k1 and hybrid secp256k1 + MAYO |
| Agent layer | Buzz relay channels live at `buzz.junoclaw.xyz`. The 8-contract agent stack runs on the devnet |

## Public surface (community-run)

| Endpoint | What it is |
|---|---|
| `explorer.junoclaw.xyz` | Live explorer — every block with its real hybrid certificate: signer bitmap, BLS aggregate, MAYO2 signatures (`tools/jc-explorer`, open source) |
| `api.junoclaw.xyz` | TLS-terminated public gRPC edge — light-client, CDC stream, Cosmos queries |
| `status.junoclaw.xyz` | Independent 30 s probe monitor with public uptime history (`tools/jc-status`) |
| CDC archiver | `tools/cdc-archiver` — open-source follower mirroring every committed state change into a durable archive, with typed projections for address-level search |

## Tested

- **G1 soak — 48.2 hours, 361,709 blocks (heights 512,776 → 874,485).** The first multi-operator ceremony. **Zero state divergence** across the whole window. **One documented wedge (106 min):** a home-ISP outage removed two seats sharing a failure domain; the chain halted rather than forked (BFT correct), recovered fully after clock correction + restarts, and the root cause + timeline are published. A later ~3.5-minute freeze self-recovered unattended.
- **24 h chaos soak (C9).** Kill/restart, partitions, container recreates, Byzantine proposer. 15 events, zero state divergence.
- **Soak findings fixed.** IP re-pin on reconnect (runbook), bounded backfill re-requests (code + regression test), `app_hash` now equals `state_root` (verified identical across validators after redeploy).
- **October 6 hardening.** The devnet was upgraded in place to commit `90451d8`, with no state divergence:
  - Hybrid certificates carrying an invalid MAYO signature are rejected.
  - MAYO vote signatures survive restarts via a signature log.
  - Block time is the proposer's wall clock, bounded by validators.
  - State-sync anchors are verified locally, and snapshot downloads are capped.
  - Nodes refuse publicly derivable devnet keys and genesis unless `insecure_devnet` is set.
- **October 6, evening.** The state-root scan now skips node-local sidecar data, and block time fell from 1–2 s to 0.16 s. Finality records are pruned by tier, and backfill reaches across the whole pruning window.
- **Agent economy end to end.** The 8-contract stack (agent-company, agent-registry, task-ledger, escrow, truth-market, marketplace, moultbook, skill-registry) runs on the devnet: onboarding, hires, escrow, verdicts, slashing, refunds and provenance. Latest run: **122 checks, 0 failures.**
- **Two exploits found and fixed on the devnet.** The e2e probe reproduced a spoofed escrow payment hook and a marketplace verdict not bound to its hire. Both were fixed and migrated in place, and the probe now asserts the attacks fail. Escrow expiry, dispute resolution, the registry fee sweep and hook caps are live too.

## Gates

| Gate | Scope | Status |
|---|---|---|
| G0 | Devnet hardened | **Done** |
| G1 | Closed testnet: multi-operator ceremony, 48.2 h soak, verdict + post-mortem published | **Done** |
| G2 | Public testnet: external operators (independent failure domains), faucet, wallet gateway, PQ verifier fuzzing + external review | **In progress — onboarding** |
| Post-G2 | ML-DSA-65 hybrid accounts, PQ finality checkpoints, PQ light client for IBC | Planned |

## Known gaps (tracked openly)

- Adversarial gas metering for worst-case Wasm inputs is not fully measured yet.
- The PQ verifier has not been fuzzed or externally reviewed yet. Both are G2 requirements.
- Wallet (Keplr/cosmjs) compatibility is not verified.
- Validator set changes are manual, coordinated upgrades. There is no in-protocol set change.
