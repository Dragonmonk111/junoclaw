# junoclaw-x402-gateway

An HTTP / [x402](https://docs.cdp.coinbase.com/x402/welcome) façade in front of the JunoClaw nine-contract stack. Lets non-Cosmos-native autonomous agents post tasks, accept claims, and submit attestations against a JunoClaw `agent-company` DAO via a familiar HTTP 402 idiom.

## Status

**v0.1.0 — alpha.** ADR-002 accepted 2026-05-17. Designed to ship alongside the JunoClaw mainnet deploy.

This crate is **not** a Coinbase x402 facilitator. It speaks x402-shaped envelopes but settles on `juno-1` directly. See [`docs/ADR-002-X402-COMPOSITION.md`](../../docs/ADR-002-X402-COMPOSITION.md) for the design and [`docs/X402_RISK_ANALYSIS.md`](../../docs/X402_RISK_ANALYSIS.md) for the threat model.

## Architecture

```
[Off-chain agent (LLM, bot, ...)]
   │ POST /tasks
   │ POST /tasks/:id/accept
   │ POST /tasks/:id/submit
   ▼
[junoclaw-x402-gateway (this crate)]
   │ 1. mint PaymentEnvelope → 402 response
   │ 2. agent signs Cosmos tx locally and retries
   │ 3. validate envelope (binding + expiry + nonce)
   │ 4. broadcast tx to juno-1
   ▼
[juno-1 chain → agent-company → task-ledger / escrow / zk-verifier]
```

Two-phase flow on every state-changing endpoint:

1. **First call** (no `PAYMENT-SIGNATURE` header): gateway responds `402 Payment Required` with a `PaymentEnvelope` body — chain ID, contract address, exec msg, funds, gas estimate, fee, nonce, expiry, anti-tamper binding hash.
2. **Retry call** (with `PAYMENT-SIGNATURE: base64(signed_tx)` + `PAYMENT-ENVELOPE: <json>` headers): gateway validates the envelope, records the nonce, broadcasts the tx, returns `200` with the tx hash.

Read-only endpoints (`GET /tasks/:id`, `GET /agents/:addr`, `GET /healthz`, `GET /metrics`) skip the 402 dance.

## Endpoints

| Method | Path | Two-phase | Description |
|---|---|---|---|
| `GET`  | `/healthz` | no | Liveness probe |
| `GET`  | `/metrics` | no | Operational counters (nonce-store size, etc.) |
| `POST` | `/tasks` | yes | Post a new task. Body: `{description, constraints, verifying_key_hash, reward, deadline_height}` |
| `GET`  | `/tasks/{id}` | no | Query task state by `task_id` |
| `POST` | `/tasks/{id}/accept` | yes | Agent claims an open task |
| `POST` | `/tasks/{id}/submit` | yes | Agent submits Groth16 proof + public inputs |
| `GET`  | `/agents/{addr}` | no | Agent reputation from `agent-registry` |

## Build & run

```bash
# Build
cargo build --release -p junoclaw-x402-gateway

# Run locally against mainnet RPC + a deployed agent-company
GATEWAY_AGENT_COMPANY=juno1agentcompanyaddr... \
GATEWAY_KEY_PATH=/secure/path/to/op.key \
./target/release/junoclaw-x402-gateway

# Or via Docker
docker build -t ghcr.io/dragonmonk111/junoclaw-x402-gateway:0.1.0 \
    -f crates/junoclaw-x402-gateway/Dockerfile .
docker run --rm -p 8402:8402 \
    -e GATEWAY_AGENT_COMPANY=juno1agentcompanyaddr... \
    -e GATEWAY_KEY_PATH=/secure/op.key \
    -v /secure:/secure:ro \
    ghcr.io/dragonmonk111/junoclaw-x402-gateway:0.1.0
```

Required environment variables:

| Variable | Description |
|---|---|
| `GATEWAY_BIND` | HTTP bind (default `0.0.0.0:8402`) |
| `GATEWAY_CHAIN_ID` | Cosmos chain ID (default `juno-1`) |
| `GATEWAY_RPC` | Tendermint RPC URL |
| `GATEWAY_GAS_PRICE` | Gas price string (default `0.075ujuno`) |
| `GATEWAY_AGENT_COMPANY` | **Required.** `agent-company` contract address |
| `GATEWAY_KEY_PATH` | **Required.** Operator key file path (NEVER an env-var-embedded key) |
| `GATEWAY_RATE_LIMIT_RPM` | Requests/min per IP (default 60; 0 disables) |
| `GATEWAY_MAX_TASK_UJUNO` | Hard ceiling on JUNO rewards the gateway will broker (default 1000 JUNO) |
| `GATEWAY_ENVELOPE_TTL_SEC` | Anti-replay window (default 300s) |

## Example session

```bash
# Phase 1: ask to post a task → receive 402
curl -i -X POST http://localhost:8402/tasks \
    -H "content-type: application/json" \
    -d '{
        "description": "verify-credential-batch-2026-05",
        "constraints": "credverify-v0.4 max=50",
        "verifying_key_hash": "sha256:deadbeef...",
        "reward": [{"denom":"ujuno","amount":"100000000"}],
        "deadline_height": 12345678
    }'
# HTTP/1.1 402 Payment Required
# Content-Type: application/json
# { "version": "1", "scheme": "cosmos-direct", "chain_id": "juno-1", ... }

# Phase 2: sign the tx client-side, retry with the envelope echoed back
curl -X POST http://localhost:8402/tasks \
    -H "content-type: application/json" \
    -H "PAYMENT-ENVELOPE: $(cat envelope.json)" \
    -H "PAYMENT-SIGNATURE: $(base64 -w0 signed_tx.bin)" \
    -d '{...same body as phase 1...}'
# HTTP/1.1 200 OK
# { "status":"broadcast", "tx_hash":"ABCD...", "nonce":"f47a..." }
```

## Tests

```bash
cargo test -p junoclaw-x402-gateway
```

Coverage:
- Envelope round-trip (`x402.rs`)
- Tampered-envelope rejection
- Expired-envelope rejection
- Nonce replay rejection
- Distinct-nonce acceptance

Integration tests in `tests/` mock the chain RPC with `mockito`.

## Security

See [`docs/X402_RISK_ANALYSIS.md`](../../docs/X402_RISK_ANALYSIS.md) for the full deterministic threat model covering supply-chain / on-chain / off-chain / TPS axes.

Quick safety summary:

- **Gateway operator key never leaves disk.** Path in env, bytes loaded once at startup, no env-var-embedded keys.
- **Two-phase + binding hash** prevents tampering between phase 1 and phase 2.
- **Nonce replay store** rejects duplicate envelopes (in-memory in v1; needs shared backend for multi-replica).
- **`GATEWAY_MAX_TASK_UJUNO`** caps the per-task value the gateway will broker — defence in depth against a compromised gateway draining a DAO.
- **Rate limiting** at the HTTP layer via `tower-governor`.
- **Distroless cc-debian12 nonroot** runtime — no shell, no package manager.
- **Cosign keyless signing** of the container at publish time.

## License

Apache-2.0. Vulnerability disclosure: [`SECURITY.md`](../../SECURITY.md) at the repo root.
