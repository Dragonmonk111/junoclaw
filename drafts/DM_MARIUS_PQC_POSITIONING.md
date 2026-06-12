# Draft: Public PQC Positioning (Telegram/Twitter)

---

Two builders, two approaches, same mission: post-quantum crypto on chain.

@marius is building a custom BFT stack with **Falcon DSA-1024** from the ground up. Highest NIST security level. Native speed. Validator signatures that survive Shor's algorithm. Signature size: 1,280 B (large, but acceptable for consensus). Timeline: 6-12 months to open source.

We're taking the other path: **MAYO-2 in a CosmWasm contract**. Smallest PQC signatures in the NIST finals: **186 B**. Pure Rust, `#![no_std]`, deployable on any CosmWasm chain *today* without a fork, without a governance vote, without asking validators to upgrade. Live on `uni-7` right now.

**The tradeoff is real:**

| | Falcon-1024 (Marius) | MAYO-2 (JunoClaw) |
|---|---|---|
| Security | NIST Level 5 (vault) | NIST Level 1 (padlock) |
| Sig size | 1,280 B | 186 B |
| Deploy | New L1 only | Any CosmWasm chain |
| Timeline | 6-12 months | **Today** |

**Our paths are complementary, not competing.**

Marius is solving the *future chain* problem — greenfield L1 with PQC consensus from genesis. We're solving the *existing chain* problem — 20+ CosmWasm chains that need PQC attestations, governance, identity *now*, without waiting for a hard fork.

When his stack is open source, we'll integrate Falcon as a precompile on our BN254-patched Juno fork. Then you get both: vault-level security when you need it, padlock-level portability when you don't.

Until then: if you need PQC on Juno, Osmosis, Neutron, or any CosmWasm chain today, MAYO-2 is live and working.

Contracts on `uni-7`:
- `jclaw-credential`: `juno1z2w...el2r` (Bud + VerifyMayoAttestation)
- `moultbook`: `juno1nm0...dx4` (MAYO-signed attestations)

Repo: `github.com/junoclaw/junoclaw`

---

*Short version for replies:*

> Thanks for sharing the details @marius — serious respect for building a BFT stack from scratch with Falcon-1024. That's the right architecture for a greenfield L1.
>
> Our paths are complementary: you're solving the **consensus layer** (validator signatures natively). We're solving the **application layer** (smart contract attestations in wasm). CometBFT on Juno still signs blocks with Ed25519 — our contract just verifies MAYO signatures for agent identity, governance votes, and moultbook entries.
>
> **MAYO-5 is on the roadmap** — streaming `expand_pk` to handle the larger parameter set within wasm memory limits. That gets us to NIST Level 5 (same security class as Falcon) while keeping the small signature size.
>
> Vaults and padlocks. Both needed. Excited to see what you ship.
