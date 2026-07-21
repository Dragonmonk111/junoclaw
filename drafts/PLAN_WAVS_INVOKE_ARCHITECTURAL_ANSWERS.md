# WAVS Off-Chain Invoke API — Architectural Best Answers

*Our recommended answers to the 6 open questions, informed by the existing codebase. These are the designs we'd implement if Jake says "go build it."*

---

## Q1: Is off-chain invoke on the WAVS roadmap already, or would this be a new contribution?

**Best answer: New contribution, but with a clear precedent in the existing code.**

The WAVS runtime already has two trigger paths in `wavs/src/trigger.rs`:

```rust
pub enum TriggerData {
    CosmosContractEvent(CosmosEvent),  // production: on-chain event
    Raw(Vec<u8>),                       // local testing: JSON bytes
    // ...
}
```

The `Raw` variant already accepts arbitrary JSON, deserializes it into a `VerificationTask`, and runs the component. The invoke API is essentially **promoting `TriggerData::Raw` from a testing path to a production path** — same component code, same task parsing, same `process_task` dispatch. The only new infrastructure is the HTTP listener, auth layer, and allowlist.

This means the contribution is narrow: we're not changing how components execute, we're adding a new *entry point* that feeds the existing execution pipeline. The component's `fn run(action: TriggerAction) -> Result<Option<WasmResponse>, String>` doesn't change at all.

---

## Q2: Authentication preference — signed HTTP vs. mTLS vs. JWT?

**Best answer: Bearer token (existing pattern) for v1, signed HTTP as v2 enhancement.**

The bridge already has a production-grade HTTP auth server (`wavs/bridge/src/admin/rpc-server.ts`) with:

- **Bearer token auth** (≥32 bytes, constant-time compared via `timingSafeEqual`)
- **Loopback-only binding** (rejects `0.0.0.0`, `::`, non-localhost)
- **Host header check** (anti-DNS-rebinding)
- **Origin header rejection** (anti-CSRF)
- **In-memory rate limiting** (default 10 req / 60s, fires before auth)
- **Audit logging** (JSON-line, token never in logs)
- **64 KiB body cap**

For v1, reuse this exact pattern: bearer token in `Authorization` header, loopback-only, rate-limited. The relayer and the WAVS daemon share a secret (like `JUNOCLAW_ADMIN_TOKEN` today). This is the fastest path to production and reuses proven code.

For v2 (when we want remote callers or multi-tenant), upgrade to **signed HTTP** (secp256k1 over canonical `method + path + timestamp + body_hash`):

```
Authorization: WAVS-Signature <juno_addr>:<sig_hex>
X-WAVS-Timestamp: <unix_ms>
```

This reuses Cosmos key infrastructure the relayer already has, doesn't require TLS certificate management (mTLS), and doesn't introduce a JWT issuer/verifier dependency. The daemon verifies the signature against the caller's Juno address and checks it against the allowlist.

**Why not mTLS:** requires cert distribution and rotation. The relayer is a single process with a Cosmos key, not a fleet with a PKI. Overkill for v1.

**Why not JWT:** introduces an issuer/verifier dependency and token refresh logic. The bearer token pattern already works and is proven in the codebase.

---

## Q3: Attestation in synchronous mode — per-invocation quote or batch?

**Best answer: Per-invocation, but lightweight — reuse the existing attestation hash, not a full SGX quote.**

The component already produces a `VerificationResult` with:

```rust
VerificationResult {
    task_type: "store_signed_tx".to_string(),
    data_hash: signed.sign_doc_sha256_hex,      // SHA-256 of the SignDoc
    attestation_hash: compute_attestation_hash(   // SHA-256 of (task_type + data_hash)
        "store_signed_tx", &data_hash
    ),
    output: serde_json::json!({ ... }),
    timestamp: current_timestamp(),
}
```

For the invoke API, the response includes `data_hash` and `attestation_hash` directly. These are deterministic SHA-256 hashes computed inside the TEE — they **are** the attestation. The relayer submits `attestation_hash` to `agent-company::SubmitAttestation` after broadcasting, same as today.

**Full SGX quote timing:** a full SGX quote (DCAP/ECDSA) is expensive (~100ms-1s) and doesn't need to be in the synchronous response path. The quote is a property of the **enclave measurement**, not of individual invocations. The measurement is established at component load time and doesn't change per call. So:

1. **At daemon startup:** the WAVS daemon obtains/verifies the TEE quote and records the measurement hash. This is a one-time cost.
2. **Per invocation:** the response includes `attestation_hash` (cheap SHA-256) + `measurement_hash` (from the daemon's startup verification). The relayer can optionally verify the measurement matches what's registered on-chain.
3. **On-chain:** `SubmitAttestation` records the `attestation_hash` + `measurement_hash`. The contract already checks the submitter is the authorized `wavs_operator`.

This means **no per-invocation SGX quote overhead** — just the existing SHA-256 hashing that already happens inside the component.

---

## Q4: Generic vs. scoped — any component or `invoke_enabled: true`?

**Best answer: Scoped, via `service.json` workflow config.**

Add an `invoke` field to the existing workflow config in `service.json`:

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

**Why scoped, not generic:**

- Not every workflow makes sense off-chain. `DataVerify` (fetch URLs, hash content) is triggered by on-chain proposals for a reason — the DAO voted to request it. Allowing arbitrary off-chain invocation would bypass governance.
- The sealed signer is different: it's a **service component** (sign on request) not a **governance-triggered component** (compute on vote). The `invoke.enabled` flag makes this distinction explicit.
- Allowlist per-workflow means different callers can be authorized for different components. The relayer can invoke `sign-request` but not `sign-moultbook-export`.
- Rate limiting per-workflow prevents abuse without global throttling.

**Default: `invoke` absent or `enabled: false`.** Existing workflows are unaffected. Only workflows that explicitly opt in get the invoke endpoint.

---

## Q5: WAVS daemon architecture — existing HTTP server or new listener?

**Best answer: Extend the existing admin RPC server pattern, but as a separate listener with its own auth.**

The bridge already runs an HTTP server (`rpc-server.ts`) when `JUNOCLAW_ADMIN_RPC=1`. It uses Node's `http.createServer`, binds to loopback, and has all the security primitives (bearer token, rate limit, host check, audit log).

For the invoke API, we have two options:

**Option A (recommended): New endpoint on the same server.**

Add `POST /invoke/:componentId` to the existing admin RPC server. Reuses all security primitives. The bearer token already protects it. The rate limiter already throttles it. The audit log already records it.

Pros: zero new infrastructure. Cons: couples invoke availability to `JUNOCLAW_ADMIN_RPC=1` being set.

**Option B: Separate `wavs-invoke` sidecar process.**

A new Node/TypeScript process that only handles invoke requests. Has its own auth, its own rate limiter, its own port. Talks to the WAVS daemon via IPC or the existing component execution path.

Pros: isolation — if the invoke listener is compromised, it doesn't have admin RPC access. Cons: another process to manage, another port to configure, IPC between processes.

**Recommendation: Option A for v1** (fastest, reuses proven code), **Option B for production hardening** (defense in depth). The existing `rpc-server.ts` is already designed to be extended — it's a router pattern with per-endpoint handlers.

**Key insight from the codebase:** the bridge's `processResult` function (`bridge.ts`) already dispatches results to submission functions. The invoke path would bypass the event-watcher → component → bridge → submit pipeline and instead go: HTTP request → component → HTTP response. The component execution itself (`lib.rs::run()`) is unchanged — only the trigger source and response destination differ.

---

## Q6: Does adding an HTTP input path change the component's measurement?

**Best answer: No — and we can prove it.**

The component's measurement (SGX MRENCLAVE) is a hash of the compiled `.wasm` binary. The trigger source is **not part of the component** — it's part of the WAVS runtime that *calls* the component.

Here's the proof from the existing architecture:

1. **The component exports `fn run(TriggerAction) -> Result<Option<WasmResponse>, String>`** — this is the WIT interface, compiled into the `.wasm`. It doesn't change based on how `TriggerAction` was constructed.

2. **`TriggerData::Raw` already exists** — the component already handles raw JSON input in testing. The `decode_trigger` function parses it into the same `VerificationTask` enum. The component code is identical whether the trigger came from an on-chain event or raw JSON.

3. **The WAVS runtime constructs `TriggerAction`** — the runtime (not the component) decides whether to parse an on-chain event or an HTTP request body. The component just receives a `TriggerAction` and processes it.

4. **`wasm-tools component wit`** would show the same WIT interfaces regardless of trigger source. The component's imports (`wasi:io`, `wasi:cli`, `wasi:random`, `wasi:keyvalue`) don't change.

**Verification step (for SGX re-run):** build the component with `cargo component build --release --target wasm32-wasip2`, hash the resulting `.wasm`, then enable the invoke path and rebuild. The hash should be identical. If it is, the measurement is unchanged. If it isn't, something in the build config leaked into the component (which would be a bug to fix, not a design problem).

---

## Architecture summary

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

## Implementation priority (if Jake says go)

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

**Total: ~10-14 days** for a working production path. The bulk of the work is reusing existing infrastructure (rpc-server.ts security, decode_trigger parsing, process_task execution) and wiring it to a new entry point.
