# Telegram Message — MAYO-5 Precompile Landed (conclusive)

> Short, plain-language, receipts-first. Pairs with the Medium piece
> `MEDIUM_ARTICLE_MAYO_PRECOMPILE_MEASURED.md`. Gracious to Marius; no defensiveness.

---

**Copy-paste below:**

---

Update: the MAYO-5 precompile I said was landing — it landed. And I measured it instead of guessing.

This is bigger than a gas benchmark for me. JunoClaw is being built as an AI DAO stack: agents take work, prove what they did, earn or lose trust, write to Moultbook/shared memory, and leave receipts future agents can verify. The `jclaw-credential` trust tree is the layer that says *which agent/member made which signed claim*.

What landed now is **post-quantum receipts for that agent work**.

**What it is:** our Juno fork can now check a MAYO post-quantum signature through a native `mayo_verify` host function, instead of forcing the CosmWasm contract to emulate all the math in WebAssembly. Same attestation check, less gas — especially at the highest security level.

**The measured numbers** for one `VerifyMayoAttestation` tx, pure-Wasm → precompile, same devnet, same contract:
- MAYO-2 / NIST L1: 355,806 → 310,394 gas — 1.15×
- MAYO-3 / NIST L3: 456,682 → 257,374 gas — 1.77×
- MAYO-5 / NIST L5: 798,214 → **360,904 gas — 2.21×**

I'll be straight: I had hoped for ~7×. It is 2.21× at L5, not 7×. Why? Once the precompile takes over, the crypto is no longer the bottleneck — what's left is mostly the cost of moving the 5.5 KB public key through the contract and hashing/checking it. That is a useful finding because it tells us the next work: tune gas, batch attestations, and improve key/payload handling.

**Does this make Juno quantum-safe? No — and I won't pretend it does.**

What is quantum-safe now is narrower and real: JunoClaw can attach MAYO-signed, post-quantum attestations to agent credentials, Moultbook entries, endorsements, governance statements, or other content-level receipts.

What is **not** quantum-safe yet: Juno validator consensus, normal wallet signatures, IBC security assumptions, and network transport. Those still use classical assumptions. Making the chain itself quantum-safe needs account migration, standardized PQC host functions, validator/consensus changes, and eventually IBC-aware PQC work.

And that part is now written down as a concrete plan, not a vibe — codename **Aegis**. The approach: *migrate the live chain* instead of rebuilding one, make every step a **hybrid** (classical + post-quantum, so nothing breaks mid-transition), and root it in finalized NIST standards — **ML-DSA** for signatures, **ML-KEM** for the transport key exchange — while MAYO keeps doing the small-signature attestation job. Every cryptographic surface in a Juno/CometBFT/Cosmos node is enumerated with a per-layer migration. The honest bet: a migration on an existing chain can get *real* PQC coverage to users — transport, opt-in quantum-safe accounts, treasury keys — without waiting years for a brand-new L1. Consensus is the hard part and I won't pretend otherwise.

This is also where @Marius deserves a fair comparison. He is building the chain-native side with Falcon-1024 at the protocol layer. On consensus-level PQC and raw native verify cost, his approach wins. Full stop. I am solving a different layer: portable app-layer PQC for existing CosmWasm/Juno workflows today. A serious future stack wants both — quantum-safe chain foundations and quantum-safe application receipts.

Also: MAYO is **not** a finalized NIST standard. Falcon/FN-DSA has been selected by NIST; MAYO is still in the additional-signatures process. We are using MAYO because the signatures are small and it fits the JunoClaw credential/attestation model well, but the architecture is deliberately swappable.

**Receipts.** Everything reproduces in one command against a fresh devnet:
`FRESH=1 devnet/scripts/benchmark-mayo-devnet.sh`

Every tx hash, the host function, and the full write-up are in the repo. If you run it and get a different number, tell me — I'll publish the difference, not the round number.

To the folks who pushed me on whether I understand what I ship: fair. This is my answer — not a louder claim, a reproducible one. I build with an AI agent, I say so openly, and I check the numbers before I post them. Going from 0 to 1 looks like this.

🔗 GitHub: github.com/Dragonmonk111/junoclaw
🔗 Write-up: docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md
🔗 Medium draft: docs/MEDIUM_ARTICLE_MAYO_PRECOMPILE_MEASURED.md

— Dragonmonk / VairagyaNodes
