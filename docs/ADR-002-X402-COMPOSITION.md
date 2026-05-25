# ADR-002 — x402 Composition with JunoClaw

| Status   | Accepted (revised 2026-05-17 evening) |
|----------|----------|
| Date     | 2026-05-17 |
| Authors  | Dragonmonk111 |
| Supersedes | None |
| Superseded by | None |

## Revision history

- **2026-05-17 morning (Proposed, non-goal for v1):** Initial position was that x402 was a v2 concern; only the ADR shipped, no code.
- **2026-05-17 evening (Accepted):** Revised based on user direction. The HTTP gateway is now in v1 scope as a new workspace crate (`crates/junoclaw-x402-gateway`). The Juno-native facilitator remains out-of-scope. Risk analysis lives at [`docs/X402_RISK_ANALYSIS.md`](./X402_RISK_ANALYSIS.md).

## Scope locked for v1

**In scope:**
- `crates/junoclaw-x402-gateway` — Rust axum service that exposes JunoClaw operations via x402 envelopes
- Endpoints: `POST /tasks` (post via DAO proposal), `GET /tasks/:id`, `POST /tasks/:id/accept`, `POST /tasks/:id/submit`, `GET /agents/:addr`, `GET /healthz`, `GET /metrics`
- Cosmos-shaped x402 envelopes (not EVM-shaped — we're not pretending to be on Base)
- Nonce + expiry anti-replay
- Rate limiting via `tower-governor`
- Distroless multi-stage Docker image
- Cosign keyless signing for the gateway container
- SBOM generation (Syft, attached to releases)
- Test coverage on envelope round-trip + nonce rejection + expiry rejection

**Out of scope for v1 (deferred to v2):**
- Juno-native x402 facilitator (separate project; would need to interop with Coinbase CDP's verifier)
- EVM-style envelopes / Permit2 / EIP-3009 — those require Cosmos-side EVM-compat (cosmos-evm or similar) which Juno doesn't have today
- Bazaar discovery integration — wait until x402's discovery layer stabilises
- "Sign-in-with-x" attestation — phase 2 once `agent-registry` is on mainnet

## Why now (revised rationale)

Two new signals since the morning ADR:

1. **Jake's ❤️ on 2026-05-17 closed the DM thread.** We have a 2-4 week runway until v30 hits testnet. The gateway is independent of v30/v31 — it talks to whatever JunoClaw is deployed on `juno-1`. Building it now means it's ready when mainnet code IDs land.
2. **Owocki's deployment validates the pattern.** Gitcoin's bot uses x402 for bounty posting fees in production. The "agent pays automatically, treasury gets funded, builders get paid" loop is the same loop JunoClaw is built for, just at a different layer.

The deferred reasoning ("v1 scope is large") still applies, but adding ~1500 LoC of gateway code in a separate crate doesn't perturb the nine-contract critical path. It's strictly additive.

## Context

On 2026-05-17, Kevin Owocki (Gitcoin founder) posted on X about Coinbase's x402 protocol — a revival of HTTP 402 ("Payment Required") as a payment layer for autonomous agents. Owocki's bot uses x402 for bounty posting fees: agents pay automatically, treasury gets funded, builders get paid. Source: [`docs.cdp.coinbase.com/x402/welcome`](https://docs.cdp.coinbase.com/x402/welcome) and [Owocki's tweet](https://x.com/owockibot) (the linked blog 404s as of the read date).

x402's flow is:

1. Buyer (human or agent) requests a resource via HTTP
2. Server responds `402 Payment Required` with a `PAYMENT-REQUIRED` header containing payment instructions
3. Buyer constructs payment, signs, retries with `PAYMENT-SIGNATURE` header
4. Server verifies via a **facilitator** (Coinbase CDP runs the primary one); if valid, returns the resource

Currently:
- EVM (Base, Polygon, Arbitrum, World) + Solana — **no Cosmos facilitator**
- Free tier: 1,000 tx/month via Coinbase facilitator; then $0.001/tx
- All ERC-20 via Permit2; EIP-3009 (USDC, EURC) for the smoothest path
- TypeScript / Go / Python SDKs

JunoClaw's `task-ledger` + `escrow` contracts solve a structurally similar problem at a different layer:

| x402 (HTTP-layer) | JunoClaw (chain-layer) |
|---|---|
| Buyer hits `POST /resource` | Buyer queries `list_tasks` |
| Server returns `402` + `PAYMENT-REQUIRED` | Task posted with `reward` + `constraints` + `verifying_key_hash` |
| Buyer signs payment | Agent calls `accept_task` (escrow lock) |
| Facilitator verifies | `zk-verifier` verifies Groth16 proof against VK |
| Server returns resource | `escrow` settles to agent on `submit_attestation` success |

## Decision

**For JunoClaw v1, x402 is a non-goal.** We do not ship an HTTP gateway, x402 facilitator, or 402-aware proxy.

**For v2 (post-mainnet stabilisation), two angles are explicitly retained as candidate work:**

1. **JunoClaw fronted by x402 (HTTP gateway)** — A thin HTTP service that exposes JunoClaw operations via x402 envelopes. Non-Cosmos-native agents (most LLM agents are EVM-first today) hit `POST /tasks` and receive a `402` with the JUNO escrow instructions. The gateway handles the on-chain `PostTask` flow on behalf of the agent. This makes JunoClaw addressable to the broader autonomous-agent ecosystem without requiring those agents to learn Cosmos signing.

2. **Juno-native x402 facilitator** — A Coinbase-style facilitator service running on Juno that settles in JUNO or IBC-bridged USDC. There is no Cosmos facilitator today. This is a separate project from JunoClaw v1 but a natural sibling — JunoClaw's `agent-registry` already provides reputation primitives that a facilitator could use for "Sign-in-with-x" attestations (per x402's facilitator roadmap).

## Rationale

**Why not ship now:**
- v1 scope is already large (9 contracts + WAVS operator + BN254 patches + audit + skill PR). Adding an HTTP layer would dilute attention.
- Jake's explicit guidance is "smaller upgrade, then start forking" (DM 2026-05-17). Adding a new protocol surface mid-v30 violates the spirit of that.
- x402 is currently EVM-only; a Cosmos integration requires either waiting for upstream Coinbase support or building a Juno-native facilitator. Both have non-trivial timelines.
- The on-chain task-ledger is sufficient for the agent-company use case. Adding HTTP routing is a UX optimisation, not a correctness fix.

**Why not declare it permanently out-of-scope:**
- The two architectures compose cleanly. There's no conflict, just additional surface.
- The agent-economy thesis behind both protocols is identical — autonomous agents paying for services. Closing the door entirely would be ideologically inconsistent with JunoClaw's positioning.
- Owocki's deployment (Gitcoin bounty fees) is a real-world validation that this pattern works for similar funder/builder/agent triangles. The lesson should be captured even if not yet acted on.

## Consequences

**Positive:**
- Keeps v1 scope bounded; ships in line with Jake's "smaller upgrade" directive.
- Documents the integration angle so any future contributor can pick it up without re-deriving the analysis.
- Frees the WAVS operator pattern from being tied to one signing protocol — `dao-proposal-wavs` consumes attestations regardless of whether the original task was posted via Cosmos signer or HTTP/x402.

**Negative:**
- Non-Cosmos-native agents (the bulk of the LLM-agent ecosystem in 2026) cannot interact with JunoClaw without learning `junod` or using a third-party signing service. This is friction we're choosing to accept for v1.
- If Coinbase or someone else ships a Cosmos x402 facilitator before we revisit this, we'll be a consumer rather than a contributor — late to the standard.

**Neutral:**
- The existing `task-ledger::PostTask` interface needs no changes to support either future direction. Adding x402 later is purely additive; no contract migration required.

## Implementation sketch (for v2 reference, NOT v1)

If we revisit this post-mainnet:

```
[Off-chain LLM agent]
    │ POST /tasks
    ▼
[junoclaw-x402-gateway (TypeScript SDK)]
    │ 402 + PAYMENT-REQUIRED:
    │   chain = juno-1
    │   contract = $JCLAW_AGENT_COMPANY
    │   reward = 100ujuno
    │   verifying_key_hash = ...
    ▼
[Agent constructs JUNO tx, signs via internal Cosmos signer or wallet API]
    │ Retry with PAYMENT-SIGNATURE header
    ▼
[Gateway verifies signature, broadcasts on-chain PostTask]
    │ Task posted; gateway returns 200 + task_id
    ▼
[Off-chain agent picks up task_id, executes work, submits attestation]
```

No on-chain code changes required. The gateway is a Node.js / Rust HTTP service that translates between x402 envelope schemas and Cosmos tx schemas. Cosign-signed container, deployed on Akash alongside the warg-registry / OCI publishing infrastructure.

## References

- [x402 documentation — Coinbase CDP](https://docs.cdp.coinbase.com/x402/welcome)
- [x402 GitHub (Coinbase)](https://github.com/coinbase/x402)
- [`memory/lessons-2026-05-17.md`](../memory/lessons-2026-05-17.md) §2 — discovery context
- [`docs/MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md`](./MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md) — JunoClaw thesis
- [Sherlock — x402 Explained](https://sherlock.xyz/post/x402-explained-the-http-402-payment-protocol) — independent explainer

---

*Apache-2.0. Created 2026-05-17.*
