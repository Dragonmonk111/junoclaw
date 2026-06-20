# Telegram Reply: Jake/Marius — L5 PQC Benchmark Table

*Draft 2026-06-20 · For Telegram reply to Jake Hartnell / Marius*

---

Hey Jake, Marius —

Quick update on the PQC front: we got NIST Level 5 MAYO-5 verification running live on Juno's uni-7 testnet this week. No fork, no new chain — just a CosmWasm contract.

**Measured gas (whole-tx, uni-7 testnet):**

| Variant | NIST Level | Sig Size | Verify Gas (wasm) | Verify Gas (precompile) |
|---------|-----------|----------|-------------------:|------------------------:|
| MAYO-2 | L1 | 186 B | 356k | 310k (1.15×) |
| MAYO-3 | L3 | 681 B | 457k | 257k (1.77×) |
| MAYO-5 | L5 | 964 B | 799k | 361k (2.21×) |

For context: a DeFi swap on Juno costs ~1M gas. So L5 PQC verification costs less than a swap.

MAYO-5 sigs are 25% smaller than Falcon-1024 (964 B vs 1,280 B) at the same NIST Level 5. Tradeoff: bigger PK (5,554 B vs 1,792 B), but we hash-store it on-chain (32 B permanent state).

Marius — your native Falcon-1024 path will always win on raw gas (~10k vs 361k). But our point stands: for anyone who needs PQC attestations on an existing Cosmos chain today, without waiting for a new L1 or a governance vote, this works now. Different tools, different jobs.

Full write-up + repro instructions coming shortly. Happy to share the contract address if anyone wants to try it.

— Dragonmonk / VairagyaNodes

---

**Notes for posting:**
- Keep the table compact for Telegram (monospace block)
- MAYO is "additional signatures (in process)" — not a finalized NIST standard
- Falcon/FN-DSA was selected by NIST — acknowledge this
- Attribution: Dragonmonk / VairagyaNodes
- Contract address on uni-7: `juno1zj39neajvynzv4swf3a33394z84l6nfduy5sntw58re3z7ef9p4q3w4y47`
