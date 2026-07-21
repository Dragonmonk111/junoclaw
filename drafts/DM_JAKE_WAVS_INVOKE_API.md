# DM to Jake — WAVS Off-Chain Invoke API

*Draft message for Jake Hartnell. Send via Telegram or whichever channel you normally reach him on. Adjust tone as needed.*

---

Hey Jake,

Following up on our earlier conversation about off-chain / on-demand component invocation in WAVS. We've hit a concrete wall where the event-driven-only model is costing us real on-chain overhead, and I'd like to propose a spec for an off-chain `invoke` API and get your read on whether it fits the WAVS roadmap.

**The use case:** our sealed signer (TEE-sealed secp256k1 key inside a WAVS component) now signs full Cosmos SDK `SIGN_MODE_DIRECT` transactions — `MsgExecuteContract` only, constructed from structured fields inside the enclave, never trusts caller-supplied protobuf. The goal is to fully replace the plaintext mnemonic our reply-bot uses to post to Moultbook. M1 proved the crypto; M2 proved the tx signing (byte-identical `SignDoc` cross-checked against CosmJS). All compiling clean for `wasm32-wasip2`.

**The problem:** there's no on-chain event to trigger on when you need to sign a tx *before* that tx is submitted. So we built an on-chain round-trip through our `agent-company` contract:

1. Relayer submits `RequestSignedTx` → contract validates + emits `sign_request` event
2. WAVS picks up the event → component signs inside TEE → bridge submits `StoreSignedTx` back on-chain
3. Relayer polls `GetSignRequest` until status=Signed → broadcasts `tx_bytes` → calls `AckBroadcastTx` to clean up

It works and it's safe, but the overhead is painful:

| Cost | Amount |
|---|---|
| Extra on-chain messages per signed post | 3 (`RequestSignedTx` + `StoreSignedTx` + `AckBroadcastTx`) |
| Extra gas per signed post | ~330,000–650,000 gas |
| Extra latency per signed post | ~30–60 seconds (3 block delays + indexer lag + polling) |
| Throughput ceiling | 1 pending request at a time (to avoid sequence collisions) = ~1 post/min |
| Failure mode | If WAVS or bridge goes down, request stays Pending forever; blocks all future posts |

All of this exists *solely* because there's no way to call a WAVS component directly without an on-chain trigger event.

**What we're proposing:** a `wavs invoke` API — either an HTTP endpoint on the WAVS daemon or a CLI subcommand — that accepts a component ID + structured input and returns the component's output synchronously. No on-chain trigger required.

The flow would collapse to:
```
relayer → POST /invoke/{component} (signed HTTP) → get tx_bytes + attestation → broadcast → SubmitAttestation
```

Authentication via signed HTTP (secp256k1 over canonical request string, reusing Cosmos key infra). Allowlist config in `service.json` or a new `invoke_policy.json` specifying which caller addresses may invoke which components. Rate limiting + logging on the daemon side.

Security properties all preserved — component still runs inside TEE, key still never leaves enclave, attestation still produced and recorded on-chain via `SubmitAttestation`. The only shift is authorization moving from the on-chain contract to the daemon allowlist.

**Can share the full spec:** covers the HTTP API, auth mechanism, allowlist config, security analysis, simplified flow, 4-phase implementation plan, architecture diagram, implementation priority table, and 6 questions **with our recommended answers** (informed by the existing codebase — e.g. `TriggerData::Raw` already exists in `trigger.rs` as a testing path, so the invoke API is essentially promoting it to production; the bridge already has a production HTTP auth server in `rpc-server.ts` with bearer token, rate limiting, and audit logging we can extend).

**Questions for you (with our recommended answers in the spec):**

1. Is off-chain invoke on the WAVS roadmap already, or would this be a new contribution from our side? **(We think: new contribution, but `TriggerData::Raw` already proves the path works.)**
2. Auth preference — signed HTTP (secp256k1) vs. mTLS vs. token-based? **(We recommend: bearer token for v1 reusing existing `rpc-server.ts`, signed HTTP for v2.)**
3. Attestation in synchronous mode — does the TEE produce a quote per invocation, or can the daemon batch? **(We recommend: per-invocation SHA-256 attestation hash, full SGX quote only at daemon startup.)**
4. Should invoke be available for any component, or restricted to trigger types that declare `invoke_enabled: true` in `service.json`? **(We recommend: scoped per-workflow, default disabled.)**
5. Is the WAVS daemon already running an HTTP server we can extend, or does this need a new listener? **(We recommend: extend existing `rpc-server.ts` for v1, separate sidecar for v2.)**
6. Does adding an HTTP input path change the component's measurement? **(We believe: no — trigger source is runtime, not component. Verifiable by hashing .wasm before/after.)**

This would benefit any WAVS workflow that produces signed data before it exists on-chain — not just our sealed signer. Signed data feeds, on-demand ZK proof generation, multi-step agent workflows with intermediate signed results. It's a WAVS runtime feature, not a JunoClaw feature.

Happy to implement it if there's a contribution path. Let me know what you think.
