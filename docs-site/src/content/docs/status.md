---
title: Status & roadmap
description: What is running, what is tested, and what is planned — kept honest and current.
---

*Last updated: October 6, 2026.*

## Running on devnet

| Area | Detail |
|---|---|
| Consensus | Commonware Simplex BFT, 4 validators, threshold BLS12-381 certificates |
| PQ hybrid | `hybrid_consensus`: BLS partial + MAYO2 signature per vote, quorum on both |
| Finality | 0.16 s per block on the 4-validator devnet (single host; G1 will measure real networks). The certificate binds the executed `state_root`. A finality record (certificate, proposal, timestamp) is stored per height |
| Validation | Proposals checked for height, parent, `state_root`, size and wall-clock timestamp (after the parent, at most 2 s ahead). Byzantine proposer tested |
| Mempool | Dedupe, recheck after commit, strict sequence admission |
| Sync | Height-range backfill, certified state-sync with a multi-peer anchor quorum, locally verified anchor certificate and a capped download size |
| Storage | Durable payload store, tx index, role-tier pruning: validators keep ~24 h (540,000 heights) of payloads and finality records, RPC nodes ~30 days, archives everything. Peers backfill across the whole window |
| gRPC | `BroadcastTx`, `GetTx`, `Simulate`, bank/wasm queries |
| Contracts | CosmWasm, in-contract MAYO-1/2/3/5 + Groth16 verification |
| Accounts | secp256k1 and hybrid secp256k1 + MAYO |
| Agent layer | Buzz relay channels live at `buzz.junoclaw.xyz`. The 8-contract agent stack runs on the devnet |

## Tested

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
| G1 | Closed testnet: 3–5 external validators, key ceremony, genesis, monitoring | **Recruiting** |
| G2 | Public testnet: faucet, status page/explorer, wallet gateway, PQ verifier fuzzing + external review | Planned |
| Post-G2 | ML-DSA-65 hybrid accounts, PQ finality checkpoints, PQ light client for IBC | Planned |

## Known gaps (tracked openly)

- Adversarial gas metering for worst-case Wasm inputs is not fully measured yet.
- The PQ verifier has not been fuzzed or externally reviewed yet. Both are G2 requirements.
- Wallet (Keplr/cosmjs) compatibility is not verified.
- Validator set changes are manual, coordinated upgrades. There is no in-protocol set change.
