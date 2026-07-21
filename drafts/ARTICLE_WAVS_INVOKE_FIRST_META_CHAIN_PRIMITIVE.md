# The WAVS Off-Chain Invoke API — The First Meta-Chain Primitive

*Summary of the proposed WAVS off-chain invoke API, framed in the context of Ethan Frey's Meta-Chain vision. Ethan has been articulating a hybrid blockchain paradigm for years — merging web2 and web3, where applications aren't purely on-chain or purely off-chain but a deliberate composition of both. The sealed signer is the first WAVS use case that requires this hybrid model, and the invoke API is the runtime primitive that makes it clean.*

---

## What Ethan proposed

Ethan Frey — the creator of CosmWasm and architect of the meta-chain concept — has been arguing for a paradigm shift in how we think about blockchain applications. Instead of forcing everything on-chain (expensive, slow, limited computation) or keeping everything off-chain (fast, cheap, but untrusted), the meta-chain model deliberately splits an application across both layers:

- **On-chain:** settlement, governance, verification, trust anchoring.
- **Off-chain:** computation, data access, cryptographic operations, anything that needs speed or privacy.
- **The bridge:** cryptographic attestations that let the on-chain layer verify what happened off-chain without re-executing it.

This is the "merge web2 and web3" thesis. Web2 gives you synchronous computation, rich APIs, real-time responses. Web3 gives you trust minimization, governance, verifiable state. The meta-chain says: use both, each for what it's best at, and cryptographically link them.

Ethan built CosmWasm to make on-chain smart contracts portable and composable. The meta-chain article extends that thinking: the *application* isn't the smart contract — it's the combination of off-chain computation and on-chain verification, linked by attestations.

---

## What WAVS does today

WAVS (WebAssembly Verification System) is the runtime that already implements half of Ethan's vision:

- **Off-chain computation in TEE:** WASI components run inside Intel SGX or AWS Nitro enclaves. The computation is attested — the TEE produces a cryptographic quote proving the code ran unmodified.
- **On-chain attestation recording:** the WAVS bridge submits attestation hashes to `agent-company` on-chain. The contract verifies the submitter is authorized and stores the attestation.
- **Event-driven triggers:** every component execution starts from an on-chain Cosmos contract event.

The missing half is **on-demand invocation**. Today, WAVS components can only run when an on-chain event triggers them. There's no way to call a component directly — no HTTP endpoint, no CLI subcommand, no "just run this and give me the result" path.

---

## What the invoke API adds

The proposed `wavs invoke` API (`drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md`) is the missing half. It adds a direct invocation path:

```
relayer → POST /invoke/{component} (authenticated HTTP) → component runs in TEE → response with output + attestation hash → relayer submits result on-chain
```

This is the meta-chain pattern, concretely:

| Meta-chain concept | WAVS invoke API implementation |
|---|---|
| Off-chain computation | WASI component runs inside TEE (SGX/Nitro) |
| On-chain verification | `SubmitAttestation` records attestation hash in `agent-company` |
| Cryptographic bridge | TEE attestation quote + SHA-256 attestation hash |
| On-demand execution | `POST /invoke/:componentId` HTTP endpoint |
| Trust minimization | Component code is attested; key never leaves enclave; caller authorized via allowlist |
| Governance control | `service.json` per-workflow `invoke` config with `allowed_callers` |

---

## Why the sealed signer is the first use case that demands it

Most WAVS workflows today fit the event-driven model: a DAO proposal triggers a verification task, the component computes, the bridge submits the result. The trigger and the result are both on-chain. This works because the computation is a *response* to something that already happened on-chain.

The sealed signer breaks this model. To sign a Cosmos SDK transaction, the enclave must construct and sign a `TxRaw` **before** that transaction is submitted on-chain. There's no on-chain event to trigger on — the transaction doesn't exist yet. The enclave needs to produce signed data *before* that data exists on-chain.

This is the fundamental pattern of any **oracle-like** WAVS workflow: the component produces signed data that will *later* be submitted on-chain, not data that *responds to* something already on-chain. Examples:

- **Signed data feeds:** a component signs external data (prices, weather, API responses) for a relayer to submit.
- **Off-chain computation with attestation:** a component does heavy computation (ML inference, ZK proof generation) on request, not on every block.
- **Multi-step agent workflows:** a component produces intermediate signed results that feed into the next step before any on-chain submission.

All of these need the same thing: a way to call a WAVS component directly, get the result, and submit it on-chain later. The invoke API is the primitive that unlocks all of them.

---

## The current workaround and its cost

Without the invoke API, M2 works around the limitation with an on-chain round-trip through `agent-company`:

1. Relayer submits `RequestSignedTx` → contract emits `sign_request` event (~150-300k gas)
2. WAVS picks up the event → component signs inside TEE → bridge submits `StoreSignedTx` (~100-200k gas)
3. Relayer polls for the signed tx → broadcasts → calls `AckBroadcastTx` (~80-150k gas)

**Total overhead per signed post:** ~330-650k extra gas, ~30-60s extra latency, one-pending-at-a-time throughput ceiling, and a stuck-pending failure mode.

All of this exists solely because there's no direct invocation path. The invoke API collapses this to:

```
relayer → invoke (HTTP) → get tx_bytes → broadcast → SubmitAttestation
```

Three steps instead of seven. No pending state machine. No polling. No gas overhead for the round-trip. The on-chain footprint shrinks to just the transaction itself plus one `SubmitAttestation` message.

---

## How it connects to Ethan's thesis

Ethan's meta-chain article argues that the *application* is the combination of off-chain and on-chain, not just the smart contract. The WAVS invoke API makes this concrete for the first time in the WAVS ecosystem:

1. **The application is not the contract.** The application is the sealed signer (off-chain, in TEE) + `agent-company` (on-chain, governance) + the invoke API (the bridge between them). No single layer is the application; the application is the composition.

2. **Trust is not all-or-nothing.** The relayer doesn't need to trust the WAVS daemon — it verifies the attestation hash. The on-chain contract doesn't need to trust the relayer — it verifies the submitter is authorized. The DAO doesn't need to trust the enclave operator — it verifies the TEE measurement. Each layer trusts only what it can verify.

3. **Computation moves off-chain; verification stays on-chain.** The heavy work (key management, transaction construction, cryptographic signing) happens in the TEE. The light work (attestation recording, authorization) happens on-chain. This is the cost optimization Ethan describes: don't pay on-chain gas for computation that can be attested instead of re-executed.

4. **The bridge is attestations, not messages.** The on-chain round-trip (M2) uses on-chain messages as the bridge — `RequestSignedTx`, `StoreSignedTx`, `AckBroadcastTx`. The invoke API replaces messages with attestations: the TEE produces a SHA-256 attestation hash, the relayer submits it once, the contract verifies it. One attestation instead of three messages.

---

## What we're proposing to Jake and the WAVS team

The full spec (`drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md`) includes:

- **HTTP API design:** `POST /invoke/:componentId` with bearer token auth, rate limiting, allowlist
- **6 architectural questions with our recommended answers**, informed by the existing codebase:
  - Q1: New contribution (but `TriggerData::Raw` already proves the path)
  - Q2: Bearer token for v1 (reuse existing `rpc-server.ts`), signed HTTP for v2
  - Q3: Per-invocation SHA-256 attestation hash, full SGX quote only at startup
  - Q4: Scoped per-workflow via `service.json` `invoke` config, default disabled
  - Q5: Extend existing admin RPC server for v1, separate sidecar for v2
  - Q6: No measurement change (trigger source is runtime, not component — provable)
- **Architecture diagram** showing the full flow
- **4-phase implementation plan:** spec review → WAVS runtime impl → JunoClaw simplification → docs + community
- **Implementation priority table:** ~10-14 days total, most work reuses existing infrastructure

The DM to Jake has been sent. The architectural answers are included so he can react to concrete proposals.

---

## Why this matters beyond JunoClaw

The sealed signer is the first WAVS use case that needs off-chain invocation, but it won't be the last. Any WAVS workflow that produces signed data before it exists on-chain has the same problem. The invoke API is a WAVS runtime feature, not a JunoClaw feature — it makes WAVS itself more useful for the entire ecosystem.

This is the first concrete implementation of the meta-chain pattern for WAVS: off-chain computation in TEE, on-demand invocation via HTTP, on-chain verification via attestation. If Ethan and Jake have been talking about hybrid blockchains for years, this is the first use instance that provides a working, tested, deployable implementation of that vision.

The DAO can be the one that builds it.
