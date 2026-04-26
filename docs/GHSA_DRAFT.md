# GitHub Security Advisory — Draft Text

> **Purpose of this file.** Pre-publication copy for the five GHSA entries on
> <https://github.com/Dragonmonk111/junoclaw/security/advisories/new>.
> One advisory per finding; request one CVE per advisory. Publishing is
> **gated on Ffern re-check sign-off on tag `v0.x.y-security-1` (commit
> `6b56230`)** per `docs/FFERN_THANK_YOU_PM.md`. Do NOT publish until
> Ffern confirms; at publication time, coordinate announcement on
> Telegram / Discord / Commonwealth / Twitter / Medium in the same hour.

---

## General fields (shared across all 5 advisories)

- **Ecosystem:** Other / Github-hosted source (JunoClaw does not currently
  publish to npm or crates.io; if that changes, add `npm` +
  `@junoclaw/cosmos-mcp` and `cargo` + `junoclaw-plugin-shell` as affected
  packages).
- **Affected versions range:** `< v0.x.y-security-1` (equivalently: any
  commit at or before `d3f4a2e`, the last commit before the security
  release on `main`).
- **Patched versions:** `>= v0.x.y-security-1` (commit `6b56230` and
  later).
- **Credit:** Ffern Institute (subject to their permission — the
  Thank-You PM invites them to redact if preferred).
- **CVE request:** yes, one per advisory.
- **Coordinator:** VairagyaNodes (repo maintainer), disclosure handled
  with the pair-programming coding agent Cascade.

---

## Advisory 1 — C-1: Shell-injection bypass in `plugin-shell` via `BLOCKED_PATTERNS`

- **Title:** `plugin-shell` shell-injection bypass via substring blocklist
- **Severity:** Critical
- **CVSS 3.1 (suggested):** `AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:H` → **8.4** (local vector because MCP runs on operator host, but single-instruction compromise yields complete host takeover)
- **CWE:** CWE-78 (OS Command Injection), CWE-184 (Incomplete List of Disallowed Inputs)

### Summary

`plugins/plugin-shell` enforced command safety via a `BLOCKED_PATTERNS`
substring blocklist applied to the raw command string. Because the check
ran on the full argument blob — not on the parsed executable — adversarial
argument constructions could bypass it. Combined with the shell-wrapping
pattern (Advisory 2), this allowed arbitrary host command execution from
any adversarial LLM tool call.

### Impact

A malicious or prompt-injected agent could execute arbitrary host
commands — exfiltrate files, install persistence, tamper with keychain
entries, read OS credential stores. On a signing host, the attacker
could also exfiltrate the decrypted mnemonic if the keychain backend was
unlocked.

### Patches

Fixed in `v0.x.y-security-1` (commit `2bc54f6`):

- `BLOCKED_PATTERNS` deleted entirely.
- Replaced with an allowlist check on the parsed first token via
  `shell-words::split`.
- `allowed_commands` defaults to empty `[]` — operators must explicitly
  opt in to specific binaries.
- The whole feature is now gated behind a Cargo `unsafe-shell` feature
  flag that is **off by default**. Code ships compiled-out unless
  explicitly enabled.

### Workarounds (for unpatched versions)

- Do not enable `plugin-shell` until upgraded.
- If already compiled in, run MCP as an unprivileged user in a
  filesystem-sandboxed environment (bubblewrap / jailed systemd unit /
  Windows AppContainer).

### References

- Commit: <https://github.com/Dragonmonk111/junoclaw/commit/2bc54f6>
- CHANGELOG: <https://github.com/Dragonmonk111/junoclaw/blob/v0.x.y-security-1/CHANGELOG.md>

---

## Advisory 2 — C-2: Shell-injection in `plugin-shell` via `sh -c` / `cmd /C` wrapping

- **Title:** `plugin-shell` shell-metacharacter injection via shell wrapper
- **Severity:** Critical
- **CVSS 3.1 (suggested):** `AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:H` → **8.4**
- **CWE:** CWE-77 (Improper Neutralization of Special Elements used in a Command), CWE-78 (OS Command Injection)

### Summary

`plugin-shell`'s `run_command` wrapped every agent-supplied command in
`sh -c` (POSIX) or `cmd /C` (Windows) and passed the entire argument
string to the shell's parser. Any shell metacharacter in the arguments
(`;`, `|`, `&&`, backticks, `$(...)`, redirection, quoted arguments
containing `$`, etc.) was interpreted by the shell, yielding
arbitrary-command execution on top of the allowlist bypass of
Advisory 1.

### Impact

Identical to Advisory 1. The two findings compound: C-1 bypasses the
blocklist, C-2 turns benign-looking arguments into shell injection.

### Patches

Fixed in `v0.x.y-security-1` (commit `2bc54f6`):

- Shell wrapper deleted. No more `sh -c` / `cmd /C`.
- Commands parsed via `shell-words::split` into argv, then spawned
  directly with `std::process::Command` — zero shell interpretation.
- `env_clear()` on the child process (blank environment).
- CWD isolated to an auto-deleted `tempfile::TempDir`.
- Output capped at 1 MiB.
- `kill_on_drop(true)` so timed-out processes are terminated, not
  orphaned.
- `run_python` switched from temp-file to stdin-piped
  `python -E -I -S -B -` (no on-disk script artefact).
- Runtime kill-switch `sandbox_mode: bool` retained at startup time.
  Hot-reload via admin RPC is scheduled for `v0.x.y-security-3`.

### Workarounds

Same as Advisory 1.

### References

- Commit: <https://github.com/Dragonmonk111/junoclaw/commit/2bc54f6>

---

## Advisory 3 — C-3: Plaintext mnemonic exposure via MCP write-tool parameters

- **Title:** MCP write tools exposed raw BIP-39 mnemonic as a tool-call parameter
- **Severity:** Critical
- **CVSS 3.1 (suggested):** `AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:N` → **8.1** (local vector because MCP is a local-process boundary, but the leak targets arbitrary logs shared with third parties — many deployments route MCP transport or LLM tool-call logs off-host, which would raise the vector to network for those specific deployments)
- **CWE:** CWE-522 (Insufficiently Protected Credentials), CWE-532 (Insertion of Sensitive Information into Log File), CWE-312 (Cleartext Storage of Sensitive Information)

### Summary

Every MCP write tool (`send_tokens`, `execute_contract`,
`instantiate_contract`, `upload_wasm`, etc.) took `mnemonic: string` as
an explicit parameter. The LLM composed the tool call with the
mnemonic literally embedded in the JSON body. As a result the 24-word
BIP-39 seed flowed through:

- The LLM provider's telemetry / logs (OpenAI, Anthropic, etc., per
  the operator's chosen backend).
- The MCP transport and any proxy, replay tool, or log sink in front
  of it (Claude Desktop logs, mitmproxy dumps, `tee`-ed stdio).
- The conversation-history storage of whatever client the agent was
  running inside.
- Any off-host log aggregator (Datadog, Loki, syslog forwarding).

Anyone with read access to any of those sinks could reconstruct the
mnemonic and drain the wallet.

### Impact

Full wallet compromise. On Cosmos SDK chains including Juno, the seed
derives the full HD-path; every sub-account and every linked contract
admin privilege is exposed.

### Patches

Fixed in `v0.x.y-security-1` (commit `339701e`):

- New wallet-handle registry at `mcp/src/wallet/`: mnemonic enrolled
  once via CLI (`cosmos-mcp wallet add`), stored encrypted at rest,
  referenced thereafter by opaque `wallet_id: string`.
- Every MCP write-tool schema's `mnemonic` parameter replaced with
  `wallet_id`.
- `getSigningClient(chain, mnemonic)` function deleted — not
  deprecated. Callers that tried to pass a raw mnemonic now fail at
  compile time.
- Two backends ship in this release:
  - **Passphrase backend:** scrypt N=2^17 r=8 p=1 from
    `JUNOCLAW_WALLET_PASSPHRASE`, AES-256-GCM envelope at rest
    (fresh 12-byte IV per wallet), POSIX 0600 file perms where
    applicable.
  - **Keychain backend:** fresh random 32-byte DEK per wallet, stored
    in the OS credential manager (DPAPI on Windows, Keychain on
    macOS, libsecret on Linux) via optional `@napi-rs/keyring`
    native binding. Backend selected per wallet and recorded in the
    enrolment file.
- Mnemonic is decrypted only inside `WalletStore.signFor()` for the
  lifetime of one `SigningCosmWasmClient` construction, then the
  buffer is zeroed in `finally{}`.
- 19 + 21 tests (`store-test.ts`, `keychain-store-test.ts`) include
  tamper detection, backend round-trip, POSIX perms, IV freshness,
  path-traversal ID rejection, and real OS keychain round-trip.
- On-chain proofs on uni-7:
  - Passphrase: TX `E2FB05A213D0C65C02EF0D5FAB1C7F8D4AF34BF275B9F0005F0B5A86FF9AED10`
  - Keychain (DPAPI): TX `00D9AC4706A5AB923B7D45E0E97CE166F2EBD34402ECEB113DC7DC9A4702AF18`

### Workarounds (for unpatched versions)

- **Assume any mnemonic that ever passed through MCP on a pre-patch
  version is compromised.** Rotate wallets: generate a new seed on a
  cold device, transfer funds, retire the old seed.
- Until upgraded, do not use MCP write tools on mainnet. Use the MCP
  query surface only and sign transactions via CLI (`junod tx ...`)
  or a hardware wallet.

### References

- Commit: <https://github.com/Dragonmonk111/junoclaw/commit/339701e>
- Module: <https://github.com/Dragonmonk111/junoclaw/tree/v0.x.y-security-1/mcp/src/wallet>

---

## Advisory 4 — C-4: Path-traversal / symlink-follow / unbounded-size in `upload_wasm`

- **Title:** `upload_wasm` accepted arbitrary filesystem paths without validation
- **Severity:** High (critical in deployments where the WASM being uploaded is auto-instantiated as a contract admin)
- **CVSS 3.1 (suggested):** `AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:L` → **7.9**
- **CWE:** CWE-22 (Path Traversal), CWE-59 (Link Following), CWE-400 (Uncontrolled Resource Consumption), CWE-20 (Improper Input Validation)

### Summary

The `upload_wasm` MCP tool accepted a filesystem path from the agent
and uploaded whatever bytes the path resolved to. No validation of:

- **Location.** Agent could specify any readable path on the host.
- **Symlink escape.** Agent could point at a symlink whose target lay
  outside any intended sandbox.
- **Size.** No upper bound; a multi-gigabyte file would be happily
  read into memory and transmitted.
- **Format.** No check that the bytes were actually a WASM module.

### Impact

A compromised or prompt-injected agent could:

- Upload a WASM that it authored from scratch elsewhere on the host
  (agent writes to `/tmp/backdoor.wasm`, then calls `upload_wasm`
  pointing at `/tmp/backdoor.wasm`), instantiating it as a contract
  on a chain where the operator's wallet has `MsgStoreCode` permission.
- Use `upload_wasm` as an arbitrary-file-read oracle (chain responses
  and log lines may include file size / hash, leaking host state).
- DoS the host via OOM by asking it to upload a 10 GB file.

### Patches

Fixed in `v0.x.y-security-1` (commit `a7886cd`):

- New `mcp/src/utils/path-guard.ts` module with `validateWasmPath()`.
- **Allow-root.** `WASM_ROOT` (default `~/.junoclaw/wasm`) defines the
  only directory under which uploads are permitted. Any path outside
  the real canonicalised root is rejected.
- **Symlink reject.** Any symlink encountered anywhere in the path is
  rejected outright — we do not follow even if the target is inside
  the allow-root. (Symlink replacement races are thereby also
  foreclosed.)
- **Size cap.** Default 8 MiB, configurable via `options.maxBytes`.
- **Magic-byte check.** First 4 bytes must equal `\0asm` (WASM v1
  module magic). Catches both format-wrong uploads and truncated
  files.
- 10 unit tests in `mcp/src/utils/path-guard-test.ts` covering all
  four defences plus edge cases (empty path, non-existent,
  custom-root, traversal segment).

### Workarounds

- Until upgraded, do not expose `upload_wasm` to an agent. Upload WASM
  manually via CLI (`junod tx wasm store`) after operator review.
- If already exposed, audit the logs for any `upload_wasm` tool call
  and verify the uploaded WASM code-IDs are what was expected.

### References

- Commit: <https://github.com/Dragonmonk111/junoclaw/commit/a7886cd>
- Module: <https://github.com/Dragonmonk111/junoclaw/blob/v0.x.y-security-1/mcp/src/utils/path-guard.ts>

---

## Advisory 5 — H-3: SSRF in `computeDataVerify` enabling cloud-metadata theft and internal reconnaissance

- **Title:** SSRF in WAVS `computeDataVerify` allows cloud-metadata and internal-service access
- **Severity:** High
- **CVSS 3.1 (suggested):** `AV:N/AC:L/PR:N/UI:R/S:C/C:H/I:N/A:L` → **8.8** (network vector because the URL is agent-supplied; scope-changed because the attack pivots from the compute worker into whatever service answers at the target URL, and cloud-metadata access yields IAM credentials for an adjacent system)
- **CWE:** CWE-918 (Server-Side Request Forgery)

### Summary

`wavs/bridge/src/local-compute.ts`'s `computeDataVerify` accepted a list
of `dataSources` URLs from the agent and called `fetch(url)` on each
without validating scheme, port, or resolved IP. A malicious or
prompt-injected agent could force the WAVS compute worker to request:

- `http://169.254.169.254/latest/meta-data/iam/security-credentials/` —
  AWS instance-metadata IAM tokens.
- `http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token` —
  GCP service-account tokens.
- `http://10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `100.64.0.0/10` —
  RFC 1918 and CGNAT neighbours.
- `http://127.0.0.1:6379/flushall` — unauthenticated Redis on loopback.
- `http://127.0.0.1:26657/*` — Cosmos RPC admin endpoints.
- `file:///etc/passwd`, `gopher://...` — non-HTTP scheme abuse.

### Impact

- Cloud-IAM-credential theft on AWS / GCP / Azure managed-identity
  environments.
- Exfiltration of internal-service data (Redis, Elasticsearch, Cosmos
  RPC admin, etc.) from any host reachable by the worker's network
  namespace.
- DoS of internal services the worker can reach.

### Patches

Fixed in `v0.x.y-security-1` (commit `a168608`):

- New `wavs/bridge/src/utils/ssrf-guard.ts` module with `safeFetch()`.
- **Scheme allowlist:** `http:`, `https:` only (default; overridable).
- **Port allowlist:** 80, 443 only (default; overridable).
- **DNS pre-resolution + private-IP block:** every hostname is resolved
  before the request; every resolved IP is checked against:
  - IPv4: RFC 1918 (`10/8`, `172.16/12`, `192.168/16`), CGNAT
    (`100.64/10`), link-local (`169.254/16` — catches cloud metadata),
    loopback (`127/8`), multicast (`224/4`), reserved (`240/4`).
  - IPv6: loopback (`::1`), ULA (`fc00::/7`), link-local (`fe80::/10`),
    multicast (`ff00::/8`).
  - IPv4-mapped IPv6 in both dotted (`::ffff:1.2.3.4`) and hex
    (`::ffff:0102:0304`) forms unwrapped and re-checked.
- **Timeout:** 5-second AbortController.
- **Response-body cap:** 1 MiB (streamed + aborted on exceed).
- Known limitations documented inline: TOCTOU DNS rebinding is not
  fully mitigated (deployment-layer egress firewall recommended for
  high-value deployments).
- `computeDataVerify` rewired to call `safeFetch()` on every user URL;
  `computeDrandRandomness` also routed through the guard for defence
  in depth even though its URL is hardcoded.

### Workarounds (for unpatched versions)

- Deploy WAVS in a network namespace with egress blocked to RFC 1918,
  CGNAT, link-local (`169.254/16`), and loopback on non-localhost
  interfaces. For containers: `--network` with explicit egress rules.
- For AWS: attach an `IMDSv2-required` instance-metadata policy and
  set a hop-limit of 1 so container workloads cannot reach metadata.

### References

- Commit: <https://github.com/Dragonmonk111/junoclaw/commit/a168608>
- Module: <https://github.com/Dragonmonk111/junoclaw/blob/v0.x.y-security-1/wavs/bridge/src/utils/ssrf-guard.ts>

---

## Publication checklist

When Ffern signs off on `v0.x.y-security-1`:

- [ ] Open each of the five advisory forms at
      <https://github.com/Dragonmonk111/junoclaw/security/advisories/new>
- [ ] Paste title + severity + CWE + CVSS + description from above
- [ ] For each, request a CVE via the checkbox
- [ ] Set affected versions `< v0.x.y-security-1`, patched
      `>= v0.x.y-security-1`
- [ ] Credit Ffern Institute (confirm permission via PM reply first)
- [ ] Publish all five within the same hour
- [ ] Tweet: thread linking the 5 GHSAs + commit SHAs + credit Ffern
- [ ] Medium: publish "Reading the Audit Report" retrospective
- [ ] Telegram `#JunoClaw` / `#Juno`: one-line announcement + link
- [ ] Discord `#governance` / `#dev`: same
- [ ] Commonwealth forum: short post
- [ ] `security@interchain.io`: email with CVE numbers + GHSA URLs
- [ ] BN254 governance proposal goes on-chain (not before)
