# Telegram Reply: Jake Hartnell — Aegis hybrid transport live + full PQC migration design

*Draft 2026-06-25 · For Telegram reply to Jake Hartnell*

---

Hey Jake —

Following up on the PQC thread. Two things landed this week, one measured-live, one design.

**1. Quantum-safe P2P transport, running on a real junod build.**

I built a `junod-aegis` (Juno v29 + a CometBFT fork) where the P2P handshake is hybrid **X25519 + ML-KEM-768** (stdlib `crypto/mlkem`, gated behind an env flag so the classical path is byte-for-byte untouched). Ran it on a 4-node localnet, classical vs hybrid transport:

```
classical transport   commit_bytes = 2,265
hybrid   transport   commit_bytes = 2,265   (identical)
```

The headline: **the PQC handshake costs zero consensus bandwidth.** It protects the link, not the payload — so the cost is handshake CPU + one ML-KEM ciphertext round at connection setup, and nothing per-block. Blocks produced, peers connected, consensus healthy the whole time.

That's the cheap, low-risk surface. The expensive one is **consensus signatures** — hybrid Ed25519+ML-DSA-44 validator keys, which I measured separately at **6.71× commit size** (2,267 → 15,208 B for a 4-val set). That's the real Q-day cost, and it's bandwidth, not CPU (ML-DSA verify is ~100µs).

**2. Full migration design — no flag day at any layer.**

The whole point is that you can cross to PQC *without halting the chain or breaking IBC*. I wrote up three ADRs:

- **Consensus (ADR-008):** heterogeneous validator set — classical and hybrid validators coexist; each signature verifies against the signer's registered key type. Hybrid `Address()` = classical address, so **no state migration**.
- **IBC (ADR-009):** the `07-tendermint` light client stays in place (no new client type, no channel re-handshake). A counterparty that hasn't upgraded keeps verifying the **classical half** of every hybrid signature, so channels stay open through the whole transition.
- **Accounts (ADR-010):** existing accounts go hybrid **without changing address or moving funds** — a staged opt-in ladder (register → dual-required → pqc-only) where you must prove the PQC key works *before* enforcement turns on, so you can't lock yourself out.

The common thread is the one you'd care about for DAO DAO / cw4: **the address never changes**, so the agent-DAO member roster, delegations, and contract allow-lists all survive the upgrade untouched.

Same ML-DSA-44 primitive across transport, consensus, accounts, and the on-chain CosmWasm attestations (the MAYO/ML-DSA precompile work) — one vendored impl, no new crypto in the trust base.

Write-up + repro coming. Curious whether the hybrid-transport-first, consensus-later sequencing matches how you'd think about rolling this out.

— Dragonmonk / VairagyaNodes

---

**Notes for posting:**
- Lead with the measured transport result (it's the concrete, live thing); consensus 6.71× is the honest "real cost" framing.
- ADR numbers: 008 consensus, 009 IBC, 010 account migration. 006 transport / 007 account-type already exist.
- The "address never changes" point is the DAO-relevant hook for Jake (cw4-group roster).
- Honesty flags: ML-DSA-44 = NIST FIPS 204 finalized (cat 2); transport handshake is env-gated, additive, classical path unchanged.
- Attribution: Dragonmonk / VairagyaNodes.
