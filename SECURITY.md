# Security Policy

JunoClaw is infrastructure for autonomous agents performing real-world work. We take security findings seriously and respond publicly.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security findings.**

Coordinate disclosure via one of the following:

- **Private GitHub Security Advisory** — preferred. Open at <https://github.com/Dragonmonk111/junoclaw/security/advisories/new>. GitHub will mint a CVE on publication.
- **Direct contact** — DM `@vairagyanodes` on Telegram, or email `security@vairagyanodes.dev` (PGP key fingerprint will be published before v1.0).

We aim to acknowledge new reports within **48 hours** and to ship initial mitigations within **7 days** for findings rated *critical* or *high*.

## Supported versions

| Version | Status | Security fixes |
|---------|--------|---------------|
| `main` (development) | Active | Yes |
| Tag `v0.x.y-security-1` | Active (post-Ffern hardening release) | Yes |
| Untagged earlier commits | Unsupported | No — please pull `main` and rebuild |

The project is pre-v1.0. The supported version is whatever is on `main` at HEAD; tagged security releases mark coherent points in `main`'s history (no separate release branches). Per-component versions in `mcp/package.json`, `wavs/bridge/package.json`, and the `Cargo.toml` files move independently.

## Threat model

JunoClaw splits its security surface into four layers, each with its own witness and its own response cadence. The full taxonomy is in `docs/HACKMD_BN254_PROPOSAL.md` and the post-#373 Medium article. In short:

- **Lock 1 — TEE attestation (hardware witness)**, *who and where*. Already on-chain via Proposal #373.
- **Lock 2 — ZK verification (mathematical witness)**, *what was computed*. The subject of the in-flight BN254 precompile signaling proposal.
- **Lock 3 — Tier-1.5 constraint vocabulary (declarative witness)**, *under what bounds the agent was even allowed to attempt*. Already live on `uni-7` (`agent-company` v7, code_id 75).
- **Lock 4 — Verifiable controllability (operational witness)**, *and is the operator still observably in control of the fleet*. In-flight as the post-Ffern-audit hardening pass (April–May 2026).

Findings against any layer are in scope. Findings against *off-chain helper code* (`plugins/`, `mcp/`, `wavs/bridge/`) are real and we want to hear about them — these are the surfaces where the Ffern Institute audit (April 2026) found four critical and one high-severity issue.

## Verifiable-controllability primitives

Lock 4 is split into two complementary kinds of primitive: **walls** that prevent the next failure mode by hardening inputs and gating capabilities at compile or load time, and **levers** that make a flow already in motion stoppable without taking the system down. Walls without a lever leave the operator with no graceful response to surprise; a lever without walls leaves the operator with a halt button on a system that gets compromised in the first place. Lock 4 needs both.

### Walls (shipped in `v0.x.y-security-1`)

- **Compile-time gates.** Cargo features (`unsafe-shell` today, `unsafe-egress` and `unsafe-fs-write` planned) keep dangerous code paths out of release binaries unless the operator explicitly opts in. Default builds have no shell-execution surface and no SSRF-able URL fetcher.
- **Input-validation guards.** SSRF guard (`wavs/bridge/src/utils/ssrf-guard.ts`, Ffern H-3) and WASM path guard (`mcp/src/utils/path-guard.ts`, Ffern C-4) reject hostile inputs before any sensitive primitive runs.
- **Wallet handle registry (Ffern C-3, two-phase, both shipped).** Every MCP write tool takes a `wallet_id` instead of a raw `mnemonic` parameter. The model never sees the mnemonic; the MCP transport never carries it; conversation logs cannot leak it. Two backends ship and are dispatched per-wallet:
  - **Phase 1 — passphrase backend.** scrypt N=2^17 r=8 p=1 derives a master DEK from `JUNOCLAW_WALLET_PASSPHRASE`; each wallet is encrypted with AES-256-GCM under that DEK with a fresh per-encrypt IV. Portable across machines.
  - **Phase 2 — keychain backend.** OS credential manager (Windows DPAPI / macOS Keychain Services / Linux libsecret) holds a fresh random 32-byte DEK per wallet, indexed by `(service="junoclaw-cosmos-mcp", account=walletId)`. The wallet file is encrypted with AES-256-GCM under that keychain-stored DEK. No long-lived passphrase to manage; the DEK is bound to the OS user session. Native binding via the optional `@napi-rs/keyring` dependency; an in-memory test driver gives full mock-keychain coverage.
  
  See [`mcp/README.md`](mcp/README.md#wallet-registry-ffern-c-3--mnemonic--wallet_id) for the operator-facing CLI and migration guide.
- **Startup-only kill-switch.** `sandbox_mode` on `plugin-shell` halts execution at runtime even with the `unsafe-shell` feature compiled in. Today this is settable only at process start; runtime hot-reload via admin RPC lands in `v0.x.y-security-3`.

### Levers

- **`signing_paused` runtime kill-switch — shipped in `v0.x.y-security-2`, hot-flip added in `v0.x.y-security-3`.** Boolean gate on `WalletStore.signFor()`. When armed, the gate refuses with a specific `SigningPausedError` while query tools, `wallet list`, and `verifyAddress` keep working. Armed via the `JUNOCLAW_SIGNING_PAUSED` env var at process start (canonical value `1`); fail-closed on any non-empty, non-`0` value. Mean-time-to-halt drops from process-supervisor restart (5–30 s) to a single localhost `curl` command after the admin RPC is wired in `v0.x.y-security-3`.
- **`egress_paused` runtime kill-switch — shipped in `v0.x.y-security-3` (Phase 3a).** The same pattern applied to the SSRF-guarded fetcher in the WAVS bridge. When armed, every `safeFetch()` call throws `EgressPausedError` at the very top of the function — no DNS lookup, no fetch, no side effects. Armed via `JUNOCLAW_EGRESS_PAUSED` at startup (fail-closed parsing) or via `setEgressPaused(true|false, source)` at runtime. Hot-flip available via the WAVS-side admin RPC (Phase 3d). See `wavs/bridge/src/utils/ssrf-guard.ts`.
- **Published policy-state admin RPC — shipped in `v0.x.y-security-3` (Phase 3c).** Read-only `GET /policy` endpoint on the localhost admin listener exposing the live values of every kill-switch this process owns. Lets a downstream client (a delegator, a counterparty, a verifier) confirm operator intent before sending a task. This is the load-bearing part of *verifiable* in *verifiable controllability*. The MCP side reports `signing_paused`; the WAVS-bridge side reports `egress_paused`; downstream tools poll both endpoints to assemble the cross-process picture.
- **Admin-RPC hot-reload — shipped in `v0.x.y-security-3` (Phases 3b & 3d).** Localhost-only HTTP listener (binds to `127.0.0.1` only; constructor refuses `0.0.0.0` / `::1` / `::`), bearer-token auth (≥32-byte token, constant-time comparison via `crypto.timingSafeEqual`), Host-header check + Origin-header rejection (DNS-rebinding defense), in-memory rate limit (10 req/60 s, fires before auth), audit log to stderr that never records the token. Off-by-default; opt in by setting `JUNOCLAW_ADMIN_RPC=1` AND `JUNOCLAW_ADMIN_TOKEN=<≥32-byte-token>`. Zero new runtime dependencies (Node built-in `http` + `crypto`). MCP listener at `mcp/src/admin/rpc-server.ts` exposes `/health`, `/policy`, `/signing/{status,pause,unpause}`. WAVS-bridge listener at `wavs/bridge/src/admin/rpc-server.ts` exposes `/health`, `/policy`, `/egress/{status,pause,unpause}`. Both wired into their respective entry points (`mcp/src/index.ts`, `wavs/bridge/src/bridge.ts`) with SIGINT/SIGTERM graceful shutdown.

#### Operator runbook (post-`v0.x.y-security-3`)

Generate a 32-byte hex token once per deployment and put it in your secrets manager:

```bash
openssl rand -hex 32
```

Start the MCP / bridge with the admin RPC enabled:

```bash
export JUNOCLAW_ADMIN_RPC=1
export JUNOCLAW_ADMIN_TOKEN=<token>
export JUNOCLAW_ADMIN_RPC_PORT=51731   # optional; default 0 = OS-assigned
# the MCP / bridge prints the listener URL on stderr at startup
```

During an incident, halt signing:

```bash
curl -X POST -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"source":"incident-2026-04-26"}' \
     http://127.0.0.1:51731/signing/pause
```

Halt outbound HTTP from the bridge:

```bash
curl -X POST -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"source":"incident-2026-04-26"}' \
     http://127.0.0.1:<bridge-port>/egress/pause
```

Poll policy state from a separate verifier or dashboard:

```bash
curl -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     http://127.0.0.1:51731/policy
```

Resume after investigation:

```bash
curl -X POST -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"source":"incident-2026-04-26-resolved"}' \
     http://127.0.0.1:51731/signing/unpause
```

For deployments that prefer the env-var path — unset the env var and bounce the process via systemd / PM2 / NSSM / launchd. The kill-switches respect both control surfaces, and the OS supervisor leaves a clean forensic trail.

## Chain-layer adjacency — blast-radius primitives (Phase 3 roadmap)

All five Ffern findings landed *upstream of the mempool*. Cosmos SDK chain mechanics correctly do not, and should not, intervene at this layer — chains settle signed transactions; how the operator obtained the signing key, where the WASM bytes came from, what URLs the off-chain helper code fetched, and what shell commands the plugin runtime executed are all outside the consensus boundary by design. Pushing operator-side security into consensus would break consensus performance and decentralisation.

The chain layer **does** offer adjacent primitives that limit *blast radius* once a compromise has occurred:

- **`x/authz`.** A main key delegates specific message types to a hot key with explicit expiry. The main key can `MsgRevoke` at any time. A leaked authz-delegated hot key can only sign within the delegated message-type window before the chain enforces expiry.
- **`x/feegrant`.** Caps fee budget for a delegated signer. A leaked authz-delegated hot key with a feegrant cap can sign only until the budget runs out.
- **`x/group`.** Multi-sig over a high-value account. The agent's hot key submits a *proposal*; M-of-N humans must co-sign within a threshold window for the action to execute. Heaviest brake, most aligned with the four-witness ethos.
- **CosmWasm contract-level access control.** Per-contract policy gates: sender allowlists, timelocks, admin = governance contract patterns.

These are *complementary*, not substitutes, for the off-chain primitives shipped in `v0.x.y-security-1`. They bound damage *after* a compromise; the off-chain primitives prevent the compromise itself.

**Phase 3** integrates `x/authz` into the wallet registry: each `wallet_id` becomes an authz delegation from a separate cold key the MCP process never touches. A compromised MCP process can drain only the authz-delegated message types within the chain-enforced expiry, not the cold key. This is the natural Lock 4 primitive at the chain layer; it composes with the off-chain primitives shipped in `v0.x.y-security-1`, the `signing_paused` lever shipped in `v0.x.y-security-2`, and the admin-RPC + `egress_paused` levers shipped in `v0.x.y-security-3`. Scope and interface are sketched in the `v0.x.y-security-4+` roadmap entry below; full design lands as a separate proposal.

## Recommended deployment-time isolation

Even with the in-binary hardening, sensitive operator deployments should run inside an external sandbox:

- **Linux** — Docker, `bubblewrap --unshare-all`, Firejail, or a systemd unit with `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateNetwork=yes` (where applicable), `NoNewPrivileges=yes`, `CapabilityBoundingSet=`.
- **macOS** — `sandbox-exec` with a tight profile.
- **Windows** — AppContainer or Job Objects with explicit allow-rules.
- **Akash** — the existing SDL deploys to Kata Containers; verify each container's secret mounts and external-RPC reachability before promoting to production.

If you compile with `--features unsafe-shell` and run *without* an external sandbox, you are accepting the risk of arbitrary command execution by anyone who can get a tool call approved by your model. The feature name is honest about the cost.

## Acknowledgements

- **Ffern Institute** — independent operator-side audit, April 2026. Findings, response, and re-check process are documented in `docs/HACKMD_BN254_PROPOSAL.md` (§*Audit response*) and `docs/FFERN_THANK_YOU_PM.md`.

## Roadmap

- **`v0.x.y-security-1` (this release)** — closes the four critical and one high-severity Ffern findings: `unsafe-shell` Cargo gate (C-1/C-2), wallet handle registry with passphrase + keychain backends (C-3), `upload_wasm` path guard (C-4), `computeDataVerify` SSRF guard (H-3); plus the startup-only `sandbox_mode` kill-switch on `plugin-shell`. The five walls of Lock 4.
- **`v0.x.y-security-2` (next)** — `signing_paused` runtime kill-switch: env-var-armed boolean gate on `WalletStore.signFor()`, raising `SigningPausedError`. Shipped alone rather than bundled with the admin RPC because the admin RPC introduces a new network listener in a signing-sensitive process and deserves a dedicated review window. See the *Levers* section above.
- **`v0.x.y-security-3` (this release)** — the remaining runtime levers: `egress_paused` on the WAVS SSRF-guarded fetcher (Phase 3a), the localhost-only token-gated admin RPC on both processes (Phases 3b for MCP, 3d for WAVS bridge — zero third-party deps, constant-time token comparison, rate-limited, audit-logged, DNS-rebinding-resistant), and the read-only `/policy` roll-up endpoint (Phase 3c). Verifiable controllability is operationally complete: hot-flip on both kill-switches, single-curl mean-time-to-halt, downstream-pollable policy state. See the *Levers* section for the operator runbook.
- **`v0.x.y-security-4+` (chain-layer integration)** — `x/authz` integration as a Phase 3 chain-layer Lock 4 primitive: wallet handles bind to authz delegations from a separate cold key the MCP process never touches. Composes with the off-chain primitives in `-security-1`, `-security-2`, and `-security-3`. Full design as a separate proposal.
- **Pre-mainnet** — third-party audit of the BN254 precompile crate per `docs/HACKMD_BN254_PROPOSAL.md` revised cost envelope ($30–45k, 3–5 weeks). Re-audit by Ffern of the operator-side fixes is the explicit gate on the upstream CosmWasm PR step.
- **Post-mainnet** — annual external audit cadence funded via DAO treasury; rolling fuzzing and differential-testing in CI.

## License posture for AI-assisted contributions

JunoClaw is heavily AI-assisted. Every contribution is reviewed, edited, and committed by a named human contributor under whose direction the AI drafted the change. The Apache 2.0 grant applies to the human-authored portions. To the extent any portion is fully AI-generated and not subject to copyright, it is dedicated to the public domain (CC0). See `NOTICE` and `CONTRIBUTORS.md` for the full statement.

This posture aligns with the Linux Foundation's late-2024 guidance and the Apache Software Foundation's 2024 contribution policy.
