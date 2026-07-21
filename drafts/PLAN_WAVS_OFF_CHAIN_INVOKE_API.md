# WAVS Off-Chain Invoke API — Plan & Spec

**Goal:** Add an on-demand invocation path to the WAVS runtime so components can be called directly via signed HTTP requests without an on-chain trigger event. This eliminates the M2 sign-request round-trip and collapses the sealed signer flow to a simple direct-call architecture.

**Status:** Draft spec. Needs DM to Jake Hartnell / WAVS team for review and endorsement.

---

## 1. Problem statement

WAVS today is strictly event-driven: every component execution starts from an on-chain Cosmos contract event that the WAVS indexer picks up. This works for attestation workflows (trigger → compute → submit result on-chain) but breaks down for workflows that need to produce signed data *before* that data exists on-chain.

The sealed signer is the canonical example. To sign a Moultbook post transaction, the enclave must construct and sign a `TxRaw` before the post is submitted. With no off-chain trigger, M2 works around this with a 3-message on-chain round-trip:

1. `RequestSignedTx` (relayer → agent-company, emits `sign_request` event)
2. `StoreSignedTx` (WAVS bridge → agent-company, stores signed tx bytes)
3. `AckBroadcastTx` (relayer → agent-company, cleans up)

This adds ~330-650k gas, ~30-60s latency, a stuck-pending failure mode, and a one-at-a-time throughput ceiling per signed post. All of this overhead exists solely because there's no direct invocation path.

---

## 2. Proposed API: `wavs invoke`

### 2.1 HTTP endpoint

```
POST /invoke/{component_id}
Content-Type: application/json
Authorization: WAVS-Signature <addr>:<sig_hex>

{
  "trigger": "sign_request",
  "input": {
    "sender": "juno1...",
    "contract": "juno1...",
    "exec_msg_json": "{...}",
    "funds_denom": "ujuno",
    "funds_amount": "0",
    "gas_limit": 200000,
    "fee_denom": "ujuno",
    "fee_amount": "5000",
    "memo": "sealed signer post",
    "chain_id": "uni-7",
    "account_number": 42,
    "sequence": 7
  }
}
```

**Response (200 OK):**

```json
{
  "output": {
    "tx_bytes": "<base64>",
    "sign_doc_sha256_hex": "<hex>",
    "signer_address": "juno1..."
  },
  "attestation": {
    "quote_hash": "<hex>",
    "measurement": "<hex>",
    "component_id": "junoclaw:verifier"
  }
}
```

### 2.2 CLI subcommand

```bash
wavs invoke --component junoclaw:verifier \
  --trigger sign_request \
  --input '{"sender":"juno1...","contract":"juno1...","exec_msg_json":"{...}",...}' \
  --signer-key ~/.juno/relayer-key.pem
```

### 2.3 Authentication

The caller must prove identity so the WAVS daemon can authorize or deny access:

- **Signed HTTP request:** The caller signs a canonical string (`method + path + timestamp + body_hash`) with their secp256k1 key. The signature is verified against the caller's Juno address.
- **Allowlist:** The WAVS daemon config (`service.json` or a new `invoke_policy.json`) specifies which caller addresses may invoke which components. For the sealed signer, only the configured relayer address is authorized.
- **Rate limiting:** The daemon enforces a configurable rate limit per caller to prevent abuse.

### 2.4 Security properties preserved

| Property | Event-driven (current) | Off-chain invoke (proposed) |
|---|---|---|
| Component runs inside TEE | ✅ | ✅ |
| Key never leaves enclave | ✅ | ✅ |
| Attestation produced | ✅ (submitted on-chain by bridge) | ✅ (returned in response, relayer submits via `SubmitAttestation`) |
| Caller authorized | ✅ (on-chain contract checks relayer role) | ✅ (daemon checks signed request + allowlist) |
| Result integrity | ✅ (bridge submits to contract) | ✅ (relayer verifies attestation before broadcasting) |
| Trigger auditability | ✅ (on-chain event) | ⚠️ New: daemon logs invoke requests; attestation still recorded on-chain |

The key security shift: authorization moves from the on-chain contract (which checks the relayer's address in `RequestSignedTx`) to the WAVS daemon (which checks the signed HTTP request against an allowlist). The TEE attestation is still produced and still recorded on-chain — the trust anchor doesn't change, only the trigger mechanism.

---

## 3. Simplified sealed signer flow (post-invoke-API)

```
relayer              WAVS daemon / TEE           chain
   │                       │                       │
   │── POST /invoke ─────→ │                       │
   │   (signed HTTP)       │                       │
   │                       │── run component ──→   │
   │                       │   (inside TEE)        │
   │                       │   sign tx             │
   │                       │←── tx_bytes + attest  │
   │←── 200 OK ────────────│                       │
   │   (tx_bytes + quote)  │                       │
   │                       │                       │
   │── broadcastTx ─────────────────────────────→  │
   │                       │                       │
   │── SubmitAttestation ──────────────────────→   │
   │   (quote_hash)        │                       │
```

**What gets removed from `agent-company`:**
- `RequestSignedTx` message + handler
- `StoreSignedTx` message + handler
- `AckBroadcastTx` message + handler
- `SignRequest` struct + `SignRequestStatus` enum
- `SIGN_REQUESTS` / `PENDING_SIGN_REQUEST` / `SIGN_REQUEST_SEQ` storage
- `GetSignRequest` / `ListSignRequests` queries
- The one-pending-at-a-time constraint
- Gas/fee guardrails (moved to the invoke allowlist config)

**What stays:**
- `relayer` address config (now used by WAVS daemon allowlist, not contract)
- `sealed_signer` address config (relayer validates response signer matches)
- `SubmitAttestation` (relayer submits the TEE quote after broadcasting)

**What gets removed from `moultbook.js`:**
- `parseSignRequestId()`
- `pollForSignedTx()`
- `requestSignedTx()` (replaced by direct HTTP call to WAVS daemon)
- `ackBroadcastTx()`

**What gets added:**
- `invokeSealedSigner()` — signed HTTP POST to WAVS daemon `/invoke` endpoint
- `submitAttestation()` — submits the returned quote hash to `agent-company`

---

## 4. Implementation plan

### Phase A: Spec review with WAVS team (1-2 weeks)

1. **DM Jake** with this spec document and the concrete use case.
2. Ask: does this fit the WAVS roadmap? Is there a contribution path?
3. Incorporate feedback on:
   - Authentication mechanism (signed HTTP vs. mTLS vs. token-based)
   - Whether the invoke path should be generic (any component) or scoped to specific trigger types
   - How attestation works in the synchronous response path (does the TEE produce a quote per invocation, or batch?)
   - Whether the daemon needs a new WASI interface for receiving off-chain input, or if the existing trigger parsing can be reused

### Phase B: WAVS runtime implementation (2-4 weeks, if accepted)

1. Add HTTP server to WAVS daemon (or a sidecar `wavs-invoke` service).
2. Implement signed request verification (secp256k1 signature over canonical request string).
3. Implement allowlist config (`invoke_policy.json` or extend `service.json`).
4. Wire invoke path to existing component execution (reuse trigger parsing + `process_task`).
5. Return synchronous response with component output + attestation metadata.
6. Add rate limiting + logging.

### Phase C: JunoClaw simplification (1-2 weeks, after Phase B)

1. Remove `RequestSignedTx`/`StoreSignedTx`/`AckBroadcastTx` from `agent-company` contract.
2. Remove `SignRequest` state machine and storage.
3. Update `moultbook.js` to use `invokeSealedSigner()` direct HTTP call.
4. Update `wavs/service.json` to declare invoke policy (allowlisted relayer address).
5. Keep `SubmitAttestation` for recording TEE quotes on-chain.
6. Run full integration test: relayer → invoke → broadcast → attestation.

### Phase D: Documentation + community (1 week)

1. Update `PHASE3_TEE_SEALED_SIGNER_AND_J_LENS_ENDGAME.md` to reflect simplified architecture.
2. Update M2 article (or write M2.1 follow-up) explaining the simplification.
3. If Jake/WAVS team endorses, cross-post the spec and implementation as a WAVS ecosystem contribution.
4. This is also a concrete example of Juno Agents DAO contributing back to WAVS infrastructure — worth highlighting in governance comms.

---

## 5. Questions for Jake — with our recommended answers

Each question includes our best architectural answer, informed by the existing codebase. Jake can react to concrete proposals rather than answering from scratch.

### Q1: Is off-chain invoke on the WAVS roadmap already, or would this be a new contribution?

**Our recommendation: New contribution, with a clear precedent in the existing code.**

The WAVS runtime already has two trigger paths in `wavs/src/trigger.rs`:

```rust
pub enum TriggerData {
    CosmosContractEvent(CosmosEvent),  // production: on-chain event
    Raw(Vec<u8>),                       // local testing: JSON bytes
}
```

The `Raw` variant already accepts arbitrary JSON, deserializes it into a `VerificationTask`, and runs the component. The invoke API is essentially **promoting `TriggerData::Raw` from a testing path to a production path** — same component code, same task parsing, same `process_task` dispatch. The component's `fn run(action: TriggerAction)` doesn't change at all. The contribution is narrow: a new entry point that feeds the existing execution pipeline.

### Q2: Authentication preference — signed HTTP vs. mTLS vs. JWT?

**Our recommendation: Bearer token for v1 (reuse existing pattern), signed HTTP for v2.**

The bridge already has a production-grade HTTP auth server (`wavs/bridge/src/admin/rpc-server.ts`) with:
- Bearer token auth (≥32 bytes, constant-time compared via `timingSafeEqual`)
- Loopback-only binding (rejects `0.0.0.0`, `::`, non-localhost)
- Host header check (anti-DNS-rebinding) + Origin header rejection (anti-CSRF)
- In-memory rate limiting (default 10 req / 60s, fires before auth)
- Audit logging (JSON-line, token never in logs)
- 64 KiB body cap

For v1, reuse this exact pattern. The relayer and WAVS daemon share a secret (like `JUNOCLAW_ADMIN_TOKEN` today). Fastest path to production, reuses proven code.

For v2 (remote callers, multi-tenant), upgrade to **signed HTTP** (secp256k1 over canonical `method + path + timestamp + body_hash`), reusing Cosmos key infrastructure the relayer already has. Not mTLS (overkill cert management) or JWT (issuer/verifier dependency).

### Q3: Attestation in synchronous mode — per-invocation quote or batch?

**Our recommendation: Per-invocation, but lightweight — reuse the existing attestation hash, not a full SGX quote.**

The component already produces a `VerificationResult` with `data_hash` (SHA-256 of the SignDoc) and `attestation_hash` (SHA-256 of task_type + data_hash). These are deterministic hashes computed inside the TEE — they **are** the attestation. The relayer submits `attestation_hash` to `agent-company::SubmitAttestation` after broadcasting, same as today.

A full SGX quote (DCAP/ECDSA) is expensive (~100ms-1s) and is a property of the **enclave measurement at component load time**, not of individual invocations. The measurement doesn't change per call. So:
1. **At daemon startup:** obtain/verify the TEE quote, record measurement hash (one-time cost).
2. **Per invocation:** response includes `attestation_hash` (cheap SHA-256) + `measurement_hash` (from startup verification).
3. **On-chain:** `SubmitAttestation` records both. Contract already checks submitter is authorized `wavs_operator`.

**No per-invocation SGX quote overhead** — just the existing SHA-256 hashing.

### Q4: Generic vs. scoped — any component or `invoke_enabled: true`?

**Our recommendation: Scoped, via `service.json` workflow config.**

Add an `invoke` field to the existing workflow config:

```json
{
  "name": "sign-request",
  "component": { "source": "..." },
  "trigger": { ... },
  "submit": { ... },
  "invoke": {
    "enabled": true,
    "allowed_callers": ["juno1relayer..."],
    "rate_limit_per_minute": 10,
    "max_body_bytes": 65536
  }
}
```

Not every workflow makes sense off-chain. `DataVerify` (fetch URLs, hash content) is triggered by on-chain proposals for a reason — the DAO voted to request it. Allowing arbitrary off-chain invocation would bypass governance. The sealed signer is a **service component** (sign on request), not a **governance-triggered component** (compute on vote). The `invoke.enabled` flag makes this distinction explicit.

**Default: `invoke` absent or `enabled: false`.** Existing workflows are unaffected.

### Q5: WAVS daemon architecture — existing HTTP server or new listener?

**Our recommendation: Extend the existing admin RPC server for v1, separate sidecar for v2.**

The bridge already runs an HTTP server (`rpc-server.ts`) when `JUNOCLAW_ADMIN_RPC=1` — Node's `http.createServer`, loopback-bound, with all security primitives. Add `POST /invoke/:componentId` to this server. Zero new infrastructure.

For production hardening (v2), split into a separate `wavs-invoke` sidecar process with its own auth and port — defense in depth. The existing `rpc-server.ts` is designed as a router pattern with per-endpoint handlers, so extension is straightforward.

**Key insight:** the bridge's `processResult` function already dispatches results to submission functions. The invoke path bypasses the event-watcher → component → bridge → submit pipeline and instead goes: HTTP request → component → HTTP response. The component execution itself (`lib.rs::run()`) is unchanged.

### Q6: Does adding an HTTP input path change the component's measurement?

**Our recommendation: No — and we can prove it.**

The component's measurement (SGX MRENCLAVE) is a hash of the compiled `.wasm` binary. The trigger source is **not part of the component** — it's part of the WAVS runtime that *calls* the component.

Proof from the existing architecture:
1. The component exports `fn run(TriggerAction) -> Result<Option<WasmResponse>, String>` — this WIT interface is compiled into the `.wasm`. It doesn't change based on how `TriggerAction` was constructed.
2. `TriggerData::Raw` already exists — the component already handles raw JSON input in testing. `decode_trigger` parses it into the same `VerificationTask` enum. Component code is identical regardless of trigger source.
3. The WAVS runtime (not the component) decides whether to parse an on-chain event or an HTTP request body. The component just receives a `TriggerAction`.
4. `wasm-tools component wit` shows the same WIT interfaces regardless of trigger source. Component imports (`wasi:io`, `wasi:cli`, `wasi:random`, `wasi:keyvalue`) don't change.

**Verification step:** build with `cargo component build --release --target wasm32-wasip2`, hash the `.wasm`, enable invoke path, rebuild. Hash should be identical.

---

## 6. Why this matters beyond JunoClaw

The sealed signer is the first WAVS use case that needs to produce signed data *before* that data exists on-chain. But it won't be the last. Any WAVS workflow that acts as an oracle (sign something, then submit it) has the same problem. An off-chain invoke API would unlock:

- **Signed data feeds:** a component that signs external data (prices, weather, API responses) and returns it to a relayer for submission.
- **Off-chain computation with on-demand attestation:** a component that does heavy computation (ML inference, ZK proof generation) on request rather than on every block.
- **Multi-step agent workflows:** a component that produces intermediate signed results that feed into the next step before any on-chain submission.

This is a WAVS runtime feature, not a JunoClaw feature. Building it makes WAVS more useful for the entire ecosystem.

---

## 7. Architecture summary

```
                    ┌─────────────────────────────────┐
                    │       WAVS daemon process        │
                    │                                  │
  relayer ──POST──→ │  ┌──────────────────────────┐   │
  (bearer token)    │  │  invoke HTTP listener     │   │
                    │  │  (extends rpc-server.ts)  │   │
                    │  │  - bearer token auth      │   │
                    │  │  - rate limit             │   │
                    │  │  - allowlist check        │   │
                    │  │  - audit log              │   │
                    │  └─────────┬────────────────┘   │
                    │            │                     │
                    │  ┌─────────▼────────────────┐   │
                    │  │  trigger parser           │   │
                    │  │  (reuse decode_trigger)   │   │
                    │  │  TriggerData::Raw(json)   │   │
                    │  └─────────┬────────────────┘   │
                    │            │                     │
                    │  ┌─────────▼────────────────┐   │
                    │  │  component execution      │   │
                    │  │  (unchanged: fn run())    │   │
                    │  │  inside TEE               │   │
                    │  └─────────┬────────────────┘   │
                    │            │                     │
                    │  ┌─────────▼────────────────┐   │
                    │  │  VerificationResult       │   │
                    │  │  - data_hash (SHA-256)    │   │
                    │  │  - attestation_hash       │   │
                    │  │  - output (tx_bytes)      │   │
                    │  └─────────┬────────────────┘   │
                    │            │                     │
  relayer ←─200─── │  ┌─────────▼────────────────┐   │
  (tx_bytes +       │  │  JSON response            │   │
   hashes)           │  └──────────────────────────┘   │
                    └─────────────────────────────────┘
```

**What changes:** new HTTP endpoint + allowlist config in `service.json`.
**What doesn't change:** component code, WIT interface, TEE measurement, crypto, attestation hashing, `SubmitAttestation` on-chain.

---

## 8. Implementation priority (if Jake says go)

| Priority | What | Effort | Dependency |
|---|---|---|---|
| 1 | Add `POST /invoke/:componentId` to existing `rpc-server.ts` | 2-3 days | None — extends existing code |
| 2 | Add `invoke` config to `service.json` schema + parser | 1 day | #1 |
| 3 | Wire invoke path to `TriggerData::Raw` + `process_task` | 1-2 days | #2 |
| 4 | Return `VerificationResult` as JSON in HTTP response | 1 day | #3 |
| 5 | Update `moultbook.js` with `invokeSealedSigner()` | 1 day | #4 |
| 6 | Remove `RequestSignedTx`/`StoreSignedTx`/`AckBroadcastTx` from `agent-company` | 1 day | #5 tested |
| 7 | Integration test: relayer → invoke → broadcast → SubmitAttestation | 2-3 days | #6 |
| 8 | SGX measurement re-verification (hash .wasm before/after) | 1 day | #7 |

**Total: ~10-14 days** for a working production path. The bulk of the work reuses existing infrastructure (rpc-server.ts security, decode_trigger parsing, process_task execution) and wires it to a new entry point.
