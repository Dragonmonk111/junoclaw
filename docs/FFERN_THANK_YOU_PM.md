# Ffern Institute — Thank You & Final Code-Check Request

> **Purpose of this file.** Copy-paste body for the private message / email to the Ffern Institute contact who delivered the JunoClaw audit. Adjust placeholders (`<FFERN-CONTACT-NAME>`, `<DATE>`, branch/commit hashes once tagged) before sending. Tone is gracious, technical-precise, non-defensive — same shape as the cosignature DMs to Jake Hartnell on #373.

---

**Subject:** JunoClaw — Ffern audit acknowledgement + final re-check request before BN254 mainnet ask

Hello `<FFERN-CONTACT-NAME>`,

Thank you, sincerely, for the JunoClaw audit. Five findings, every one of them valid against `main` at the time you poked it; four critical, one high. The audit landed at exactly the right moment — a week before our **BN254 precompile signaling proposal** was due on-chain — and your shape of disclosure (private, ahead of public promotion, with reproducible vectors) was textbook responsible-disclosure. We owe you a debt.

I'm writing for two reasons: to acknowledge what we are doing in response, and to ask you for one more, narrow code-check before we put the BN254 proposal to the validators.

---

## What we are doing in response

A small team — myself (VairagyaNodes) plus Cascade (the pair-programming AI agent that wrote the bulk of the implementation under direction) — has shipped the following on `main` and tagged the result `v0.x.y-security-1`. Per the JunoClaw repo's existing workflow (previous releases such as `feat(mcp): v0.3.0` and `v6: F1-F4 hardening` went directly to `main` with annotated tags), I'm not branching for the security release; the tag itself is the named, attestable artefact. Substantive content first, then the prose:

### 1. C-1 / C-2 — `plugins/plugin-shell/src/lib.rs` (shipped)

- **Cargo feature flag `unsafe-shell`, default OFF.** All execution paths gated at compile time. Default `cargo build` produces a binary with no shell-execution code present.
- **`BLOCKED_PATTERNS` substring blocklist deleted.** Replaced with strict allowlist enforcement.
- **`allowed_commands` becomes the only enforced gate**, default empty (`[]`). Even with the feature compiled in, an operator must explicitly opt in to specific binaries.
- **`run_python` switched from temp-file to `python -E -I -S -B -` via stdin.** No on-disk script artefact. `env_clear()` for blank child environment. Isolated CWD via `tempfile::TempDir` (auto-deleted). Output capped at 1 MiB. `kill_on_drop(true)` so timed-out processes are terminated, not just orphaned.
- **`run_command` shell-wrap (`sh -c` / `cmd /C`) deleted.** Direct executable spawn after `shell-words::split` allowlist check on the first token. Eliminates the shell-injection class entirely.
- **`sandbox_mode: bool` retained as runtime kill-switch** (default `false` when feature compiled in; flipping to `true` halts all execution. Hot-reload via SIGHUP / admin-RPC lands as a separate commit, `v0.x.y-security-2`).

### 2. C-3 — wallet handling at the MCP boundary (two phases, both shipped)

**Phase 1 — passphrase backend.** `mcp/src/wallet/{store,crypto,key-store,cli}.ts`. New `WalletStore` registry encrypts mnemonics at rest under a per-wallet AES-256-GCM envelope; the master DEK is derived from `JUNOCLAW_WALLET_PASSPHRASE` via scrypt (N=2^17, r=8, p=1) with a fresh 32-byte salt. POSIX file mode 0600 on enrolment files. Fresh 12-byte IV per encrypt so two wallets with the same mnemonic produce distinct ciphertexts. New CLI `cosmos-mcp wallet add | list | rm` reads the mnemonic from stdin or a named env var so it never lands as a process argument. 19 tests in `mcp/src/wallet/store-test.ts` (round-trip, tamper detection, wrong-passphrase rejection, path-traversal id rejection, invalid-mnemonic refusal, IV freshness, POSIX 0600 file mode where applicable).

**Phase 2 — keychain backend.** `mcp/src/wallet/keychain-store.ts`. OS credential manager (Windows DPAPI / macOS Keychain Services / Linux libsecret) holds a fresh random 32-byte DEK *per wallet*, indexed by `(service="junoclaw-cosmos-mcp", account=walletId)`. Native binding via the optional `@napi-rs/keyring` dependency; an in-memory `KeyringDriver` test seam gives full mock-keychain coverage. The `WalletStore` was refactored multi-backend: each `.enc` file records `backend: "passphrase" | "keychain"`, the store dispatches per-wallet at decrypt time, and Phase 1 files (no `backend` field) are read as `passphrase` for backward compatibility. The CLI gained `--backend passphrase|keychain` with auto-default policy: `JUNOCLAW_WALLET_DEFAULT_BACKEND` → keychain (no passphrase env var) → passphrase. 21 tests in `mcp/src/wallet/keychain-store-test.ts`, including **two real on-OS keychain round-trips** that pass on this Windows build (DPAPI exercised live; the same code paths exercise Keychain on macOS and libsecret on Linux, skipping gracefully when `@napi-rs/keyring` is absent).

**Schema flip + signing pipeline.** Every MCP write tool's parameter changed from `mnemonic: string` to `wallet_id: string`. The deleted `getSigningClient(chain, mnemonic)` helper in `mcp/src/utils/cosmos-client.ts` is the canonical signal that the unsafe path is gone. The single signing path is now `WalletStore.signFor(walletId, chain)` which decrypts the mnemonic, builds one `SigningCosmWasmClient`, and zeroes the buffer in `finally{}`.

**On-chain proofs (`uni-7`).** Both backends were exercised end-to-end (enrol → DEK → AES-GCM encrypt → broadcast):
- **Passphrase backend:** TX `E2FB05A213D0C65C02EF0D5FAB1C7F8D4AF34BF275B9F0005F0B5A86FF9AED10`
- **Keychain backend (DPAPI):** TX `00D9AC4706A5AB923B7D45E0E97CE166F2EBD34402ECEB113DC7DC9A4702AF18`

### 3. C-4 — `mcp/src/tools/tx-builder.ts` `uploadWasm` (shipped)

- **Path normalised** to absolute via `path.resolve`.
- **Reject if outside `JUNOCLAW_WASM_ROOT`** (defaults to `~/.junoclaw/wasm/`).
- **Symlink reject** via `fs.lstat` + `isSymbolicLink()`.
- **Size capped at 8 MiB.**
- **Magic-byte check:** first 4 bytes must be `\0asm`. Anything else rejected before the bytes leave the function call.

### 4. H-3 — `wavs/bridge/src/local-compute.ts` `computeDataVerify` (shipped)

- **Scheme allowlist:** `http:`, `https:` only.
- **Hostname check via `dns.lookup`:** reject IPv4 in `10/8`, `172.16/12`, `192.168/16`, `169.254/16`, `127/8`, `0.0.0.0`; IPv6 equivalents (`fc00::/7`, `fe80::/10`, `::1`).
- **Port reject** for `22`, `25`, `53`, `135`, `139`, `445`, `3306`, `5432`, `6379`, `9090`, `9200`, `1317`, `26656`, `26657`.
- **`AbortController` 5-second timeout.**
- **Response-body cap at 1 MiB.**

### 5. Documentation (shipped)

- New `SECURITY.md` at the repo root with disclosure policy, supported versions, the four-witness threat model, the verifiable-controllability roadmap, and a contact route. Already pushed.
- New `NOTICE` and `CONTRIBUTORS.md` clarifying the human-direction-of-AI-assistance posture under Apache 2.0 (per the Linux Foundation late-2024 / ASF 2024 guidance on AI-assisted contributions).
- New `docs/FFERN_AUDIT_RESPONSE.md` — public technical companion to the GitHub Security Advisory.

### 6. The walls / lever architectural framing

`SECURITY.md` and `CHANGELOG.md` describe Lock 4 as a two-sided thing. **Walls** prevent the next failure mode by hardening inputs and gating capabilities at compile or load time — that is what `v0.x.y-security-1` ships across all five Ffern findings. **Levers** make a flow already in motion stoppable without taking the system down — that is what `v0.x.y-security-2` will ship as `signing_paused`, `egress_paused`, the published policy-state admin RPC, and SIGHUP / admin-RPC hot-reload. The release naming reflects the substantive distinction: walls and levers solve different problems and shipping them as separate tags lets the audit re-check focus on one architectural primitive at a time.

### 7. Phase 3 named (out of scope for this release, named for the audit response)

`SECURITY.md` includes a *Chain-layer adjacency* section that names `x/authz` integration as the Phase 3 chain-layer Lock 4 primitive: each `wallet_id` becomes an authz delegation from a separate cold key the MCP process never touches, so a compromised MCP process can drain only the authz-delegated message types within the chain-enforced expiry, not the cold key. This composes with the off-chain primitives shipped in `v0.x.y-security-1` and the runtime levers planned for `v0.x.y-security-2`. Full design lands as a separate proposal once `v0.x.y-security-2` is tagged. This is not a `v0.x.y-security-1` deliverable; it is named for transparency and to make the audit-response trajectory legible.

### 8. Disclosure

- GitHub Security Advisory drafted privately at `https://github.com/Dragonmonk111/junoclaw/security/advisories/new`. Will publish concurrent with the security-release tag.
- Cross-posts: Telegram (`#JunoClaw / #Juno`), Discord (`#governance`, `#dev`), Commonwealth forum, Medium audit-response chapter, brief Twitter thread linking GHSA + commit hashes.
- Cosmos-wide notice: `security@interchain.io`.

### 9. The BN254 governance text already names you

In the in-flight HackMD for the BN254 precompile signaling proposal — the text the validators will read before voting — there is now a section titled **"Audit response — Ffern Institute, April 2026"** that publicly acknowledges the audit, lists the categories of findings at high level, states the remediation status, and **commits the project to your re-check as the explicit gate on the upstream CosmWasm PR step.** I have left your specific contact name and any commercially-sensitive detail out of the public version. If you would like to be unnamed entirely until the re-check is complete, say the word and I will redact the institute name and replace it with *"a community-engaged independent reviewer"* until you authorise publication.

The current public draft is at `<HACKMD_URL_TO_BE_INSERTED>` — search for `## Audit response`. The cosignature line lower in the document also names Ffern with the same caveat.

---

## What we are asking of you

We have chosen disclosure **Track B with explicit Ffern verification before BN254 submission** — i.e., the BN254 signaling proposal does **not** go on-chain until you have re-checked the four code-paths above against the security-release commit and given a final go.

When you have a window after the patches land — target: `<DATE — five working days from this message>` — would you do a focused re-audit of:

- `plugins/plugin-shell/src/lib.rs` — full diff against the version you audited.
- `mcp/src/tools/tx-builder.ts`, `mcp/src/utils/cosmos-client.ts`, `mcp/src/index.ts`, plus the new wallet-registry crate (path TBD, likely `crates/junoclaw-wallet-registry/`).
- `wavs/bridge/src/local-compute.ts` — `computeDataVerify` and any neighbour function that takes a URL.
- The new prose files: `SECURITY.md`, `NOTICE`, `CONTRIBUTORS.md`, `docs/FFERN_AUDIT_RESPONSE.md`. Pure prose review, but the public-trust framing matters.

Scope is narrow: roughly **600 LoC of changed code plus four small docs**. We expect this is a half-day pass for you, not a re-audit of the full repo. Anything you would flag as still-not-quite-right, even if not strictly a finding, we will treat as blocking on the BN254 submission.

---

## Practicalities

- **Repo:** <https://github.com/Dragonmonk111/junoclaw>
- **Security release tag:** `v0.x.y-security-1` on `main` at `<MAIN_HEAD_SHA_TO_BE_INSERTED>`. Per the JunoClaw repo's existing workflow, security releases live on `main` and are identified by tag rather than by long-lived release branches. The tag is signed (or annotated, depending on the available key material at tag time).
- **Per-finding commit SHAs:** I will send the individual commit SHAs in a follow-up once the tag is pushed. The commit history splits cleanly: one commit per finding (C-1/C-2 plugin-shell, C-3 wallet registry, C-4 path-guard, H-3 SSRF guard) plus a docs commit (`SECURITY.md`, `CHANGELOG.md`, audit-response prose) and the `CHANGELOG.md` finalisation.
- **Compensation.** The earlier BN254 proposal line was *"$15–25k, 1–2 weeks for an external audit, post-mainnet, via DAO treasury."* After re-reading the realistic scope through your engagement, the proposal's audit-cost section has been revised to **$30–45k, 3–5 weeks**, with separate line items for multi-platform validation, differential-test review, fork-integration, and re-audit. If a Ffern engagement on either the original BN254 audit or this re-check fits your shop's commercial frame, please let me know on what terms — funding is via DAO treasury post-mainnet, per the #373 plan, but that does not preclude bridge funding now.
- **Public credit.** With your permission the BN254 HackMD names Ffern Institute in two places (the *Audit response* section and the cosignature). If you would prefer the credit deferred until you have signed off on the fixes, I will redact and re-publish; tell me what you want.
- **PGP / signed messages.** Happy to switch to encrypted comms if you would like; my key fingerprint is `<TO_BE_INSERTED>` and you can find me on `keys.openpgp.org`. Until then this PM is the channel.

---

Whatever you can do, on whatever timeline — thank you again. The audit changed the shape of this week in the best possible way. The BN254 vote will be a stronger proposal because of it, and the broader project a more honest one.

— **VairagyaNodes** (with **Cascade**, the pair-programming AI agent that did the implementation work under direction)

JunoClaw maintainer · Juno staker since December 2021 · Validator candidate (unbonded)
