# Release notes — `v0.x.y-security-3`

> **Purpose.** Paste-ready release-note body for the GitHub Release at
> <https://github.com/Dragonmonk111/junoclaw/releases/new?tag=v0.x.y-security-3>.
> Title field: `v0.x.y-security-3 — admin RPC + egress_paused; verifiable controllability complete`. Description: paste below.

---

The remaining runtime levers. Closes out the post-Ffern hardening track that began with `v0.x.y-security-1`. After this release, every kill-switch defined in JunoClaw is hot-flippable from a single localhost `curl`, and every kill-switch state is pollable read-only by downstream verifiers.

## What's in this release

Four primitives + an integration commit:

- **Phase 3a** — `egress_paused` runtime kill-switch on the WAVS bridge. Mirrors `signing_paused` from `v0.x.y-security-2` but applied to the SSRF-guarded fetcher. When armed, every `safeFetch()` call throws `EgressPausedError` at the very top of the function — no DNS lookup, no fetch, no side effects.
- **Phase 3b** — admin RPC primitive on the MCP side. Localhost-only HTTP listener with bearer-token auth, Host/Origin defenses, rate limit, audit log. Hot-flips `signing_paused`.
- **Phase 3c** — `GET /policy` read-only roll-up endpoint extending the MCP admin RPC. Lets downstream verifiers and dashboards poll the live kill-switch state without ever mutating it.
- **Phase 3d** — admin RPC primitive on the WAVS bridge side. Mirrors Phase 3b for hot-flipping `egress_paused`. The headline assertion: in the smoke, the same process that received `POST /egress/pause` then refuses `safeFetch()` with `EgressPausedError`, end-to-end.
- **Wiring + docs** — `mcp/src/index.ts` and `wavs/bridge/src/bridge.ts` start the admin RPC when both `JUNOCLAW_ADMIN_RPC=1` and `JUNOCLAW_ADMIN_TOKEN` are set, with SIGINT/SIGTERM graceful shutdown. `SECURITY.md`, `CHANGELOG.md`, and `mcp/README.md` updated with the operator runbook.

Mean-time-to-halt drops from process-supervisor restart (5–30 s) to a single localhost `curl` (~200 ms).

## Operator quick-start

Generate a 32-byte hex token once per deployment:

```bash
openssl rand -hex 32
```

Start the MCP and/or bridge with the admin RPC enabled:

```bash
export JUNOCLAW_ADMIN_RPC=1
export JUNOCLAW_ADMIN_TOKEN=<token>
export JUNOCLAW_ADMIN_RPC_PORT=51731   # optional; default 0 = OS-assigned
cosmos-mcp                              # the listener URL prints on stderr
```

During an incident, halt signing:

```bash
curl -X POST -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"source":"incident-2026-04-26"}' \
     http://127.0.0.1:51731/signing/pause
```

Halt outbound HTTP from the bridge (replace port with bridge's actual port):

```bash
curl -X POST -H "Authorization: Bearer $JUNOCLAW_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"source":"incident-2026-04-26"}' \
     http://127.0.0.1:<bridge-port>/egress/pause
```

Poll policy state from a separate verifier:

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

## Threat model and defenses

The admin RPC introduces a new network listener in two signing-sensitive processes. Defenses, in evaluation order:

1. **Host header check** — must equal `127.0.0.1:<port>` or `localhost:<port>` (DNS-rebinding defense).
2. **Origin rejection** — any non-empty `Origin:` header (browsers always set it) is rejected with 400.
3. **Rate limit** — default 10 req/60 s, fires *before* auth so token-spamming cannot bypass the limit. Returns 429 with `Retry-After`.
4. **Bearer token** — ≥32-byte token, constant-time comparison via `crypto.timingSafeEqual`. Tokens shorter than 32 bytes fail at constructor time.
5. **Body size cap** — 64 KiB.
6. **Schema check** — `body.source` must be a non-empty string ≤256 chars.
7. **Route dispatch** — unknown path → 404; wrong method on known path → 405 with `Allow:`.

The token never appears in any audit-log field (verified by an explicit test). Off-by-default: the admin RPC only starts when **both** `JUNOCLAW_ADMIN_RPC=1` and `JUNOCLAW_ADMIN_TOKEN` are set. Missing token while `JUNOCLAW_ADMIN_RPC=1` causes startup to fail loudly.

Zero new runtime dependencies. Uses Node's built-in `http` and `crypto` modules.

## Verification

132 unit tests passing across both packages, plus five live smokes:

- `mcp/src/admin/admin-rpc-test.ts` — 36 cases.
- `mcp/src/admin-rpc-smoke.ts` — 11 phases live against a real loopback HTTP listener.
- `wavs/bridge/src/admin/admin-rpc-test.ts` — 36 cases.
- `wavs/bridge/src/admin-rpc-smoke.ts` — 13 phases live; **headline coupling assertion** (Phases 3, 5, 7) verifies that arming the gate via the admin RPC actually causes `safeFetch()` to refuse in the same process.
- `wavs/bridge/src/utils/egress-pause-test.ts` — 15 cases.
- `wavs/bridge/src/utils/ssrf-guard-test.ts` — 45 regression cases (no changes since `v0.x.y-security-1`).

`tsc --noEmit` clean across both `mcp/` and `wavs/bridge/` workspaces.

## Compatibility

Operators who don't set the new env vars see no behavior change. The admin RPC is opt-in. The `signing_paused` env-var path from `v0.x.y-security-2` remains supported and unchanged. The four-witness incident-response procedure (`JUNOCLAW_SIGNING_PAUSED=1` + supervisor restart) still works — the admin RPC is a faster alternative, not a replacement.

## Not in this release

This release is **preventive hardening, not CVE-bearing**. The five GHSAs disclosed against `v0.x.y-security-1` remain scoped to that release.

The `sandbox_mode` kill-switch on `plugin-shell` is still startup-only; integrating it into the admin RPC is deferred to a future release because it requires additional plugin-runtime instrumentation.

The chain-layer `x/authz` integration (`v0.x.y-security-4+`) is the next major piece. See `SECURITY.md` for scope.

## Acknowledgements

- **Ffern Institute** — the audit that triggered this entire hardening track. The four-phase shape of `v0.x.y-security-3` was implicit in the *Levers* section of `SECURITY.md` written for `v0.x.y-security-1`, then deferred to give each primitive its own threat-model review window.

---

**Full diff:** [v0.x.y-security-2…v0.x.y-security-3](https://github.com/Dragonmonk111/junoclaw/compare/v0.x.y-security-2...v0.x.y-security-3)

**Commits:**

- `39a163e` — Phase 3a: `egress_paused` on the bridge
- `a644eee` — Phase 3b: admin RPC primitive (MCP)
- `7dab878` — Phase 3c: read-only `/policy` roll-up
- `b7d50c6` — Phase 3d: admin RPC primitive (WAVS)
- `2440228` — wiring + docs (this tag)

For the complete API and test surface, see [`CHANGELOG.md`](https://github.com/Dragonmonk111/junoclaw/blob/v0.x.y-security-3/CHANGELOG.md#v0xy-security-3) at the tagged SHA.
