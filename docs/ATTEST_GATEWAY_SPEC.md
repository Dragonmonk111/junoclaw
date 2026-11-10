# /attest Gateway — specification (g1 testnet)

Status: spec v1 — committed build target (decided 2026-11-10).
Customer-facing ingestion surface for per-action attestation on JunoClaw.
The chain produces the certificates; this service turns them into a
sellable API. Payment is fiat/USDC-metered — no proprietary token.

## What it is / is not

- **Is:** an HTTP front door that accepts a record of an agent action,
  batches it into a Merkle window, anchors the root on-chain, and returns
  a verifiable artifact: inclusion proof + covering certificate + optional
  evidence-tier references.
- **Is not:** a proof of inference, a policy engine, or a replacement for
  thought-court. It records *that an action occurred and was attested*,
  with optional pointers to evidence about *what the agent was doing*.
  Trust tiers stay explicit (t0 operator claim / economic recompute /
  t2 TEE / t3 ZK receipt) — the gateway never merges them.

## Architecture (thin slice)

```
customer ──POST /attest──▶ gateway ──queue──▶ batcher ──anchor_root──▶ merkle-verifier (or thought-court)
                │                                     ▲
                └──202 + attestation_id               └── poll /verify_proof → leaf proof + root + cert height
customer ──GET /attest/{id}──▶ {status, leaf_hash, proof, root, anchor_tx, cert}
```

Single Rust binary (`crates/junoclaw-attest-gateway`), axum, built on the
x402-gateway skeleton: it already has `CosmosClient` (query_smart +
broadcast), x402 payment envelopes with nonce-replay protection, and the
route pattern. Reuses, does not fork.

## Endpoints

| method | path | auth | purpose |
|---|---|---|---|
| POST | `/attest` | API key or x402 envelope | submit one action; idempotent |
| GET | `/attest/{id}` | same customer | poll status; returns proof when anchored |
| GET | `/attest/{id}/receipt` | same customer | full verifiable artifact (proof + cert + anchors) |
| GET | `/attest/{id}/evidence` | same customer | off-chain evidence bundle pointer (if tier ≥ t0+disclosure) |
| POST | `/attest/batch` | API key | submit up to N actions in one call (same queue) |
| GET | `/usage` | API key | per-customer metering: submitted/anchored/failed counts + window |
| GET | `/healthz` | none | liveness |
| GET | `/metrics` | ops | queue depth, anchor lag, batch stats |

## POST /attest request

```json
{
  "idempotency_key": "cust-uuid-1234",           // required; max 255 chars
  "action": {
    "kind": "tool_call|inference|message|custom",
    "payload_hash": "sha256:abc…",               // OR
    "payload": { … },                            // raw JSON; hashed server-side, NOT stored raw (see privacy)
    "canonicalization": "jcs"                     // RFC 8785 JSON Canonicalization Scheme
  },
  "context": {
    "agent_id": "cust:agent-name",
    "session_id": "optional",
    "occurred_at": 1794312000,                    // customer-claimed wall time (unverified)
    "policy_ref": "optional pointer",
    "jlens": {
      "attestation_id": "thought-court id, if a J-Lens snapshot exists",
      "bank_hash": "c550b971…",
      "integrity": 0.97
    },
    "zk_receipt": "base64, t3 tier, future"
  },
  "tier": "anchor|bonded",                        // anchor = merkle-verifier; bonded = thought-court operator bond
  "callback_url": "https://… optional webhook"
}
```

Response (always 202 — finality is never synchronous):

```json
{
  "attestation_id": "att_01JXYZ…",     // gateway ULID; also = leaf_hash for single-action submits
  "status": "queued",
  "est_anchor_secs": 60,
  "deduplicated": false
}
```

## Canonicalization & leaf hash

- If `payload` given: canonicalize per `canonicalization` (default JCS),
  `leaf = sha256(kind ‖ jcs(payload) ‖ agent_id ‖ occurred_at)`.
- If `payload_hash` given: customer asserts the hash; leaf = that hash
  bound to agent_id. Raw payload never touches us — best privacy tier.
- `attestation_id` = ULID issued by gateway; `leaf_hash` returned in
  receipt. Tree leaf = `sha256(0x00 ‖ leaf_hash)` (domain-separated).

## Idempotency & errors

- `idempotency_key` scoped per customer. Same key + same body → returns
  the original `attestation_id`, `deduplicated: true`. Same key + different
  body → `409 idempotency_conflict`. Keys retained ≥ 30 days.
- Errors: `400 bad_request`, `401 unauthorized`, `402 payment_required`
  (x402 path), `409 idempotency_conflict`, `413 payload_too_large`
  (payload cap 64 KiB — bigger must use payload_hash), `429 rate_limited`,
  `503 chain_unavailable` (retryable — queue survives restarts, redb/wal).

## Anchor pipeline

1. Queue drained on window close: 60s or 1024 leaves, whichever first
   (tune; claw-relay used :47-periodic checkpoints, same idea).
2. Build Merkle tree (SHA-256, sorted pairs — matches merkle-verifier's
   `verify_merkle_proof` convention: confirm leaf/index encoding against
   `contracts/merkle-verifier/src/contract.rs`).
3. `anchor_root { robot_id: "attest:<customer>", batch_height, merkle_root,
   cycle_count: leaves }` — robot_id is the contract's namespace key; map
   one per customer (or shared pool with prefix if key mgmt is heavy).
4. On tx confirm: write `anchor_tx`, `batch_height`, per-leaf `proof[]` +
   `leaf_index` to store; status `queued → anchored`.
5. `tier: bonded` additionally submits thought-court `--anchor` with
   operator bond — evidence tier stays separate in the receipt.

## GET /attest/{id} → receipt

```json
{
  "attestation_id": "att_01JXYZ…",
  "status": "anchored",
  "leaf_hash": "sha256:…",
  "batch": {
    "robot_id": "attest:acme", "batch_height": 4999999,
    "merkle_root": "…", "leaf_index": 7,
    "proof": ["h1","h2",…], "anchor_tx": "ABC…", "anchored_at_height": 4999999
  },
  "certificate": {
    "height": 4999999, "scheme": "hybrid-bls-mayo2",
    "cert": "portable cert bytes (fetchable from node gRPC)",
    "verify": "offline: pubkey set + junoclaw-mayo-verify"
  },
  "evidence": { "jlens_attestation_id": "…", "tier": "t0-operator" },
  "metered": { "credits": 1 }
}
```

The receipt is the sellable artifact: anyone holding the validator pubkey
set verifies the cert offline (ANODE/UTSA do this today), and the proof
verifies against the on-chain root via the contract's `verify_proof`
query — zero trust in the gateway at verification time.

## Auth & metering

- Phase 1: static per-customer API keys in config (like claw-relay's
  allowlist — admission gate, not crypto identity).
- Phase 2: x402 envelopes (crate already has them) → per-request payment;
  or NIP-98 signed requests if the customer is a Nostr-native agent.
- Metering: count per `{customer, status, day}`; `GET /usage` exposes it;
  a daily rollup job emits invoice line-items + validator-pool share.
- Revenue split accounting only (no on-chain distribution in v1): record
  `validator_pool = 0.40 × revenue`, note equal-per-active-seat division
  keyed off cert signer bitmaps — settle off-chain per the operator-
  economics doc; flag as provisional.

## Privacy & retention

- Raw `payload` is hashed and discarded immediately — store leaf_hash +
  jcs form only if `retain_payload: true` is explicitly set (default off).
- Activation blobs / jlens evidence stay OFF-chain (existing boundary);
  receipt carries ids/hashes only.
- Retention: receipts + proofs kept indefinitely (they're hashes);
  payloads ≤ 7 days if retained; idempotency keys 30 days.
- `DELETE /attest/{id}/payload` — customer-initiated purge of any
  retained payload (hashes stay — that's the point of the service).

## Failure modes

- Anchor tx fails: exponential backoff ×5 → window parked `anchor_failed`,
  ops paged; leaves stay queued (no silent drop).
- Chain halt: submissions still accepted/queued (202), `est_anchor_secs`
  inflates; `chain_unavailable` only if queue is full.
- Gateway restart: queue is redb-backed — replay on boot, in-flight window
  re-anchored idempotently (anchor_root is keyed by (robot_id,
  batch_height) — a duplicate anchor just overwrites same root).

## Build order (g1 thin slice)

1. Scaffold `crates/junoclaw-attest-gateway` from x402-gateway skeleton:
   axum + CosmosClient + redb queue. `POST /attest` → queue → ULID.
2. Batcher: 60s window → merkle → `anchor_root` on deployed
   merkle-verifier (existing contract, admin = gateway wallet).
3. `GET /attest/{id}` + receipt assembly (proof + anchor_tx + cert height).
4. Idempotency + metering counters + `/usage`.
5. API-key admission + callback webhooks + retry/backoff.
6. Bonded tier: wire thought-court anchor for premium actions.
7. Hardening: rate limits, payload caps, metrics, ops runbook.

Out of scope for v1: x402 live payment, validator on-chain payout,
zk-receipt verification (bn254 precompile path is a later milestone),
multi-customer contract namespaces (start single-tenant `attest:default`).

## Open questions

- ~~robot_id per customer vs shared pool~~ — resolved: merkle-verifier
  gates `anchor_root` on a single ADMIN key (the gateway wallet);
  `robot_id` is a free-form namespace string in the storage key
  `(robot_id, batch_height)` → per-customer namespaces are free.
- Webhook signing scheme for callbacks.
- Whether `payload` retention should ever default on (lean: no).
