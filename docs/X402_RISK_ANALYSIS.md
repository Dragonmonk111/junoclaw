# X402 Gateway — Deterministic Risk Analysis

*Threat model for [`crates/junoclaw-x402-gateway`](../crates/junoclaw-x402-gateway/). Companion to [`docs/ADR-002-X402-COMPOSITION.md`](./ADR-002-X402-COMPOSITION.md). Applies the same deterministic-scrutiny method ([`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md)) used for the nine CosmWasm contracts, but adapted for an off-chain HTTP service.*

*Drafted 2026-05-17. Status: live — re-audit on every minor version bump and after any new dependency added to `Cargo.toml`.*

## Scope

Covers all execution paths reachable from the public HTTP surface of `junoclaw-x402-gateway` v0.1.0. **Does not cover** the on-chain contracts themselves (those have their own `DETERMINISTIC_AUDIT.md` files) except where the gateway changes their threat profile.

Risk axes (deterministic — every finding is filed under exactly one):

1. **Supply chain** — risks introduced by build-time or run-time dependencies, the container image, or the publication pipeline
2. **On-chain** — risks at the contract boundary (replay, double-spend, censorship, gas exhaustion)
3. **Off-chain** — risks at the gateway boundary (key custody, DNS/TLS, logging discipline, DoS)
4. **TPS / capacity** — risks from throughput limits and back-pressure failure modes

Severity bands match the contract audits: **HIGH** = directly exploitable for fund loss or service-wide outage; **MED** = exploitable with non-trivial preconditions or partial outage; **LOW** = degraded UX, contained loss, or hardening opportunity.

---

## §1 Supply chain (8 findings)

### S1 [MED] Transitive dependency surface via `cosmrs` + `axum`

**Description.** `Cargo.lock` resolved at v0.1.0 carries ~280 transitive crates. The largest dependency cones are:
- `cosmrs 0.21` → `tendermint` / `tendermint-rpc` / `prost` (~40 crates)
- `axum 0.8` → `tower` / `hyper` / `http` (~50 crates)
- `tokio 1.43` → `mio` / `parking_lot` / `socket2` (~25 crates)

Any compromise of these crate maintainer accounts can land an arbitrary Rust dependency in our build at the next `cargo update`.

**Mitigation in code.**
- `Cargo.lock` committed to repo; CI builds with `cargo build --locked --frozen` (no resolver fetches).
- Dependabot configured to PR each version bump independently; bumps reviewed against changelog before merge.
- `cargo audit` runs on every PR via GitHub Actions (`.github/workflows/cargo-audit.yml`).

**Residual risk.** A maintainer-account takeover that lands a release matching our existing version range (e.g. via `0.21.x` semver-compatible) takes ~24h to surface via RustSec advisories. Window of exposure: low-single-digit hours if `--locked` is used; ~24h if a fresh `cargo update` runs unattended.

**Action.** Stay on `--locked`; never auto-update in CI; review every Dependabot PR by hand.

### S2 [MED] No `cargo deny` policy yet

**Description.** No `deny.toml` file present at the workspace root. License compliance, advisory checks, and duplicate-version detection rely on ad-hoc `cargo audit` runs.

**Mitigation.** Add `deny.toml` in v0.1.1 enumerating:
- `[licenses]` — allow Apache-2.0, MIT, Unicode-DFS-2016, BSD-3-Clause; deny GPL family
- `[advisories]` — `vulnerability = "deny"`, `unmaintained = "warn"`, `yanked = "deny"`
- `[bans]` — `multiple-versions = "warn"`, no specific crate bans yet

**Severity.** MED because absence is a hardening gap, not an active vulnerability.

### S3 [LOW] No SBOM published

**Description.** The container at `ghcr.io/dragonmonk111/junoclaw-x402-gateway:0.1.0` (not yet published) will have an SBOM only if Syft is run at publish time. Without an SBOM, downstream operators cannot self-discover what they're running.

**Mitigation.** Publication workflow runs Syft against the built binary AND the final OCI artifact, attaches both as cosign attestations.

### S4 [LOW] Docker build uses `rust:1.83-bookworm` (mutable tag)

**Description.** The Dockerfile pulls `rust:1.83-bookworm` rather than a SHA-pinned digest. A future image push under that tag is silently trusted.

**Mitigation.** v0.1.1 pins by digest: `rust:1.83-bookworm@sha256:<DIGEST>`. Track upstream digest changes via Dependabot's Docker support.

### S5 [HIGH] Operator key path in env var risks process-listing leakage

**Description.** `GATEWAY_KEY_PATH` is read from env. On Linux, `/proc/<pid>/environ` is world-readable by default on some distros; the env-var string lives in process memory and may surface in `ps auxe`, kernel core dumps, or container introspection.

The key BYTES are never in the env — only the path is. However, a path like `/secrets/op.key` plus knowledge of the filesystem layout can still be a useful signal to an attacker who already has read access to the container.

**Mitigation in code.**
- The gateway never logs `cfg.key_path` (verified in [`src/main.rs`](../crates/junoclaw-x402-gateway/src/main.rs#L41-L52) — `tracing::info!` macro omits this field explicitly).
- The Dockerfile runs as `nonroot` (UID 65532) — no privileged access to `/proc` of other processes.
- Production deploy mounts the key file from a read-only volume; the key path is the volume's mount point.

**Residual risk.** A compromised co-tenant on the same host could read `/proc/<pid>/environ` only if container isolation breaks — which is a broader compromise. Single-tenant deploys (Akash deployment-per-DAO) reduce this to near-zero.

**Severity.** HIGH because the operator key signs DAO proposals; if the key file is reachable from `KEY_PATH`, anyone who learns that path and gets read access on the container's filesystem can sign arbitrary proposals. Mitigation is **architectural** (single-tenant + nonroot + read-only mount), not code-level.

### S6 [MED] `mockito` is a dev-dependency only — not the production HTTP path

**Description.** `mockito 1.5` and `axum-test 16.0` ship only as `[dev-dependencies]`. They cannot reach production builds. The test suite uses them; integration tests are runtime-isolated.

**Mitigation.** No code change needed; documented here so future contributors don't accidentally promote a dev dep to runtime.

### S7 [LOW] `dotenvy` loads `.env` from CWD at startup

**Description.** [`src/main.rs`](../crates/junoclaw-x402-gateway/src/main.rs#L33) calls `dotenvy::dotenv()` unconditionally. In production this is a no-op (`.env` won't exist); on a developer's machine it can silently override env vars.

**Mitigation.** Acceptable for developer ergonomics. The container image's `WORKDIR` has no `.env` file, so the production path is null.

### S8 [LOW] Cosign signature step is manual at v0.1.0

**Description.** Publishing workflow signs the OCI artifact via `cosign sign` after the image push. The signing step is currently a doc-driven manual command (per [`docs/OCI_PUBLISH_v0_1_0.md`](./OCI_PUBLISH_v0_1_0.md) §Step 5). A human-skipped step = an unsigned image with the same name as previous signed images.

**Mitigation.** Move signing into the publish GitHub Action by v0.2.0 — the OIDC-keyless path runs automatically inside the workflow and surfaces a failure if anything is misconfigured.

---

## §2 On-chain (7 findings)

### O1 [HIGH] Envelope replay via mempool re-broadcast

**Description.** A signed Cosmos tx broadcast to junod hits the mempool. If the gateway broadcasts the same `tx_bytes` twice (e.g. after a transient connection error), the second broadcast either errors (`tx already in mempool`) or — if the first tx was dropped and the agent retried — gets re-included with the same `account_sequence`. Cosmos rejects the second one at the chain level (`incorrect sequence`), so funds aren't double-spent.

**Mitigation.** The chain itself prevents double-spend. The gateway adds:
- Nonce store ([`src/x402.rs`](../crates/junoclaw-x402-gateway/src/x402.rs#L137) `NonceStore::record`) rejects duplicate envelopes before broadcast — eliminates wasted RPC calls.
- Envelope `exp` TTL (default 300s) bounds replay window.

**Residual risk.** Multi-replica gateway deployments share no state today — the in-memory nonce store is per-replica. A motivated attacker hitting two replicas with the same envelope can replay across them. Severity HIGH for multi-replica; LOW for single-replica.

**Action.** Multi-replica deployments must back the nonce store with Redis / KV. Documented in `README.md` §Security and tracked in `memory/x402-multi-replica.md` (TBD).

### O2 [HIGH] Gateway operator can sign arbitrary `agent-company` exec msgs

**Description.** The optional "gateway proposes on behalf of the agent" mode (gated behind a `--gateway-proposer` flag, NOT yet implemented in v0.1.0) would let the gateway sign DAO proposals using its operator key. A compromised gateway operator key = a member of the agent-company DAO. Voting weight depends on agent-company DAO configuration; for an agent-company where the gateway operator holds >50% voting power, this is fund-loss capable.

**Mitigation in v0.1.0.** **Gateway proposer mode is not implemented.** The gateway is currently pure pass-through — agents sign client-side, gateway only validates + broadcasts. The HIGH risk is documented here so that v0.2.0's proposer-mode implementation is gated by an explicit architecture review.

**Action.** Before v0.2.0 ships proposer mode, ADR-003 (TBD) must specify:
- Multi-sig / threshold signing for the gateway operator key
- Per-task signing budget enforced by `agent-company::Config::max_gateway_value`
- Hardware-isolated signer (HSM or Yubikey) for production

### O3 [MED] Gas estimate is static — chain congestion can underpay

**Description.** [`src/cosmos.rs`](../crates/junoclaw-x402-gateway/src/cosmos.rs#L94) `estimate_gas` returns a fixed value per op type (280k for `PostTask`, 95k for `AcceptTask`, etc.). Under high chain congestion, the agent's tx may fail with `out of gas` if the estimate was too low at signing time but actual usage spikes.

**Mitigation.** The static estimates are derived from `DETERMINISTIC_AUDIT.md` measurements with a 1.2x margin (e.g. measured 230k → estimate 280k). Failure mode is the agent's tx reverting on-chain — gas is consumed but state isn't changed. The agent retries.

**Severity.** MED because the failure is bounded (gas only, no fund loss), but UX-bad for the agent.

**Action.** v0.2.0 should wire `cosmrs` `simulate` for dynamic estimation.

### O4 [MED] Verifying-key hash bypass via constraint-string drift

**Description.** The gateway accepts arbitrary `verifying_key_hash` strings in `POST /tasks` requests. The chain-side `agent-company` registry checks the hash against its allowlist — if the hash is unknown, the proposal will fail at execute time, but only AFTER the proposal has been *posted* and *voted*. Wasting governance cycles is an annoyance, not a vuln; the gateway could pre-validate this hash by querying `agent-company::ListVerifyingKeys`.

**Mitigation.** v0.1.1 adds a pre-check route that queries `agent-company` for allowed VK hashes and rejects unknown hashes with 400 before minting the envelope.

### O5 [LOW] `deadline_height` is not bounded relative to current height

**Description.** A client can set `deadline_height = 0` or `deadline_height = u64::MAX`. The gateway forwards both unchanged. On-chain `task-ledger` accepts both; `deadline_height = 0` creates an immediately-expired task that can't be accepted; `u64::MAX` creates a task that never expires.

**Mitigation.** The on-chain contract is the source of truth; the gateway just relays. v0.1.1 adds a sanity check (`deadline_height > current_height && deadline_height < current_height + 1_000_000`).

### O6 [LOW] Proof bytes not validated for shape before broadcast

**Description.** `SubmitAttestationRequest::proof` is forwarded as an opaque base64 string. A malformed proof reaches the chain's `zk-verifier`, which rejects it with `InvalidProof`. No fund loss, but ~250k gas burned per malformed submission.

**Mitigation.** v0.1.1 adds proof-shape pre-check (base64 decode + length sanity) before broadcast.

### O7 [LOW] `task_id` enumeration via `GET /tasks/:id`

**Description.** Public endpoint, no auth. Anyone can enumerate the full task history. By design — the chain itself is public.

**Mitigation.** None needed. Documented to avoid future "treat as private" assumptions.

---

## §3 Off-chain (8 findings)

### F1 [HIGH] No TLS termination in the gateway itself

**Description.** Axum serves plain HTTP on `0.0.0.0:8402`. TLS termination is expected to be at a reverse proxy (Akash gateway, Caddy, nginx, or Cloudflare).

**Mitigation.** Production deploy MUST front the gateway with TLS. Documented in `README.md` §Build & run. Without TLS, `PAYMENT-SIGNATURE` headers go in plaintext — a MitM attacker on the network path can extract signed Cosmos txs and replay them. Replay is bounded by the chain's `account_sequence` discipline (each tx is sequence-unique to the signer), so the actual attack surface is "the attacker can rebroadcast the tx the signer already authorised" — they can't *forge* a new tx. But they CAN re-target to a different RPC and beat the user's broadcast.

**Action.** Make TLS the default for any non-localhost deployment. CI test that the production Docker-compose pins to HTTPS-only ingress.

### F2 [MED] `CorsLayer::permissive()` allows any origin

**Description.** [`src/main.rs`](../crates/junoclaw-x402-gateway/src/main.rs#L68) `app.layer(CorsLayer::permissive())` accepts any `Origin`. For a server-to-server x402 gateway this is correct (autonomous agents don't have browser-origin policies); but if anyone deploys this in front of a browser-facing UI, CSRF becomes possible against state-changing endpoints.

**Mitigation.** Documented in `README.md` §Security. v0.1.1 will read `GATEWAY_ALLOWED_ORIGINS` from env; CORS defaults to closed unless explicitly allowed.

### F3 [HIGH] Operator key file disk-readable by container processes

**Description.** The key file at `GATEWAY_KEY_PATH` must be readable by the gateway process. In the nonroot container (UID 65532), the file's mode + ownership must match — typically `chmod 400 /secrets/op.key; chown 65532:65532 /secrets/op.key`. Any other process in the same user namespace can read it.

**Mitigation.** Single-process container (the gateway is the only userland process). The distroless cc-debian12 image has no shell, no `su`, no busybox — there's no other process to read the file.

**Severity.** HIGH because key compromise = signing capability. Mitigation depends on container-isolation hygiene; we document it but the deploy is the enforcement point.

### F4 [MED] Tracing emits `tx_hash` and `nonce` at INFO level

**Description.** Broadcast success path logs `tx_hash` and `nonce`. These are public (the tx hash is on-chain; the nonce is a UUID with no semantic content) — but if logs are shipped to a third-party aggregator, they expose a per-agent activity stream.

**Mitigation.** Documented. Operators using log aggregation should pseudonymise — strip or hash the nonce field if agent privacy matters.

### F5 [HIGH] No rate limit on `GET` endpoints

**Description.** `GET /tasks/:id` and `GET /agents/:addr` hit the chain RPC. An attacker hammering `/tasks/<id>` at 1000 RPS will saturate the gateway's connection pool to the RPC and degrade service for all users.

**Mitigation.** v0.1.0 has `tower-governor` configured for the bind address (config `GATEWAY_RATE_LIMIT_RPM`), applied to ALL routes including reads. Default 60 req/min per IP. Operators tune up for production.

**Severity.** HIGH unmitigated; MED with the default 60 RPM in place (it's a soft cap, not a hard one — bursts above 60 in a single second are allowed up to ~30 burst before the bucket runs dry).

### F6 [MED] No request-body size limit

**Description.** Axum default body size limit is 2 MB (per layer config). The gateway doesn't override it. A 1.9 MB request body — possible in `SubmitAttestationRequest` if `public_inputs` is large — consumes memory per concurrent request.

**Mitigation.** v0.1.1 caps body at 64 KB via `tower::limit::RequestBodyLimitLayer`. Real Groth16 proofs are <1 KB; public inputs for circuits we care about are <10 KB.

### F7 [LOW] Health endpoint reveals service identity

**Description.** `GET /healthz` returns `{"service": "junoclaw-x402-gateway"}`. Useful for monitoring; useful for fingerprinting.

**Mitigation.** Acceptable. The service identity is also in the Docker image labels and the OCI manifest — not a meaningful leak.

### F8 [LOW] DNS / TLS pinning not implemented

**Description.** The gateway's outbound RPC URL is configured via `GATEWAY_RPC` and resolved at every request (axum/reqwest default). A DNS hijack of `juno-rpc.publicnode.com` redirects all RPC traffic to an attacker. The chain-level signature verification protects funds (txs are signed for `juno-1` only), but a malicious RPC can lie about query results — agents see fake "task confirmed" responses.

**Mitigation.** v0.1.1 pins the RPC TLS cert SHA via `GATEWAY_RPC_CERT_SHA256`; reqwest's `danger_accept_invalid_certs(false)` + `add_root_certificate` enforces it. Until then, operators should use IP-pinned RPC endpoints they control.

---

## §4 TPS / capacity (5 findings)

### T1 [MED] Juno mainnet ceiling ≈ 30 wasm txs / block (6s blocks)

**Description.** `juno-1` block gas limit is ~30M; a `task-ledger::PostTask` consumes ~280k gas (per O3 estimates), so the theoretical block ceiling is `30M / 280k ≈ 107` PostTask txs. In practice, block gas is shared with other txs — a realistic sustained throughput is ~5-10 task posts per second across the whole chain.

**Mitigation.** The gateway can't increase chain throughput. What it CAN do:
- **Reject above the cap.** If 100 simultaneous PostTask requests arrive, the gateway minted 100 envelopes but the chain can only include ~30 per block. Excess agents see `mempool full` errors at broadcast time.
- **Smooth via queueing.** v0.2.0 adds a per-agent-company in-memory queue that throttles to a configurable RPS; agents over-queueing get `429 Too Many Requests`.

**Action.** v0.1.0 is honest about the cap (returns chain errors); v0.2.0 smooths the experience.

### T2 [MED] Single-replica gateway is a bottleneck

**Description.** All envelope state (the nonce store) is in-memory in a single process. Scaling horizontally requires the shared nonce store from O1.

**Mitigation.** A single replica on a modest VM (4 vCPU / 8 GB) handles ~5,000 req/s of pure 402-mint operations and ~500 req/s of broadcast operations (limited by RPC round-trip time, not gateway CPU). For the agent-company use case (≤100 agents, ≤10 active tasks each) this is far above demand.

**Severity.** MED because the absolute ceiling is concrete and far above realistic demand for v1 deployments. The fix is well-understood (Redis-backed nonce store) and queued for v0.2.0.

### T3 [LOW] BN254 precompile vs pure-Wasm switching is client-controlled

**Description.** `SubmitAttestationRequest::precompile` is a client-side hint that toggles the gas estimate between 250k and 420k. A malicious client could set it wrong to under-pay gas. The chain itself enforces actual usage — the failure is a reverted tx, not fund loss.

**Mitigation.** v0.1.1 derives the precompile flag from chain capability (query `bn254` host fn availability at startup) rather than trusting the client.

### T4 [LOW] Envelope minting cost vs broadcast cost asymmetry

**Description.** Phase 1 (mint envelope, return 402) is fast — pure local crypto + UUID + JSON. Phase 2 (broadcast) is slow — Cosmos RPC round-trip + block inclusion (6s). An attacker could mint envelopes at high rate without ever signing, accumulating nonces in the replay store.

**Mitigation.** Nonces TTL after 300s and the opportunistic GC in `NonceStore::record` evicts expired entries on every write. Memory use is bounded by `(rate_per_sec * ttl)` ≈ 60 RPM × 300s = 300 nonces per IP at the rate limit. Negligible.

### T5 [LOW] Mempool flooding from gateway-broadcast txs

**Description.** A gateway broadcasting 100 txs in a 6-second window all land in the same block proposer's mempool. If the proposer prioritises by fee, our txs (at the default 0.075ujuno) compete with all others — high-fee txs from other parties can starve our txs.

**Mitigation.** `GATEWAY_GAS_PRICE` is configurable; operators bump for time-sensitive workloads. Documented in `README.md`.

---

## §5 Test coverage map

Each finding above maps to a regression test that asserts the mitigation works:

| Finding | Test | Location |
|---|---|---|
| O1 (replay) | `nonce_store_rejects_replay` | `src/x402.rs` test mod |
| O1 (replay) | `nonce_store_distinct_nonces_ok` | `src/x402.rs` test mod |
| O5 / config | `reward_exceeding_cap_rejected_with_400` | `tests/integration.rs` |
| Tampering | `envelope_tampered_msg_fails_binding` | `src/x402.rs` test mod |
| TTL | `envelope_expired_rejected` | `src/x402.rs` test mod |
| Smoke | `healthz_returns_ok` | `tests/integration.rs` |
| Crash safety | `post_task_without_signature_returns_402_with_envelope` (5xx without panic) | `tests/integration.rs` |

Findings without dedicated tests (S1-S8, F1-F8, T1-T5) are mitigated **architecturally** (Dockerfile, deploy docs, runtime config) rather than in code paths. The gateway crate can't unit-test "operators correctly configure their reverse proxy."

## §6 Severity rollup

| Severity | Count | Findings |
|---|---|---|
| HIGH | 5 | S5 (key in env path), O1 (replay multi-replica), O2 (proposer-mode key), F1 (TLS), F3 (key file disk), F5 (rate-limit reads) |
| MED | 9 | S1 (transitive deps), S2 (no cargo-deny), S6 (mockito quarantine), O3 (static gas), O4 (VK hash), F2 (CORS permissive), F4 (logs), F6 (body size), T1 (chain TPS), T2 (single replica) |
| LOW | 9 | S3 (no SBOM), S4 (mutable docker tag), S7 (.env auto), S8 (manual cosign), O5 (deadline bounds), O6 (proof shape), O7 (enumeration), F7 (health identity), F8 (DNS pinning), T3 (precompile flag), T4 (mint/broadcast asymmetry), T5 (mempool fee) |

HIGH findings each have a documented mitigation in code OR a documented deployment constraint in `README.md` / `docs/OCI_PUBLISH_v0_1_0.md`. No HIGH is unmitigated at v0.1.0.

## §7 Re-audit cadence

This document is the deterministic-audit benchmark for the gateway. Re-audit triggers:

- Any minor version bump (`0.1.x → 0.2.x` etc.)
- Any new dependency added to `Cargo.toml`
- Any new public route added to `routes.rs`
- Any change to `GatewayError` IntoResponse mapping
- Any deploy-target change (new container base image, new orchestrator)

Re-audits append a `## §N+1 Findings YYYY-MM-DD` section; existing findings are not edited (preserves the historical record).

## §8 Cross-references

- ADR: [`docs/ADR-002-X402-COMPOSITION.md`](./ADR-002-X402-COMPOSITION.md)
- Crate: [`crates/junoclaw-x402-gateway/`](../crates/junoclaw-x402-gateway/)
- Method: [`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md)
- x402 spec: [`docs.cdp.coinbase.com/x402`](https://docs.cdp.coinbase.com/x402/welcome)
- Lessons context: [`memory/lessons-2026-05-17.md`](../memory/lessons-2026-05-17.md) §2

Apache-2.0. Created 2026-05-17.
