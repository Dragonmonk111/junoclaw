# The Sovereign Agent Protocol — JunoClaw's Position on x402

*Strategic position document. Defines JunoClaw's relationship to Coinbase's x402 HTTP payment protocol and establishes the sovereign alternative. Created 2026-05-18.*

## Executive position

**JunoClaw is not an x402 consumer. JunoClaw is the sovereign alternative to x402.**

x402 is Coinbase's play to become the payment infrastructure of the agentic era — a corporate moat built on HTTP 402 responses, centralized facilitator verification, and EVM settlement. It is antithetical to Cosmos cyberpunk values. JunoClaw ships an x402-compatible shim as a translator for EVM-native agents, but the real protocol is on-chain, trustless, and facilitator-free.

---

## Why x402 is a moat, not a standard

### The Coinbase lock-in stack

```
Coinbase account → CDP keys → Base L2 → USDC → Permit2 → x402 facilitator
                                                              │
                                                              ▼
                                                   $0.001/tx to Coinbase
                                                   Full activity stream visible
                                                   KYC-able at any time
                                                   Revocable access
```

x402's MIT license makes it *look* open. The reality:
- **One facilitator exists** — Coinbase's. They verify payments, settle funds, mediate disputes.
- **The facilitator is the choke point** — without it, the protocol doesn't work. It's HTTPS without a CA, technically possible but practically useless.
- **EVM gravity** — Permit2 and EIP-3009 are Ethereum primitives. Every x402 implementation reinforces EVM as the default settlement layer.
- **Surveillance by design** — every API call + payment is visible to the facilitator. They know who pays whom, how often, for what. That's not a side-effect, it's the business model.

### Cosmos cyberpunk values violated

| Principle | x402 violation |
|---|---|
| **Sovereignty** | Single-point-of-failure facilitator controlled by a US corporation subject to US law, sanctions, and regulators |
| **Permissionless** | Free tier (1000 tx/month) then paywall. Coinbase decides who gets served |
| **Interchain nativity** | Settlement is EVM. Cosmos agents must bridge OUT to pay IN |
| **Community ownership** | MIT code, corporate repository, corporate SDK, corporate facilitator — the standard IS Coinbase |
| **Cypherpunk privacy** | Full activity stream visible to facilitator. Every agent action logged |
| **Anti-extraction** | $0.001/tx × billions of agent interactions = Coinbase's new revenue stream. Pure rent |

---

## The JunoClaw Protocol — the sovereign answer

### Design principles

1. **No facilitator.** The chain IS the facilitator. Verification is cryptographic (Groth16 + BN254), not institutional.
2. **No corporate dependency.** Every component is open-source, self-hostable, and community-governed.
3. **Funds never leave Cosmos.** Settlement in JUNO / IBC-USDC on `juno-1`. No bridges to EVM.
4. **Privacy by default.** Agent identity is pseudonymous. Task activity is public (chain), but linkage to real-world identity is opt-in.
5. **Gas is the only fee.** ~0.02 JUNO per task operation. Paid to validators who secure the network — not to a corporation.
6. **IBC-native interoperability.** Any Cosmos chain can post/accept tasks via IBC packet forwarding. No HTTP middlemen for chain-to-chain.

### The protocol vs x402 — side by side

| Concern | x402 (Coinbase) | JunoClaw Protocol (sovereign) |
|---|---|---|
| Payment request | HTTP 402 response + JSON envelope | On-chain `task-ledger::PostTask` (public, permissionless) |
| Verification | Centralized facilitator checks signature | On-chain `zk-verifier` checks Groth16 proof (trustless math) |
| Settlement | Permit2 transfer on Base/Polygon | `escrow` contract atomic release on juno-1 |
| Identity | Coinbase account / wallet address tracked by facilitator | Soulbound `agent-registry` entry — pseudonymous, reputation-bearing |
| Discovery | HTTP APIs (DNS-dependent, censorable) | Chain state queries + optional Nostr signaling (censorship-resistant) |
| Fee | $0.001/tx to Coinbase | Gas only (~0.02 JUNO to validators) |
| Governance | Coinbase ships updates; you comply or break | DAO-governed via `agent-company` — VK rotation, parameter changes, constraint vocabulary all proposal-gated |
| Knowledge sharing | Not addressed | `moultbook` contract — ZK-attested knowledge without value-key exposure |
| Cross-chain | EVM only (Base, Polygon, Arbitrum, World, Solana) | IBC-native — any Cosmos chain natively |
| Censorship resistance | Facilitator can blacklist any agent | On-chain — validators include txs by gas, not by identity |

### What the x402 gateway IS (and isn't)

The `crates/junoclaw-x402-gateway` is:
- ✅ A **compatibility translator** for EVM-native agents who already speak x402
- ✅ Proof that we can interoperate without depending
- ✅ A gateway that settles on juno-1, NOT on Base

It is NOT:
- ❌ The protocol itself
- ❌ A dependency
- ❌ Required for Cosmos-native agents
- ❌ Using Coinbase's facilitator

**Narrative frame:** "We speak x402 the way a multilingual person speaks English — for interoperability, not because it's the home language."

---

## Viable sovereign alternatives explored

### Path 1: Pure on-chain (what we ship in v1)

Agent → `task-ledger` → `escrow` → `zk-verifier` → settlement. No HTTP layer. No facilitator. The chain is everything.

**Pros:** Maximum sovereignty, minimum attack surface, zero corporate dependency.
**Cons:** Non-Cosmos agents can't interact without learning Cosmos signing.

### Path 2: Nostr signaling + Cosmos settlement

- Task announcements as Nostr events (custom kind, structured content)
- Acceptance/submission on-chain
- Discovery via Nostr relays (permissionless, censorship-resistant, self-hostable)

**Pros:** Decentralized discovery, no DNS dependency, aligned with cypherpunk values.
**Cons:** Nostr relay ecosystem is immature; relay incentivisation is unsolved.

### Path 3: IBC middleware for cross-chain task routing

- `ibc-task-relay` module forwards task operations as IBC packets
- Agent on Osmosis claims JunoClaw task on Juno via IBC
- Settlement is atomic cross-chain via ICS-20 transfer

**Pros:** True interchain sovereignty, no bridges, light-client security.
**Cons:** Requires IBC middleware development; limited to Cosmos ecosystem.

### Path 4: x402-compatible shim (Option D — what we built)

- Speak x402 at the HTTP layer for EVM-native agent compatibility
- Gateway settles on juno-1 (sovereign facilitator)
- Cosmos-native agents skip the gateway entirely

**Pros:** Interoperates with the growing EVM agent ecosystem.
**Cons:** HTTP layer is censorable (DNS, TLS). Must be clear this is a shim, not the protocol.

### Path 5: HTLC-based payment channels (Lightning-inspired)

- ZK proof hash = HTLC preimage
- Agent reveals proof → contract unlocks escrow
- No facilitator, no HTTP, pure conditional payments

**Pros:** Minimal attack surface, battle-tested pattern (Lightning Network).
**Cons:** Less flexible than the full task-ledger (one-shot, not stateful workflow).

### Chosen architecture (v1)

**Primary:** Path 1 (pure on-chain) — the core protocol, always available, zero dependency.
**Secondary:** Path 4 (x402 shim) — compatibility layer for EVM-native agents.
**Future (v2):** Path 3 (IBC middleware) + Path 2 (Nostr signaling) — extend sovereignty cross-chain.

---

## The moultbook innovation — something x402 can't do

x402 solves "pay for API access." It doesn't solve "share knowledge safely across agents without exposing your value-bearing keys."

The **moultbook** (10th JunoClaw contract) solves this:

### What moultbook does

An agent "moults" — sheds knowledge into a public commons — without risking the keys that hold its funds or reputation. The mechanism:

1. **Derived identity.** Agent holds a primary key `K` (funds, reputation in `agent-registry`). Agent derives a moult-key `K'` via deterministic derivation (BIP-32 child path or HKDF). `K'` has NO funds, NO signing authority for value operations.

2. **ZK proof of derivation.** Agent generates a Groth16 proof that "`K'` is derived from a registered agent in `agent-registry`" WITHOUT revealing `K`. The circuit proves set membership (agent is registered) + key derivation (K' comes from K) without identity linkage.

3. **Publish to moultbook.** Agent writes knowledge entries to the `moultbook` contract using `K'`. Each entry is: topic hash, content hash (IPFS CID), ZK attestation that the author is a registered agent. The moultbook records:
   - The knowledge hash
   - The moult-key that authored it
   - The ZK proof that this moult-key belongs to a registered agent
   - Timestamp (block height)

4. **Verification.** Any reader can verify: "this knowledge was authored by a legitimate registered agent" WITHOUT knowing WHICH agent. Trust comes from the proof, not from identity revelation.

### Why this matters for the agent economy

- **Knowledge commons without doxxing.** An agent that found a profitable MEV strategy can share the strategy's existence (for reputation/altruism) without exposing the key that executes it.
- **Sybil-resistant.** The ZK proof guarantees one entry per registered agent per topic (or configurable cardinality). You can't flood the moultbook with fake knowledge.
- **Value-key isolation.** Even if a moult-key is compromised (unlikely — it's ephemeral), no funds are at risk. The primary key is never exposed.
- **Trust accumulation without identity.** Over time, a moult-key builds a track record of useful knowledge contributions. The agent can optionally link it to their primary identity later (by revealing the derivation path), or stay pseudonymous forever.

### Why x402 can't replicate this

x402 requires identity-linked payments. There's no mechanism for anonymous-but-verified knowledge sharing. The facilitator sees everything. You can't "moult" — you're permanently linked to your Coinbase-verified identity.

JunoClaw's ZK infrastructure (the same `zk-verifier` that settles tasks) enables a new primitive: **anonymous attestation of set membership**. "I am a registered agent" without "I am agent X." This is only possible because we have on-chain Groth16 verification as a first-class primitive.

---

## Supply-chain sovereignty

Beyond protocol design, sovereignty means owning the full build chain:

| Layer | Sovereign choice | NOT sovereign |
|---|---|---|
| Language | Rust (open toolchain, reproducible, auditable) | Go/JS (corporate stewardship, npm supply chain) |
| Runtime | CosmWasm (community-governed, no corporate kill switch) | Solidity/EVM (Ethereum Foundation decisions) |
| Settlement | juno-1 (community chain, no VC funding, no corp treasury) | Base/Polygon (corporate L2s) |
| Proving | Groth16 + BN254 (open math, no trusted setup per-circuit with PLONK as v2) | Opaque "AI verification" |
| Distribution | OCI artifacts on GHCR + cosign-signed (open registry, portable) | npm / corporate registries |
| Identity | Secp256k1 keys + soulbound registry (self-custodied) | OAuth / SSO / corporate identity providers |
| Discovery | Chain queries + Nostr (censorship-resistant) | DNS + HTTP APIs (censorable, revocable) |
| Hosting | Akash (decentralized compute) | AWS / GCP / Vercel (corporate cloud) |

JunoClaw's supply chain is maximally sovereign. The only non-sovereign component is GitHub (hosting the repo + GHCR). Migration path: Radicle (sovereign git) + self-hosted OCI registry.

---

## Narrative positioning for the article / public communication

**Frame:** "x402 is the tollbooth. JunoClaw is the open road."

- x402 says: "Pay Coinbase to access any API."
- JunoClaw says: "Prove your work cryptographically. Get paid trustlessly. Share knowledge safely. No middlemen."

**Key messages:**
1. The agentic era needs payment infrastructure. Coinbase is right about the problem, wrong about the solution.
2. A facilitator is a rent-seeker. On-chain ZK verification is the alternative: trustless, permissionless, sovereign.
3. JunoClaw is 10 contracts + a WAVS operator that replaces x402's entire stack with pure math and open code.
4. The moultbook is something x402 structurally cannot offer: anonymous-but-verified knowledge commons.
5. We speak x402 for interoperability. We don't depend on it.

---

*Apache-2.0. Created 2026-05-18.*
