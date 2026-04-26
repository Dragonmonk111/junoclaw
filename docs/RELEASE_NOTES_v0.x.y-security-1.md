# Release notes — `v0.x.y-security-1`

> **Purpose.** Paste-ready release-note body for the GitHub Release at
> <https://github.com/Dragonmonk111/junoclaw/releases/new?tag=v0.x.y-security-1>.
> Title field: `v0.x.y-security-1 — five walls`. Description: paste below.

---

The five walls. Closes all four critical and one high-severity finding from the Ffern Institute operator-side audit (April 2026).

## What's in this release

| ID | Layer | Wall |
|---|---|---|
| C-1 | `plugins/plugin-shell` | Shell-injection bypass via `BLOCKED_PATTERNS` substring blocklist → replaced with allowlist + Cargo `unsafe-shell` feature flag (off by default) |
| C-2 | `plugins/plugin-shell` | Shell-metacharacter injection via `sh -c` / `cmd /C` wrapper → `shell-words` argv parsing + direct `process::Command::spawn`, no shell |
| C-3 | `mcp/src/wallet/` | Plaintext mnemonic on the tool-call surface → wallet-handle registry, encrypted at rest (passphrase or OS keychain) |
| C-4 | `mcp/src/utils/path-guard.ts` | `upload_wasm` path traversal + symlink exfil → allow-root + symlink reject + size cap + magic-byte check |
| H-3 | `wavs/bridge/src/utils/ssrf-guard.ts` | SSRF in `computeDataVerify` → scheme/port allowlist, DNS pre-resolution, private-IP block, timeout, body cap |

Per-finding commits (chronological): `2bc54f6`, `a7886cd`, `a168608`, `339701e`, `7d17fd1`, `6b56230`.

Full prose in `CHANGELOG.md` (`[v0.x.y-security-1]` section) and `SECURITY.md` (Walls section).

## Verification

- 50 unit tests across the three packages (19 wallet-store + 21 keychain + 10 path-guard), tsc clean, cargo test clean
- On-chain proofs on `uni-7` (Juno testnet) for C-3:
  - Passphrase backend: TX `E2FB05A213D0C65C02EF0D5FAB1C7F8D4AF34BF275B9F0005F0B5A86FF9AED10`
  - Keychain (DPAPI) backend: TX `00D9AC4706A5AB923B7D45E0E97CE166F2EBD34402ECEB113DC7DC9A4702AF18`

## Upgrading

```bash
# npm-published MCP server:
npm install -g @junoclaw/cosmos-mcp@0.3.0    # ships with the patches

# Source builds: pull main, rebuild, redeploy:
git fetch --tags origin
git checkout v0.x.y-security-1
cargo build --release    # for the Rust workspace
cd mcp && npm install && npm run build
```

If you used MCP write tools with a raw mnemonic on a pre-patch version, **rotate that wallet** — generate a new seed on a cold device, transfer funds, retire the old seed. Detail and rationale in `CHANGELOG.md`.

## Credit

Audit by **Ffern Institute** (April 2026). Five findings, every one accepted; rapid responsible-disclosure cadence agreed. Public re-check pending.

Patches authored by **VairagyaNodes** in pair-programming with **Cascade** (the Windsurf coding agent) under direction. AI-assisted contribution policy in `NOTICE` and `CONTRIBUTORS.md`.

## What's next

- `v0.x.y-security-2` (already shipped — see its Release): the first lever, `signing_paused` runtime kill-switch
- `v0.x.y-security-3` (in design): admin RPC + `egress_paused` + policy-state RPC + hot-reload
- BN254 precompile signaling proposal: held until Ffern re-check completes

Five GHSAs with CVE assignment will be published shortly after the auditor signs off.
