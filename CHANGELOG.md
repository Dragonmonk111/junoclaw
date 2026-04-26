# Changelog

All notable changes to JunoClaw are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
JunoClaw is pre-`v1.0`; per-component versions (in `mcp/package.json`,
`wavs/bridge/package.json`, `Cargo.toml` files) move independently of
project-level security tags such as `v0.x.y-security-1`.

## [v0.x.y-security-3] — 2026-04-26

The remaining runtime levers. Verifiable controllability is now
operationally complete: hot-flip on both kill-switches, single-curl
mean-time-to-halt, downstream-pollable policy state. Closes out the
post-Ffern hardening track that began with `v0.x.y-security-1`.

### Phase breakdown

The release shipped in four primitive commits plus an integration
commit that wires both admin RPCs into their respective process
entry points:

- **Phase 3a** (`security(wavs):` 39a163e) — `egress_paused` runtime
  kill-switch on the WAVS bridge. Mirrors `signing_paused` from
  `v0.x.y-security-2` but applied to the SSRF-guarded fetcher.
- **Phase 3b** (`security(mcp):` a644eee) — admin RPC primitive on
  the MCP side. Localhost-only HTTP listener with bearer-token auth,
  Host/Origin defenses, rate limit, audit log. Hot-flips
  `signing_paused`.
- **Phase 3c** (`security(mcp):` 7dab878) — `GET /policy` read-only
  roll-up endpoint extending the MCP admin RPC. Lets downstream
  verifiers and dashboards poll the live kill-switch state.
- **Phase 3d** (`security(wavs):` b7d50c6) — admin RPC primitive on
  the WAVS bridge side. Mirrors Phase 3b for hot-flipping
  `egress_paused`. Headline assertion: in the smoke, the same process
  that received `POST /egress/pause` then refuses `safeFetch()` with
  `EgressPausedError`, end-to-end.
- **Wiring + docs** (this commit) — `mcp/src/index.ts` and
  `wavs/bridge/src/bridge.ts` start the admin RPC when both
  `JUNOCLAW_ADMIN_RPC=1` and `JUNOCLAW_ADMIN_TOKEN` are set; SIGINT/
  SIGTERM handlers close the listener gracefully; `SECURITY.md`
  *Levers* section retitled with operator runbook; this CHANGELOG
  entry; `mcp/README.md` admin-RPC subsection.

### Added

#### `egress_paused` kill-switch (Phase 3a)

- `EgressPausedError` — new exported error class from
  `wavs/bridge/src/utils/ssrf-guard.ts`. Carries the URL that was
  refused so operators can see in the log exactly what was blocked.
- `setEgressPaused(paused, source)` / `getEgressPaused()` /
  `getEgressPausedSource()` — module-level setters and getters with
  argument validation. `source` is a free-text label logged on
  every state change.
- `parseEgressPausedEnv(raw)` — fail-closed parser. Any non-empty
  value other than `"0"` / `"false"` / `"no"` / `"off"`
  (case-insensitive) arms the gate. Typos like `"flase"` arm rather
  than silently fail open.
- `safeFetch()` checks the gate **first**, before any URL parsing,
  DNS lookup, or fetch attempt. Refused requests incur zero side
  effects (verified by an explicit counter-based test).
- `JUNOCLAW_EGRESS_PAUSED` env var read at module load with a
  startup warning to stderr when armed.
- 15 unit tests (`wavs/bridge/src/utils/egress-pause-test.ts`) and a
  two-phase live smoke against `https://example.com/`
  (`wavs/bridge/src/egress-pause-smoke.ts`).
- `npm run egress-pause-test` and `npm run egress-pause-smoke`
  scripts in `wavs/bridge/package.json`.

#### Admin RPC primitive — MCP side (Phase 3b)

- New file `mcp/src/admin/rpc-server.ts`. Localhost-only HTTP
  listener (`127.0.0.1` bind enforced; constructor refuses
  `0.0.0.0` / `::1` / `::`), bearer-token auth (≥32-byte token,
  constant-time comparison via `crypto.timingSafeEqual`), Host
  header check, Origin header rejection, in-memory rate limit
  (default 10 req/60 s — fires before auth check so token-spamming
  cannot bypass the limit), audit log to stderr that never records
  the token. Zero new runtime dependencies (Node built-in `http` +
  `crypto`).
- Endpoints: `GET /health`, `GET /signing/status`, `POST /signing/pause`,
  `POST /signing/unpause`. All token-required. `/policy` added in
  Phase 3c.
- `SigningPausedController` interface — minimal contract the admin
  RPC needs from `WalletStore`. `WalletStore` satisfies it natively;
  tests inject a fake.
- 31 unit tests + 10-phase live smoke (real loopback HTTP listener
  on a fresh OS-assigned port).
- `npm run admin-rpc-test` and `npm run admin-rpc-smoke` scripts in
  `mcp/package.json`.

#### Read-only `/policy` endpoint (Phase 3c)

- `GET /policy` added to the MCP admin RPC. Returns
  `{process, version, tag, kill_switches: {signing_paused: {...}}, reported_at}`.
  Read-only, never mutates state.
- `processName?` option added to `AdminRpcServerOptions` (default
  `"mcp"`). Surfaces verbatim in the response so downstream tools
  can tell which process they're talking to.
- 5 additional unit tests + 1 additional live smoke phase. Combined
  MCP test count after Phase 3c: 36 passed, 0 failed.

#### Admin RPC primitive — WAVS bridge side (Phase 3d)

- New file `wavs/bridge/src/admin/rpc-server.ts`. Same threat model,
  defense layers, and wire format as the MCP admin RPC, with three
  intentional differences: (a) controls `egress_paused` instead of
  `signing_paused`; (b) exposes `/egress/{status,pause,unpause}`
  instead of `/signing/*`; (c) `processName` defaults to
  `"wavs-bridge"`.
- `EgressPausedController` interface and `defaultEgressController`
  adapter that wraps the module-level setters/getters from
  `ssrf-guard.ts` so the admin RPC and direct `safeFetch()` share
  state.
- 36 unit tests + 13-phase live smoke. The smoke's headline
  assertion (Phases 3, 5, 7) verifies the end-to-end coupling:
  `safeFetch()` succeeds while disarmed, `POST /egress/pause` flips
  module state, `safeFetch()` then refuses with `EgressPausedError`
  in the same process, and `POST /egress/unpause` restores fetch
  capability.

#### Entry-point wiring (this commit)

- `mcp/src/index.ts` — new `maybeStartAdminRpc()` helper reads the
  env-var contract, starts the listener, prints the URL to stderr,
  and registers SIGINT/SIGTERM handlers for graceful close. The
  wallet store passed to the listener is the cached singleton from
  `getDefaultWalletStore()`, so `signFor()` and the admin RPC see
  the same `signing_paused` state.
- `wavs/bridge/src/bridge.ts` — same helper pattern, with
  `defaultEgressController` as the controller. Top-level await is
  used (target ES2022, module ESNext).
- Both helpers fail loud if `JUNOCLAW_ADMIN_RPC=1` is set but
  `JUNOCLAW_ADMIN_TOKEN` is empty: a half-configured admin RPC is
  worse than no admin RPC.

### Env-var contract

| Variable | Process(es) | Required to enable admin RPC | Default | Notes |
| --- | --- | --- | --- | --- |
| `JUNOCLAW_ADMIN_RPC` | both | yes (`=1`) | unset (admin RPC off) | Off-by-default master switch. |
| `JUNOCLAW_ADMIN_TOKEN` | both | yes | unset | ≥32 bytes. Generate with `openssl rand -hex 32`. |
| `JUNOCLAW_ADMIN_RPC_PORT` | both | no | `0` (OS-assigned) | Bind port. The listener prints the actual URL on stderr at startup. |
| `JUNOCLAW_ADMIN_RPC_HOST` | both | no | `127.0.0.1` | Only `127.0.0.1` and `localhost` are accepted. |
| `JUNOCLAW_SIGNING_PAUSED` | mcp | n/a | unset | Startup-time arm of the kill-switch. Independent of the admin RPC. |
| `JUNOCLAW_EGRESS_PAUSED` | wavs | n/a | unset | Same shape, for the bridge. |

### Test additions

- **MCP side:** `mcp/src/admin/admin-rpc-test.ts` (36 cases) +
  `mcp/src/admin-rpc-smoke.ts` (11 phases live).
- **WAVS side:** `wavs/bridge/src/admin/admin-rpc-test.ts` (36 cases) +
  `wavs/bridge/src/admin-rpc-smoke.ts` (13 phases live, includes
  end-to-end coupling) + `wavs/bridge/src/utils/egress-pause-test.ts`
  (15 cases) + `wavs/bridge/src/egress-pause-smoke.ts` (2 phases).
- Combined v0.x.y-security-3 test surface: **132 unit cases passing**
  (36 + 36 + 15 + 45 ssrf-guard regression). All five live smokes
  pass (egress-pause-smoke, signing-pause-smoke, admin-rpc-smoke ×2,
  ssrf-guard-test as smoke-equivalent).

### Changed

- `SECURITY.md` *Levers* section retitled with the four primitives
  marked as shipped, and an operator runbook added (token
  generation, start-up env vars, incident-response curl commands,
  policy poll, resume after investigation).
- `SECURITY.md` Roadmap entry for `v0.x.y-security-3` flipped from
  *planned* to *this release*.
- `mcp/src/index.ts` `main()` — admin RPC startup hook before
  `server.connect`, SIGINT/SIGTERM handlers after.
- `wavs/bridge/src/bridge.ts` — admin RPC startup hook before
  `runLoop()`, SIGINT/SIGTERM handlers after.

### Security

The admin RPC introduces a **new network listener** in two
signing-sensitive processes. The listener is hardened against the
threats it could plausibly face on a single-operator deployment:

- **Local malicious process under the same user** — bearer token,
  ≥32 bytes, constant-time comparison, never logged.
- **Other users on the same multi-tenant box** — bind to
  `127.0.0.1` only; non-loopback hosts are rejected at constructor
  time.
- **Browser DNS-rebinding** — `Host:` header check against the
  bound socket; any `Origin:` header is rejected with 400.
- **Token brute-force** — in-memory rate limit (10 req/60 s by
  default), fires before the auth check.
- **Secrets in audit log** — token never appears in any
  `AuditEntry` field; verified by a test that JSON-stringifies the
  entry list and asserts the token does not appear.
- **Off-by-default** — both `JUNOCLAW_ADMIN_RPC=1` and
  `JUNOCLAW_ADMIN_TOKEN` must be set; missing token while
  `JUNOCLAW_ADMIN_RPC=1` causes startup failure.

The admin RPC has no privileged operations beyond hot-flipping
kill-switches that an operator could already flip via env-var +
restart in `v0.x.y-security-2`. The new attack surface buys
mean-time-to-halt at roughly 0 seconds in exchange for one extra
local listener; the threat-model trade-off is documented in the
Phase 3b commit message.

The five Ffern findings closed in `v0.x.y-security-1` remain scoped
to that release. This release is **preventive hardening, not
CVE-bearing**.

### Roadmap

- **`v0.x.y-security-4+`** — chain-layer integration: `x/authz`
  delegations bound to `wallet_id` so a compromised MCP can only
  drain delegated message types within chain-enforced expiry. See
  the *Phase 3 roadmap* section in `SECURITY.md` for scope.

---

## [v0.x.y-security-2] — 2026-04-26

The `signing_paused` runtime kill-switch. The first lever to complement
`v0.x.y-security-1`'s five walls.

### Scope split rationale

The `v0.x.y-security-1` roadmap entry listed four items for the `-security-2`
cycle: `signing_paused`, `egress_paused`, the published policy-state admin
RPC, and admin-RPC hot-reload. That bundle has been split. Only
`signing_paused` ships in this release; the other three move to
`v0.x.y-security-3`. The reason is targeted at high-value deployments: an
admin RPC is a new network listener in a signing-sensitive process, and
bundling it with the kill-switch primitive would (a) enlarge the attack
surface shipping with the primitive, (b) enlarge the Ffern re-check scope
against the same tag, and (c) weaken the modular-update framing the
`-security-1` release established (one commit per primitive, one tag).
Shipping the env-var-armed gate first, with the admin RPC to follow under
its own threat-model review, preserves the safer ordering.

During the `v0.x.y-security-2` window, the documented incident-response
procedure for high-value deployments is `JUNOCLAW_SIGNING_PAUSED=1` plus a
process-supervisor restart (systemd / PM2 / NSSM / launchd), which also
leaves a clean OS-level forensic trail.

### Added

- `SigningPausedError` — new exported error class from `mcp/src/wallet/store.ts`.
  Carries `walletId` and `chainId` so a downstream task scheduler can
  pattern-match on `instanceof SigningPausedError` and treat it as "operator
  halt, retry later" rather than a hard failure.
- `WalletStore.setSigningPaused(paused, source)` — public instance mutator.
  `source` is a free-text label logged on every state transition for operator
  forensics (e.g. `env:JUNOCLAW_SIGNING_PAUSED`, `admin-rpc:127.0.0.1` once
  `-security-3` ships, `test`).
- `WalletStore.getSigningPaused()` — public read method returning
  `{paused, source}`. For tests, metrics, and the future admin RPC.
- `WalletStore.signFor()` now checks the kill-switch **first**, before any
  file read or backend access. A paused signer therefore refuses for
  non-existent wallet IDs too — no wallet-enumeration signal via
  differentiated "paused" vs "not found" errors.
- `parseSigningPausedEnv()` helper (module-internal). Fail-closed on typos:
  any non-empty, non-`"0"` value of `JUNOCLAW_SIGNING_PAUSED` is treated as
  paused. Canonical value is `"1"`.
- `WalletStore.defaultStore()` now applies the startup-time kill-switch
  automatically, logging the state transition and a forensic tip.
- `mcp/src/wallet/signing-pause-test.ts` — 12-test regression suite covering
  the state machine, gate ordering (including the no-enumeration-leak
  property), the `SigningPausedError` shape, and the fact that `add` / `list`
  / `verifyAddress` / `remove` remain functional while paused (registry
  management is signing-independent).
- `mcp/src/signing-pause-smoke.ts` — two-phase on-chain proof: Phase A arms
  the kill-switch and expects `SigningPausedError`; Phase B disarms and
  expects a successful broadcast on `uni-7`. Reuses the `signing-smoke-uni7`
  wallet from the existing smoke test.
- `npm run signing-pause-test` and `npm run signing-pause-smoke` scripts in
  `mcp/package.json`.

### Changed

- `SECURITY.md` — *Levers* section retitled and reorganised to mark
  `signing_paused` as shipped in this release and the other three levers
  as planned for `v0.x.y-security-3`. Roadmap section reflects the same
  split; the original `v0.x.y-security-2` bundle is now split across
  `-security-2` and `-security-3` with the rationale documented in the
  *Levers* prose.

### Security

No new attack surface. The change is a defensive primitive that can
only **prevent** signing calls, never force them. The env var is read
once at startup by `defaultStore()` and does not create a persistent
configuration file, remote endpoint, or unauthenticated control plane.
The `setSigningPaused` method is an instance method with no external
exposure; the future admin RPC (which *is* a new attack surface) is
deliberately deferred to `v0.x.y-security-3`.

### On-chain proof

Phase A + Phase B both exercised against `uni-7` (Juno testnet) via
`npm run signing-pause-smoke`:

- **Phase A (armed) — `SigningPausedError` raised as expected.** No on-chain
  TX; the gate refuses before the signing client is even constructed
  (instance state: `paused=true, source=smoke:phase-A`).
- **Phase B (disarmed, source flipped to null) — successful broadcast.**
  - TX hash: `346CC7FF418019A4FBA68D7847112954E2D8D9ECE3E27B314357408E8AE42B6A`
  - Signer: `juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6`
  - Gas used: 72,581
  - Explorer: <https://testnet.mintscan.io/juno-testnet/tx/346CC7FF418019A4FBA68D7847112954E2D8D9ECE3E27B314357408E8AE42B6A>

The smoke exercises the gate inside the same `sendTokens → tx-builder.ts →
WalletStore.signFor()` code path used by every production write tool;
no test-specific scaffolding bypasses the production path.

### Roadmap

- **`v0.x.y-security-3`** — the remaining levers deferred from the
  originally-scoped `-security-2` bundle: `egress_paused` on the WAVS
  SSRF-guarded fetcher, the published policy-state admin RPC (localhost-only,
  token-gated, off-by-default, no third-party deps, constant-time token
  comparison, rate-limited, audit-logged), and admin-RPC hot-reload of all
  kill-switches. Mean-time-to-halt drops from process-supervisor restart
  (5–30 s) to ~200 ms. Designed for its own Ffern re-check.

---

## [v0.x.y-security-1] — 2026-04-26

The post-Ffern operator-side hardening release. Closes the four critical
and one high-severity findings of the Ffern Institute audit (April 2026).

### Architectural framing

JunoClaw's security model splits into four locks: hardware (TEE attestation,
already on-chain via Juno Prop #373), mathematical (BN254 ZK precompile,
signaling proposal in flight), declarative (Tier-1.5 constraint vocabulary,
live on `uni-7` as `agent-company` v7), and **operational — Lock 4: is the
operator observably in control of the agent fleet?** All five Ffern findings
landed at Lock 4, and so does this release.

The release is **five walls and one explicit absence** of the lever. Walls
prevent the next failure mode by hardening inputs and gating capabilities at
compile or load time. The lever — runtime kill-switch on the signing path —
makes a flow already in motion stoppable without taking the system down.
Walls without a lever leave the operator with no graceful response to
surprise. This release ships the walls; the lever (`signing_paused`) opens
the next release cycle, `v0.x.y-security-2`.

### Added

#### Wall 1 — `plugins/plugin-shell` compile-time gate (Ffern C-1, C-2)

- New Cargo feature `unsafe-shell`, default OFF. With the feature off, the
  shell- and Python-execution code paths are not compiled into the binary;
  the public methods compile to a stub returning a clear error pointing at
  `SECURITY.md`.
- Replaced the `BLOCKED_PATTERNS` substring blocklist (folk wisdom that
  doesn't work — every interesting bypass was documented decades ago) with
  a strict allowlist applied to the parsed first token of the command.
- Dropped the shell wrapper (`sh -c` / `cmd /C`); commands spawn directly,
  so metacharacters have no special meaning to the OS because no shell is
  involved.
- `run_python` now uses `python -E -I -S -B -` via stdin (no on-disk script
  artefact), `env_clear()` on the child process, an isolated `tempfile::TempDir`
  CWD, 1 MiB output cap per stream, and `kill_on_drop(true)`.
- `sandbox_mode: bool` on `ShellPlugin` is the runtime kill-switch; default
  empty `allowed_commands` means the plugin is fail-closed even when the
  feature is compiled in.
- Files: `plugins/plugin-shell/src/lib.rs`, `plugins/plugin-shell/Cargo.toml`,
  `crates/junoclaw-runtime/{src/lib.rs,Cargo.toml}`, `Cargo.lock`.

#### Wall 2 — `mcp/src/utils/path-guard.ts` (Ffern C-4)

- New input-validation guard for `upload_wasm`. The audit noted the tool
  accepted unbounded local paths with no allow-root, no symlink check, no
  size cap, and no magic-byte verification. A symlink under the operator's
  home directory could exfiltrate any readable file via on-chain bytes.
- The guard runs eight checks in order: (1) non-empty input, (2) `WASM_ROOT`
  exists, (3) `lstat` rejects leaf symlinks, (4) `realpath` rejects parent-symlink
  escapes, (5) pre-read size check (default 8 MiB), (6) read, (7) post-read
  size check (TOCTOU defense against file growth), (8) wasm magic bytes
  (`\0asm`).
- `JUNOCLAW_WASM_ROOT` defaults to `~/.junoclaw/wasm`; override via env var
  or per-call options.
- 10 test cases in `mcp/src/utils/path-guard-test.ts` cover happy path,
  empty/missing path, missing WASM_ROOT, outside WASM_ROOT, traversal `..`
  segment, undersized file, oversized file, custom maxBytes, and wrong magic
  bytes. Symlink-creation tests skip gracefully on Windows (EPERM).
- Files: `mcp/src/utils/path-guard.ts`, `mcp/src/utils/path-guard-test.ts`,
  `mcp/src/tools/tx-builder.ts` (integration).

#### Wall 3 — `mcp/src/wallet/` wallet handle registry (Ffern C-3)

The audit's most operationally consequential finding: every MCP write tool
took `mnemonic: string` as a parameter. The mnemonic flowed through the LLM
tool-call JSON, the MCP transport, and conversation logs. Anyone with a log
got the seed.

The fix replaces `mnemonic` with an opaque `wallet_id` handle, ships in two
phases that were both completed for this release, and is documented in detail
in `mcp/README.md`.

**Phase 1 — passphrase backend.**
- `WalletStore` registry: encrypted-at-rest mnemonic store. `~/.junoclaw/wallets/<id>.enc`
  is an AES-256-GCM envelope under a 32-byte data-encryption key.
- `PassphraseKeyStore` derives the master DEK from `JUNOCLAW_WALLET_PASSPHRASE`
  via scrypt (N=2^17, r=8, p=1) with a fresh 32-byte salt persisted in
  `.keystore.json`. Same passphrase across distinct roots produces independent
  salts.
- AES-256-GCM with a fresh 12-byte IV per encrypt: two wallets with the same
  mnemonic still produce distinct ciphertexts.
- POSIX file mode 0600 on enrollment files (skipped on Windows where POSIX
  perms don't apply).
- New CLI: `cosmos-mcp wallet add | list | rm`. The `add` subcommand reads
  the mnemonic from stdin (no echo if a TTY) or from a named env var; the
  mnemonic never appears as a process argument.
- The MCP write tool schemas changed: `mnemonic: string` → `wallet_id: string`.
  The deleted `getSigningClient(chain, mnemonic)` helper is the canonical
  signal that the unsafe path is gone.

**Phase 2 — keychain backend (operating-system credential manager).**
- `KeychainKeyStore` stores a fresh random 32-byte DEK *per wallet* in the
  OS credential manager via the optional `@napi-rs/keyring` native binding.
  Indexing: `(service="junoclaw-cosmos-mcp", account=walletId)`.
- Backend selection is per-wallet: each `.enc` file records `backend:
  "passphrase" | "keychain"`. The `WalletStore` dispatches on the recorded
  backend at decrypt time, so a single store can hold a mix.
- Files written before April 2026 (no `backend` field) are read as
  `"passphrase"` for backward compatibility.
- `KeyringDriver` interface decouples `KeychainKeyStore` from the native
  binding. `InMemoryKeyringDriver` gives full mock-keychain coverage in tests;
  `loadNativeKeyringDriver()` lazy-loads `@napi-rs/keyring` only on first use.
- The `cosmos-mcp wallet add` CLI gained a `--backend passphrase|keychain`
  flag; the default is auto-selected from `JUNOCLAW_WALLET_DEFAULT_BACKEND`
  → keychain (if no passphrase env var) → passphrase.
- 21 test cases in `mcp/src/wallet/keychain-store-test.ts` cover the in-memory
  driver, the `KeychainKeyStore` (DEK generation, idempotence, distinctness,
  corruption rejection, service isolation), the `WalletStore`+keychain pipeline
  (round-trip, recorded-backend-on-disk, tamper detection, revoked-keychain-entry
  rejection), multi-backend dispatch, validation (unknown/empty/invalid-default),
  Phase 1 backward compatibility, and **two real on-OS keychain round-trips**
  (DPAPI on Windows / Keychain on macOS / libsecret on Linux). Native tests
  skip gracefully if `@napi-rs/keyring` is not installed.

**Combined Phase 1 + Phase 2 verification.**
- 19 + 21 = 40 wallet-store tests passing on Windows, including 2 real DPAPI
  round-trips.
- On-chain proofs on `uni-7` (Juno testnet, code path: enrol → DEK → AES-GCM
  encrypt → broadcast):
  - **Passphrase backend:** TX `E2FB05A213D0C65C02EF0D5FAB1C7F8D4AF34BF275B9F0005F0B5A86FF9AED10`
  - **Keychain backend (DPAPI):** TX `00D9AC4706A5AB923B7D45E0E97CE166F2EBD34402ECEB113DC7DC9A4702AF18`
- Files: `mcp/src/wallet/{store,crypto,key-store,keychain-store,cli}.ts`,
  `mcp/src/wallet/{store-test,keychain-store-test}.ts`,
  `mcp/src/{index,signing-smoke}.ts`, `mcp/src/tools/tx-builder.ts`,
  `mcp/src/utils/cosmos-client.ts`, `mcp/package.json`, `mcp/README.md`.

#### Wall 4 — `wavs/bridge/src/utils/ssrf-guard.ts` (Ffern H-3)

- New input-validation guard for outbound HTTP. The audit noted that
  `computeDataVerify` in `local-compute.ts` called `fetch(url)` on
  agent-supplied URLs with zero validation. A compromised or prompt-injected
  agent could exfiltrate cloud-metadata credentials (`169.254.169.254` →
  AWS IAM, GCP service-account tokens, Azure managed identity), poke admin
  endpoints on RFC 1918 networks (Redis FLUSHALL on `localhost:6379`, Cosmos
  RPC admin on `26657`, Elasticsearch on `9200`), or use non-HTTP schemes
  (`file:///etc/passwd`, `gopher://`).
- Layered defenses: (1) scheme allowlist (default `http:` / `https:`),
  (2) port allowlist (default `80` / `443`), (3) DNS pre-resolution with
  per-IP private-range block (IPv4 RFC 1918, CGNAT, link-local, multicast,
  reserved; IPv6 loopback, ULA, link-local, multicast; IPv4-mapped IPv6 in
  both dotted and hex forms; `169.254.169.254` cloud-metadata included),
  (4) 5-second timeout via `AbortController`, (5) 1 MiB body cap via
  streaming abort.
- Every default is overridable via options or `JUNOCLAW_SSRF_*` env vars.
  Limitations (TOCTOU DNS rebinding, IPv6 special-use coverage gaps) are
  documented inline.
- Files: `wavs/bridge/src/utils/ssrf-guard.ts`, `wavs/bridge/src/local-compute.ts`,
  `wavs/bridge/package.json`, `wavs/README.md`.

#### Documentation

- `SECURITY.md` (new at repo root): disclosure policy, supported versions,
  four-witness threat model, verifiable-controllability primitives, Phase 3
  `x/authz` roadmap section, deployment-time isolation guidance, AI-assisted
  contribution posture under Apache 2.0.
- `docs/FFERN_THANK_YOU_PM.md` (new): private-message body to the Ffern
  contact acknowledging the audit and requesting the re-check.
- `docs/HACKMD_BN254_PROPOSAL.md` (new): governance signaling proposal for
  upstream BN254 precompile work; references this hardening pass.
- `docs/GOV_PROP_COPYPASTE_BN254.md` (new): on-chain proposal copy-paste body.
- `docs/MEDIUM_ARTICLE_AFTER_THE_VOTE.md` (updated): post-vote retrospective
  reframed as the pre-audit hardening pass.

### Changed

- `mcp/src/index.ts`: every write-tool schema's `mnemonic` parameter replaced
  with `wallet_id`. Tool descriptions updated.
- `mcp/src/utils/cosmos-client.ts`: deleted `getSigningClient(chain, mnemonic)`
  helper; the only signing path is now via `WalletStore.signFor(walletId, chain)`,
  which decrypts the mnemonic, builds a single `SigningCosmWasmClient`, and
  zeroes the buffer in `finally{}`.
- `mcp/src/tools/tx-builder.ts`: every tool function now takes `walletId:
  string` and resolves it via `getDefaultWalletStore().signFor(...)`.
- `mcp/src/signing-smoke.ts`: backend-aware precondition check (passphrase
  env var OR keychain backend available) replaces the Phase-1-only
  passphrase-required guard.
- `mcp/package.json`: added `@napi-rs/keyring` as an `optionalDependencies`
  entry; new test scripts `keychain-store-test` and (already present)
  `wallet-store-test`, `path-guard-test`.
- `mcp/README.md`: new "Wallet registry" and "`upload_wasm` security" sections;
  Security section flipped from listing C-3 Phase 2 as planned to listing it
  as shipped.

### Removed

- `mcp/src/utils/cosmos-client.ts::getSigningClient(chain, mnemonic)`. The
  function is gone, not deprecated. Callers that took a raw mnemonic now fail
  to compile.
- The default `allowed_commands` list in `plugins/plugin-shell` (was
  `[python, python3, echo, ls, dir, pwd, cat, type]`). Default is now empty
  (fail-closed). Operators must explicitly opt in to specific binaries.
- The `BLOCKED_PATTERNS` substring blocklist in `plugins/plugin-shell`.
  Replaced by allowlist-on-parsed-first-token; the substring approach is
  unfixable in principle.

### Security

The five Ffern findings closed by this release are all **off-chain,
operator-side, upstream-of-mempool** failures. Cosmos SDK chain mechanics
(signature verification, replay protection, gas metering, CosmWasm sandboxing)
correctly do not, and should not, intervene at this layer — chains settle
signed transactions; how the operator obtained the signing key, where the
WASM bytes came from, what URLs the off-chain helper code fetched, and what
shell commands the plugin runtime executed are all outside the consensus
boundary by design. The chain layer's adjacent primitives (`x/authz`,
`x/feegrant`, `x/group`) provide *blast-radius* limits on a compromised hot
key but cannot prevent the compromise itself; integrating them is named as
Phase 3 in `SECURITY.md`.

### Roadmap

- **`v0.x.y-security-2`** (next release cycle): runtime kill-switches —
  `signing_paused` (gate on `WalletStore.signFor`), `egress_paused` (gate on
  the SSRF-guarded fetcher), published policy-state admin RPC, SIGHUP /
  admin-RPC hot-reload of all kill-switches. The "lever" complement to this
  release's "walls."
- **Phase 3 — `x/authz` integration** (post-v0.x.y-security-2): wallet handles
  bind to authz delegations from a separate cold key the MCP process never
  touches. A compromised MCP process can drain only the authz-delegated message
  types within the chain-enforced expiry, not the cold key. This is the
  natural Lock 4 primitive at the chain layer; it complements the off-chain
  primitives shipped in this release.
- **Re-audit gate.** This release is offered to the Ffern Institute for a
  focused re-check before any further release tag. Public announcement and
  the BN254 precompile signaling proposal both gate on the re-check outcome.

[v0.x.y-security-1]: https://github.com/Dragonmonk111/junoclaw/releases/tag/v0.x.y-security-1
[v0.x.y-security-2]: https://github.com/Dragonmonk111/junoclaw/releases/tag/v0.x.y-security-2
