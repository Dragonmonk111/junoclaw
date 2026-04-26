# Release notes — `v0.x.y-security-2`

> **Purpose.** Paste-ready release-note body for the GitHub Release at
> <https://github.com/Dragonmonk111/junoclaw/releases/new?tag=v0.x.y-security-2>.
> Title field: `v0.x.y-security-2 — first lever (signing_paused)`. Description: paste below.

---

The first lever. Complements the five walls of `v0.x.y-security-1` with a runtime kill-switch on signing.

## What's in this release

A single primitive: `signing_paused`, an env-var-armed boolean gate on `WalletStore.signFor()`. When armed, every MCP write tool refuses with a dedicated `SigningPausedError` carrying the wallet ID and chain ID. Wallet registry management (`add`, `list`, `verifyAddress`, `remove`) and every read tool keep working so the operator can investigate during an incident.

Arming and disarming is a one-line env var change followed by an MCP restart:

```bash
# Arm:
export JUNOCLAW_SIGNING_PAUSED=1
cosmos-mcp                  # restart the MCP process

# Disarm:
unset JUNOCLAW_SIGNING_PAUSED
cosmos-mcp                  # restart again
```

Mean-time-to-halt in this release is process-supervisor restart (5–30 s). The admin-RPC hot-flip (drops MTH to ~200 ms) is scheduled for `v0.x.y-security-3`. The env var is fail-closed on typos: any non-empty, non-`"0"` value arms the gate.

## Scope split — why this is a separate release

The originally-bundled admin RPC and `egress_paused` were deliberately split out to `v0.x.y-security-3`. The admin RPC introduces a new network listener in a signing-sensitive process and deserves its own threat-model review window. Rationale in `CHANGELOG.md` (`Scope split rationale`) and `SECURITY.md` (Levers section).

## Verification

- 12 new unit tests in `mcp/src/wallet/signing-pause-test.ts` (state machine, gate ordering, error shape, registry-while-paused)
- Full suite: 62 passed, 0 failed (19 wallet-store + 21 keychain + 10 path-guard + 12 signing-pause); tsc clean
- On-chain proof on `uni-7` (Juno testnet):
  - Phase A (armed): `SigningPausedError` raised as expected, no TX
  - Phase B (disarmed): TX `346CC7FF418019A4FBA68D7847112954E2D8D9ECE3E27B314357408E8AE42B6A`
  - Signer: `juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6`
  - Explorer: <https://testnet.mintscan.io/juno-testnet/tx/346CC7FF418019A4FBA68D7847112954E2D8D9ECE3E27B314357408E8AE42B6A>

The smoke exercises the gate inside the same `sendTokens → tx-builder → WalletStore.signFor()` code path used by every production write tool — no test-specific scaffolding bypasses production.

## Upgrading

```bash
git fetch --tags origin
git checkout v0.x.y-security-2
cd mcp && npm install && npm run build
```

No mandatory configuration changes. The kill-switch is opt-in via env var.

## Operator docs

- `mcp/README.md` → *Runtime kill-switch* section: full operator-facing prose with PowerShell + bash, systemd / PM2 / NSSM / launchd integration, and the incident-response procedure
- `SECURITY.md` → *Levers* section: design rationale and roadmap context

## Credit

Lever design and patches authored by **VairagyaNodes** in pair-programming with **Cascade** (the Windsurf coding agent) under direction. AI-assisted contribution policy in `NOTICE` and `CONTRIBUTORS.md`.

The Ffern Institute audit window remains open for re-check on both `v0.x.y-security-1` and `v0.x.y-security-2`. CVEs will be requested for the five `-security-1` findings on auditor sign-off; this release is preventive hardening with no associated CVE.

## What's next

- `v0.x.y-security-3` (in design): admin RPC + `egress_paused` + policy-state RPC + hot-reload
- BN254 precompile signaling proposal: held until Ffern re-check completes
