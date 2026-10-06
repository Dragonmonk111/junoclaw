---
title: Security model
description: What JunoClaw's certificates guarantee, where post-quantum security applies, and what it does not yet cover.
---

## Consensus safety

- **Threshold certificates.** A block is final once a BLS12-381 threshold certificate is formed over its proposal. A validator that equivocates can only misuse its own share. It cannot forge a certificate.
- **Hybrid PQ.** When `hybrid_consensus` is on, a certificate also needs a quorum of MAYO2 signatures. To forge one, an attacker would have to break **both** BLS12-381 and MAYO2. Security = `max(classical, PQ)`. Every MAYO signature a certificate carries must verify. A certificate with even one invalid signature is rejected, so certificates cannot be altered in transit.
- **Restart-safe PQ votes.** Each validator logs its MAYO vote signatures (`mayo_sigs.log`, fsynced) and re-sends the same bytes after a restart, instead of producing a second, different signature for the same vote.
- **State binding.** The certified payload includes the executed `state_root`. Nodes that compute a different state stop (fail-stop) rather than fork.
- **Proposal validation.** Honest validators reject proposals with a wrong height, parent or `state_root`, a timestamp that is not after the parent or is more than 2 s ahead of their clock, or that are oversized. This was tested with a Byzantine proposer during the soak.

## Where PQ applies today

| Layer | Status |
|---|---|
| Consensus certificates | Hybrid BLS + MAYO2, running |
| Accounts | Hybrid secp256k1 + MAYO spends verified on devnet. ML-DSA-65 hybrid accounts planned |
| Contracts | MAYO-1/2/3/5 verification in-contract |
| P2P transport | Ed25519 (classical) |

## Caveats

- **MAYO is not a NIST standard yet.** It is a candidate in NIST's additional-signatures process. We use it for compact signatures and pair it with BLS, so a MAYO weakness alone cannot break consensus. ML-DSA-65 (FIPS 204) is the planned choice for accounts.
- **Not audited yet.** External review of the MAYO port and fuzzing of the verifiers are required before G2.
- **Devnet keys are devnet-grade.** They are derived from public seeds, so the node refuses them (and any genesis that funds or empowers a built-in devnet address) unless `insecure_devnet` is set. G1 uses a key ceremony with per-operator shares.

Report security issues privately to the core team through [Telegram](https://t.me/junoclaw). Ask in the group and a maintainer will open a private chat.
