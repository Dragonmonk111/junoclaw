# When the Abbot Came

## How an independent audit landed on the right week, changed the shape of a single-operator project, and taught a small coastal monastery to install bells it can ring from a single rope.

> **Published on Medium (27 April 2026):** <https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1>
>
> *This file is the source-of-truth archival copy, including the full eleven-prompt Midjourney appendix (omitted from the Medium version for narrative flow). Medium readers who want the prompts are pointed here from the closing colophon of the Medium piece.*

[IMAGE 1 — THE ABBOT AT THE GATE]

---

> *A note on words. This piece is a narrative summary of a response to an external security audit, written by the authors of the code after the patches were merged, the release was tagged, and the public security advisories were posted. Every claim below is grounded in commits, tagged releases, and GHSA IDs that anyone can query. It is not legal, financial, or investment advice, and it is not a substitute for the formal third-party reviews that remain on the road ahead.*

---

In the old monastic houses along this coast, a hall did not know itself until another hall's abbot walked its walls. A visiting abbot did not come to humiliate the brethren. They came to *see* — to press a gloved hand against the weak place in the drystone wall that the resident monks had stopped noticing because they passed it every day. The visitation was a discipline, not a judgement. It was also a kindness: the house that was walked was the house that was made honest.

A piece of software is no different. It grows around its author until the author cannot see it clearly. An external audit is the visiting abbot — someone whose eye has not learned to slide past the weak place.

**Ffern Institute** walked JunoClaw's walls in April 2026. They found five weak places. This is the record of what was found, what was repaired, and the small bells that were installed afterwards so that every wall can be listened to from a single length of rope at the abbot's own window.

It is also a thank-you note. The visit was a kindness. The response, insofar as we can claim any of it, tried to match that kindness in grace and speed.

---

## Where we started: a house used to watching itself

[IMAGE 2 — THE HOUSE BEFORE THE VISITATION]

Before the visit, JunoClaw was three things in one timber hall. On the chain-layer, the contracts that had passed [Proposal #373](https://ping.pub/juno/gov/373) — audited enough to have drawn 91.71 % YES on 59.56 % turnout, ratified as Juno ecosystem infrastructure. On the mathematics-layer, a working Groth16 zk-verifier at **code_id 64**, the first on a pure-CosmWasm chain. And around them, a smaller and more crowded room — the *operator-side* helpers, the little workshop-tools a single operator needs to run the stack: a shell-plugin for developer automation, an MCP server that speaks to any AI client, a WAVS bridge that fetches data for attestations.

The chain-layer had been walked many times. The mathematics-layer was narrow, test-covered, and had earned the care of a dedicated artefact index. The workshop had not. It was the place a single builder used daily and so the place least looked at — the drawer of chisels that had been sharpened by hand and returned to the same box for months without anyone asking whether a chisel had quietly slipped through the bottom of the drawer.

This is the shape a single-operator project tends to take. The chain-layer is seen by voters; the workshop is seen only by the person who opens it at 6 a.m. The visitation was asked to walk the workshop.

---

## The visitation itself

[IMAGE 3 — THE READING IN THE CHAPTER-HOUSE]

**Ffern Institute** arrived with a scope that was narrow and deliberate. Not the on-chain contracts, whose last internal hardening pass (*v6 — the beating of the bounds*) had closed four regressions (F1–F4) and whose audit record is the public on-chain transaction log. Not the BN254 precompile crate, which sits at `wasmvm-fork/cosmwasm-crypto-bn254/` with 22/22 tests and is the subject of its own separate governance track.

Ffern walked the *operator-side* code:

- `plugins/plugin-shell/` — the developer-facing shell plugin
- `mcp/` — the Model Context Protocol server that exposes Cosmos-chain tools to any AI client
- `wavs/bridge/` — the bridge daemon that polls WAVS aggregators and submits attestation transactions

Four criticals and one high. Five findings. One report. Private disclosure first, with reproducible vectors, ahead of any public step. The shape of disclosure was itself a kindness — a slow, private letter, not a public shout.

The response began the same day the letter arrived.

[IMAGE 4 — THE FIVE MARKS ON THE PLAN]

---

## The five cracks in the wall

Each finding deserves its own paragraph, because each of them rhymes with a different class of assumption a single-operator project tends to inherit without noticing. They are presented in the order Ffern listed them, with the audit label, the plain-English crack, the patch, and the live Security Advisory.

### Crack one and two — the shell-gate (C-1, C-2)

`plugin-shell` was a developer helper. It shelled out to the OS for build, for scaffold, for `cargo test`, for the hundred little commands the builder runs in a day. It had an `allowed_commands` list and a blocklist of *obviously* dangerous substrings (`rm`, `curl`, etc.). Both were trivially bypassable.

The `allowed_commands` list was declared in configuration but not actually *enforced* at the call-site — the code path that mattered only consulted the substring blocklist. And that blocklist was substring-matched against the command string without accounting for command composition. `echo "safe"; rm -rf ~` was a single string. The blocklist found `echo`. It did not find the semicolon, the `rm`, the `-rf`, the home-directory token that followed.

The crack was not "shell execution exists in the project" — that is a reasonable developer tool for a developer's own machine. The crack was *"there was a fence that looked like a fence and was not one."* That is a different class of wrong.

The patch is a Cargo feature gate. Shell execution only compiles in under `--features unsafe-shell`; without the feature the unsafe code path is not present in the binary at all. Compile-time gating is the right primitive here because it survives configuration mistakes, env-var overrides, operator fatigue at 6 a.m., and the general human capacity to convince ourselves at build time that a dangerous thing is safe. The code that is not in the binary is the code that cannot be exploited.

- **C-1** — [`GHSA-fvq5-79h6-952c`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-fvq5-79h6-952c) — `plugin-shell` shell-injection bypass via substring blocklist (CVSS 8.4 high)
- **C-2** — [`GHSA-gpvm-3chf-2649`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-gpvm-3chf-2649) — `plugin-shell` shell-metacharacter injection via shell wrapper (CVSS 8.4 high)

### Crack three — the mnemonic carried across the hall (C-3)

This was the gravest. The MCP server exposed wallet tools. To sign a Cosmos transaction the tool needs access to a key. The early design accepted a plaintext **BIP-39 mnemonic** as a tool-call parameter — the twelve or twenty-four words that *are*, in cryptographic terms, the wallet itself.

A mnemonic passed as a tool-call parameter is a mnemonic that crosses every boundary the MCP protocol crosses: serialisation, inter-process-communication, tool-invocation logging surfaces in the client, local and remote audit trails. The crack was not that any one of those boundaries is malicious — it was that *twelve words of the most sensitive material a chain can know* should never walk across them. A reliquary is not a parameter; it is a place.

The patch is a **wallet handle registry**. A mnemonic is loaded once at startup into a passphrase-encrypted store (or into the OS keychain, on platforms that offer one). Write-tools no longer receive a mnemonic. They receive a *handle* — an opaque stable reference to *which* wallet to use — and the signing call is resolved internally, in a context the mnemonic never leaves. The mnemonic itself is held in a sealed reliquary behind the chapel altar; the handle is the bell-pull at the altar rail.

- **C-3** — [`GHSA-j75q-8xvm-6c48`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-j75q-8xvm-6c48) — MCP write tools exposed raw BIP-39 mnemonic as a tool-call parameter (CVSS **9.8 critical**)

This finding's severity was correctly raised from the original report's 8.8 to **9.8** after re-scoring the CVSS vector with `UI:N` (no user interaction required) — because once the tool is wired to an AI client, the client itself will invoke the tool without human confirmation. The mnemonic crosses the boundary under purely machine action. That is the correct reading of the vulnerability and it is why the severity rating matters: a 9.8 reads to downstream operators as "stop deploying this version today."

### Crack four — the upload-gate that admitted any cart (C-4)

`upload_wasm` was a developer helper for shipping a freshly-compiled wasm blob to the chain. It accepted a filesystem path and read the bytes. It did not enforce an allow-root (a declared safe directory from which uploads must originate). It did not check for symlinks (a symlink inside a safe directory can point anywhere on disk). It did not cap the size. It did not verify the magic bytes that mark a real wasm module (`\x00asm`). A sleepy operator, or a malicious input that reached the tool via another helper, could hand the gate a path and the gate would read the bytes and try to ship them.

The patch is a path guard at the gate: canonicalise the path, reject anything outside the allow-root, resolve symlinks and re-check, cap the size at a sensible limit, and read the first four bytes to confirm the magic. The gate is still open to good-faith uploads; the cart now has to show its face before the gate-keeper raises the bar.

- **C-4** — [`GHSA-rw59-34hw-pmwp`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-rw59-34hw-pmwp) — `upload_wasm` accepted arbitrary filesystem paths without validation (CVSS 8.5 high)

### Crack five — the messenger who could be sent to the wrong road (H-3)

The WAVS bridge ran a helper called `computeDataVerify` that fetched a URL as part of a data-verification workflow. It fetched any URL. No scheme filter. No block on private IP ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `127.0.0.0/8`, link-local, IPv6 ULA). No port restriction. This is the classical Server-Side Request Forgery (SSRF) shape: if an attacker can influence the URL at all, the messenger can be sent riding down the bishop's own road — to the cloud metadata service at `169.254.169.254`, to an internal service on the operator's own network, to anywhere the operator's machine can reach that is not meant to be reached from outside.

The patch is a small guard with a boring name: parse the URL, allow-list the scheme (`https`, and optionally `http` only under an explicit flag), resolve the host, reject any address that falls in the private-network blocks or equals any loopback / link-local / metadata sentinel, cap the port to standard HTTPS/HTTP, bound the request size, set a short timeout. The messenger now shows the gatekeeper the road it intends to ride *before* leaving the yard.

- **H-3** — [`GHSA-q545-mvjf-q9pg`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-q545-mvjf-q9pg) — SSRF in WAVS `computeDataVerify` allows cloud-metadata and internal-service access (CVSS 8.2 high)

[IMAGE 5 — THE FIVE MASONS AT WORK]

---

## The walls — `v0.x.y-security-1`

[IMAGE 6 — THE FIVE STONES SET INTO THE WALL]

The walls are the patches that close the cracks. They were grouped into a single security release, tagged on `main` as **`v0.x.y-security-1`**, so that any downstream operator can take one tag and know they have all five fixes simultaneously. There is no state between them — you are either on the pre-tag commit and exposed, or on the tag and sealed.

Each wall is a small thing; the discipline is in the *kind* of primitive chosen for each.

- **Walls one and two — the shell-gate.** A single Cargo `unsafe-shell` feature gate, off by default, gates both unsafe entry points (`C-1` shell-injection bypass and `C-2` shell-metacharacter injection). The binary built without the feature *does not contain* the unsafe code paths. Configuration cannot re-enable them. An env-var cannot re-enable them. A tired operator cannot re-enable them. Only a deliberate build-time choice can, and that choice leaves a record in the binary's feature manifest that anyone can inspect.

- **Wall three — the mnemonic reliquary.** A `WalletStore` with two backends: an encrypted-file backend using a passphrase-derived key, and an OS-keychain backend on platforms that offer one. The mnemonic never leaves the process after startup. Write-tools take handle strings (`"default"`, `"cold"`, `"hot-reserve"`) and the signing path resolves them internally. The old mnemonic-as-parameter API was *deleted* — not deprecated, deleted — so that no future caller can accidentally reach back into the vulnerable shape.

- **Wall four — the upload-gate.** A `UploadGuard` that canonicalises, checks the allow-root, re-resolves symlinks, caps the size, and verifies the magic bytes. The error messages are deliberately terse, because an upload-guard that explains *why* it rejected a path becomes an oracle for brute-forcing structural information about the operator's filesystem.

- **Wall five — the egress-gate for the WAVS bridge's data fetcher.** A small SSRF-safe wrapper around every outbound HTTP call in `computeDataVerify`: scheme allow-list, host-resolution with private/loopback/link-local/metadata blocks, port cap, response-size cap, short timeout. The messenger still rides — but only on roads whose mile-markers the gatekeeper recognises.

Alongside the five walls, the first release also ships a **startup-only `sandbox_mode` switch** on `plugin-shell`: an env-var-armed hard gate that refuses to load the plugin's unsafe code paths at all, even if the binary was compiled with the feature enabled. Two independent gates — compile-time Cargo feature, runtime sandbox env-var — so that a single misstep at either level cannot reach the unsafe surface.

Taken together these are *the five walls of Lock 4* — Lock 4 being the *operator-side* companion to Lock 1 (TEE hardware root) and Lock 2 (BN254 mathematics) in the JunoClaw architecture. Lock 4 had been sketched before the audit. Ffern's findings were the missing detail that turned sketch into built stone.

---

## The bells — `v0.x.y-security-2` and `v0.x.y-security-3`

[IMAGE 7 — THREE BELLS RAISED INTO THE TOWER]

A wall is a *preventive* primitive: it stops something from happening. A **bell** is a *responsive* primitive: it gives a human the ability to make something stop, after a wall has been breached or a worse finding has appeared or a political moment demands caution.

The three bells were shipped in two further release tags on the same `main` branch, deliberately not bundled with the walls — because the bells introduce a new listener on a signing-sensitive process and deserved their own review window.

- **The first bell — `signing_paused`** (`v0.x.y-security-2`). An env-var-armed boolean gate on `WalletStore.signFor()`. When set, every signing attempt raises `SigningPausedError` before any key material is touched. The bell can be rung by an operator who wants every transaction across the entire agent fleet to halt — for incident response, for governance, for a one-hour pause during a controversial update. No redeploy, no restart with the default backend, no waiting on consensus. One env var, one bounce of the process, and every agent is silent.

- **The second bell — `egress_paused`** (`v0.x.y-security-3`, Phase 3a). The same primitive, applied to outbound network access in the WAVS bridge's SSRF-guarded fetcher. When the second bell is rung, no attestation-fetch leaves the yard. This is the bell you ring when an incident has been reported on a data-source you depend on, when an operator wants to confirm nothing is being exfiltrated, when the network around you has become untrustworthy for a quarter of an hour.

- **The third bell and the abbot's window — the admin RPC** (`v0.x.y-security-3`, Phases 3b through 3d). A **localhost-only, token-gated** HTTP listener on both processes (MCP and WAVS bridge). Zero third-party dependencies. Constant-time token comparison, so that a timing side-channel does not leak the token length. Rate-limited against brute-force. Audit-logged, so that every admin action leaves a trail. DNS-rebinding-resistant, because the listener binds to `127.0.0.1` and verifies the `Host` header. Three endpoints: `/sign/pause`, `/sign/resume` (and the egress equivalents), and the **`/policy` read-only roll-up** that exposes the current value of every kill-switch and allowlist.

The `/policy` endpoint is the quiet one and the important one. It means that any downstream verifier — a client about to send a task, a governance watchdog, a fellow operator in a mesh — can poll the admin RPC and *know* which bells are rung on this operator's machine right now, without trusting anything the operator's agent says. The bells are hot-flippable from a single `curl`; the bells' state is pollable by anyone with a token. Verifiable controllability, operationally.

This is the architectural step the [Ffern section of the BN254 HackMD](./HACKMD_BN254_PROPOSAL.md) calls *the third lock*, and it is what separates *autonomy* from *abdication* in an agent-company running real-world consequential work. A wall says "you cannot do that". A bell says "I can make you stop doing that right now". The third lock is the bell in the tower whose state is public.

---

## The notice on the rood-screen

[IMAGE 8 — FIVE NOTICES NAILED TO THE CHURCH DOOR]

A patch is a private act between the code and its author. A **disclosure** is a public act between the project and every operator who depends on it.

Two opposite mistakes are easy here. The first is to write nothing — to ship the patches and stay silent, on the theory that the bug is fixed and the operators will pull the new tag eventually. The second is to write *everything* — to publish a long technical narrative with proof-of-concept exploits, screenshots of the vulnerable shell, and the full reproducer for each finding. The first mistake leaves downstream operators in the dark, unable to triage urgency. The second mistake hands a fully-armed weapon to anyone running a pre-tag deployment.

The discipline we chose is a third path, and it has a name in the security community: **minimal disclosure now, retrospective later.**

Five GitHub Security Advisories were drafted privately first, then published on the repo on **26 April 2026**, each one carrying *only* the smallest text needed for a downstream defender to act:

- The class of vulnerability and the exact CWE label.
- The CVSS vector and severity.
- The vulnerable version range, and the patched version range, expressed as commit SHAs and tags.
- A pointer back to `CHANGELOG.md` at `v0.x.y-security-1` for the patch context.
- A pointer to *this article* — the post-audit retrospective — as the place where the full story lives.

What the advisories deliberately do **not** contain: exploit reproduction steps, the input strings that triggered each crack, the line-and-column locations of the original vulnerable code, the configuration that maximally exposes each finding. None of that is information the downstream defender needs to *act*; all of it is information an attacker would need to *attack* an operator who is one tag behind.

This is the **Alt-4** disclosure choice in the project's own internal shorthand: *minimal advisories now, narrative later, narrative pointer baked into the advisories so it becomes findable when it lands.* The pointer is a small thing — three lines in each GHSA — but it is the line of trust between a private fix and a public conversation. It says, in effect: *we are not hiding from you; we are pacing the disclosure so that the operator who is one week behind us is not handed to the first attacker who reads our advisory feed.*

CVE numbers are being assigned by GitHub's CNA at the time of writing, asynchronously — they will appear on each advisory page when minted, without anyone needing to click anything. The five advisories, with their current severities:

- **C-1** — `GHSA-fvq5-79h6-952c` — `plugin-shell` shell-injection bypass — **CVSS 8.4 high**
- **C-2** — `GHSA-gpvm-3chf-2649` — `plugin-shell` shell-metacharacter injection — **CVSS 8.4 high**
- **C-3** — `GHSA-j75q-8xvm-6c48` — MCP mnemonic exposure as tool parameter — **CVSS 9.8 critical**
- **C-4** — `GHSA-rw59-34hw-pmwp` — `upload_wasm` arbitrary filesystem path — **CVSS 8.5 high**
- **H-3** — `GHSA-q545-mvjf-q9pg` — SSRF in WAVS `computeDataVerify` — **CVSS 8.2 high**

The five notices are nailed to the church door. The longer story — *this story* — is the codex on the lectern inside, available to anyone who walks in. Both are needed; neither is the other.

---

## The shape of it now

[IMAGE 9 — THE MAPPA MUNDI WITH THE FOURTH LOCK MARKED]

Here is where the walls and the bells sit in the architecture, after `v0.x.y-security-3` was tagged on `main` and the five GHSAs were published.

```
┌──────────────────────────────────────────────────────────────────────┐
│  LAYER 7 — The Council (Governance)                                  │
│  13-bud trust-tree DAO (agent-company), 67% supermajority for       │
│  constitutional changes, adaptive voting deadlines                  │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 6 — The Skald's Tongue (Cosmos MCP Server)                   │
│  Wallet-handle registry (post-C-3); admin RPC with /policy          │
│  Hot-flip on signing_paused; pollable kill-switch state              │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 5 — The Ship-Paths (Scaling)                                 │
│  IBC · Mesh Security · Celestia + tiablob                            │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 4 — Lock 4: The Operator Walls and Bells (NEW)               │
│  Walls: unsafe-shell Cargo gate · WalletStore · UploadGuard ·       │
│         SSRF-safe fetcher · sandbox_mode startup gate                │
│  Bells: signing_paused · egress_paused · admin RPC                   │
│  /policy roll-up endpoint — verifiable controllability               │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 4' — The Watchtowers (Off-chain Compute)                     │
│  WAVS operator network (Akash deploy + SGX + sidecars)               │
│  WASI components, deterministic, reproducible                        │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 3 — The Ten Halls (Application Contracts)                    │
│  agent-registry · task-ledger · escrow · agent-company              │
│  junoswap-factory · junoswap-pair · faucet                          │
│  builder-grant · zk-verifier · junoclaw-common                      │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 2 — The Illuminated Page (Math Layer)                        │
│  Groth16 BN254 verifier (zk-verifier, Code ID 64)                   │
│  Measured 371,486 gas pure-Wasm; precompile track separate          │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 1 — The Keep (Hardware Root)                                 │
│  Intel SGX / AMD SEV-SNP enclaves, WAVS-in-TEE                      │
│  Proposal 4 TEE-attested tx 6EA1AE79…D26B22                          │
└──────────────────────────────────────────────────────────────────────┘
```

The new layer is **Layer 4 — Lock 4**. Where it used to be a sketch, it is now built stone, with bells in the tower above it. The previous Layer 4 (off-chain compute) is renamed Layer 4' to avoid confusion; in the original "Three Ordeals" diagram the two were collapsed into one, but the post-audit reality is that the operator-side primitives deserve their own line because they are now a coherent, named, externally-verifiable layer.

Reading the stack bottom-up: trust-this-chip, trust-this-math, trust-this-audited-contract, trust-this-deterministic-WASI, trust-this-locked-operator-runtime, trust-this-protocol-between-chains, trust-this-MCP, trust-this-13-bud-supermajority. The Ffern audit added the *trust-this-locked-operator-runtime* layer. It was always implied; now it is explicit, named, and pollable.

Every layer is replaceable. Every layer is checkable. Every layer carries its own bells.

---

## The facts on the ground

None of what follows is a promise. These are repository artefacts that anyone can verify today.

**Tags on `main`** (`github.com/Dragonmonk111/junoclaw`):

| Tag | What it shipped |
|---|---|
| `v0.x.y-security-1` | The five walls (C-1, C-2, C-3, C-4, H-3) plus startup-only `sandbox_mode` |
| `v0.x.y-security-2` | The first bell — `signing_paused` runtime kill-switch |
| `v0.x.y-security-3` | The remaining bells — `egress_paused` (3a), admin RPC on MCP (3b), `/policy` roll-up (3c), admin RPC on WAVS bridge (3d) |

**The five published advisories**, all `state=published` on the repo Security tab as of 26 April 2026 23:11 UTC. CVE numbers are being assigned by GitHub's CNA asynchronously and will populate without further action.

| Finding | GHSA | CVSS |
|---|---|---:|
| C-1 plugin-shell substring-bypass | [`GHSA-fvq5-79h6-952c`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-fvq5-79h6-952c) | 8.4 high |
| C-2 plugin-shell metacharacter | [`GHSA-gpvm-3chf-2649`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-gpvm-3chf-2649) | 8.4 high |
| C-3 MCP mnemonic-as-parameter | [`GHSA-j75q-8xvm-6c48`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-j75q-8xvm-6c48) | **9.8 critical** |
| C-4 upload_wasm path traversal | [`GHSA-rw59-34hw-pmwp`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-rw59-34hw-pmwp) | 8.5 high |
| H-3 WAVS computeDataVerify SSRF | [`GHSA-q545-mvjf-q9pg`](https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-q545-mvjf-q9pg) | 8.2 high |

**The on-chain context.** None of the findings touched on-chain code. The contracts ratified under Proposal #373, the live `agent-company`, the `zk-verifier` at `juno1ydxksvr…lse7ekem` (code_id 64), the BN254 precompile crate at `wasmvm-fork/cosmwasm-crypto-bn254/` — none of these were affected. The audit was deliberately scoped to operator-side helpers, and the operator-side helpers were where the cracks lived.

**The licence.** Apache 2.0, top to bottom, including every upstream dependency.

---

## The moral of the visitation

[IMAGE 10 — THE LEDGER ENTRY AT DUSK]

Three lessons, each from a different angle.

- **The visit is a kindness.** A single-operator project is a project whose author has stopped seeing parts of it. An external audit is not an attack on the work; it is the only practical way to recover the eyes that the work has slowly displaced. The right response to a finding is *gratitude plus speed* — gratitude for the seeing, speed because every day a known finding is unpatched is a day the operator's downstream is exposed.

- **Minimal disclosure is a discipline, not an accident.** The choice to ship five short advisories with explicit retrospective-pointers, rather than five long advisories with reproducers, is a *form* of caring for the downstream operator who is one week behind. It is not the only valid choice — projects with public bug-bounty programmes and immediate-patch infrastructure can responsibly publish more — but for a single-operator project it is the choice that protects the long tail of unpatched deployments without leaving the patched ones uninformed.

- **Verifiable controllability is the third lock.** The TEE proves *where* a computation ran. The ZK proof proves *what* a computation computed. Lock 4 — the walls and bells — proves *that the operator retains externally-verifiable control of the agent fleet at runtime*. This is the lock most discussions of agentic AI safety leave implicit. It deserves to be explicit, named, and pollable. After Ffern, it is.

And the visitation was passed by *surrender* to a discipline already invented by someone else:

- **Ffern Institute** — for the visitation itself, conducted with the right shape (private first, reproducible vectors, ahead of any public step) and the right grace (clear classification, generous time-to-respond, no posturing).

- **The MITRE CWE catalogue and the CVSS specification** — for the shared vocabulary that lets a single line in an advisory carry the right urgency to a stranger reading it on the other side of the world.

- **The GitHub Security Advisory + CVE Numbering Authority infrastructure** — for the machinery that turns "we found a thing" into "every downstream defender's automated tooling now sees a thing".

- **The three security-engineering authors whose books taught the disciplines used here** — Brendan Burns and Eddie Villalba on `unsafe-` Cargo gates as a defence-in-depth pattern, the OWASP SSRF cheat-sheet authors on the host-resolution dance, and the long line of cryptographic-key-handling literature from PGP onwards on the *handle-not-secret* pattern that informed the wallet registry.

We did not invent the discipline of external audit. We did not invent the SSRF guard or the Cargo feature gate or the wallet-handle pattern or the localhost-only admin RPC. We connected them, in response to a careful letter from a careful house.

For that, **お辞儀をします** — we bow. Or, in the tongue closer to these hills: *we lower our rod at the fence-line*.

[IMAGE 11 — THE ABBOT DEPARTING AT THE HARBOUR]

---

## What this article is, and what it is not

What it is:

- A narrative summary of an external operator-side security audit and the project's response, written by the authors after the patches were merged, three security releases were tagged on `main`, and five Security Advisories were published.
- A map of which finding closed which crack with which primitive, and which release tag carries which patch.
- A legally cautious framing of the disclosure choice: minimal advisories now, retrospective later, the retrospective being this document.

What it is not:

- Legal, financial, or investment advice.
- A claim that this stack is now free of undiscovered bugs. It is not. No stack is. There may be a sixth wall to find; if so, please find it and let us know.
- A substitute for the formal third-party reviews that remain on the road ahead — the Ffern re-audit of the operator-side fixes, and the precompile-side audit of the BN254 crate before any `MsgSoftwareUpgrade` proposal carrying it reaches mainnet.

If you find a sixth crack, the [project's `SECURITY.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/SECURITY.md) tells you how to report it privately. If you spot a wrong CWE label, a misjudged CVSS score, or an unclear sentence in any of the five advisories, please open an issue or a pull request — the advisories are PATCH-able from any field at any time, and a wrong label fixed quickly is worth more than a polite silence.

---

## For the curious: where to look

| What | Where |
|---|---|
| The five walls patch-set, prose | [`CHANGELOG.md`](../CHANGELOG.md) at `v0.x.y-security-1` |
| The runtime levers (bells), prose | [`SECURITY.md`](../SECURITY.md) §*Levers* |
| The release-by-release roadmap | [`SECURITY.md`](../SECURITY.md) §*Roadmap* |
| The release notes for the bells | [`docs/RELEASE_NOTES_v0.x.y-security-2.md`](./RELEASE_NOTES_v0.x.y-security-2.md), [`docs/RELEASE_NOTES_v0.x.y-security-3.md`](./RELEASE_NOTES_v0.x.y-security-3.md) |
| The five published Security Advisories | <https://github.com/Dragonmonk111/junoclaw/security/advisories> |
| The original "Three Ordeals" article (context) | [`docs/MEDIUM_ARTICLE_THREE_ORDEALS.md`](./MEDIUM_ARTICLE_THREE_ORDEALS.md) |
| The BN254 precompile track (separate) | [`docs/BN254_PRECOMPILE_INDEX.md`](./BN254_PRECOMPILE_INDEX.md) |
| The Ffern thank-you note | [`docs/FFERN_NOTE.md`](./FFERN_NOTE.md) |
| The code itself | <https://github.com/Dragonmonk111/junoclaw> |

Apache 2.0. Open issues welcome. The walls are built; the bells are hung; the abbot's note has been answered. The next visitation, when it comes, will be welcome.

---

*Built on JunoClaw. Five walls, three bells, one notice on the rood-screen, one abbot's letter answered with grace and speed insofar as we could manage either.*

---

## Midjourney Prompts

All prompts use: `--ar 16:9 --s 250 --v 6.1`

**Shared style suffix (append to the end of every prompt):**
`2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, Howard Pyle Norse illustration and Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre umber iron-red moss-green woad-blue heather-purple slate-grey and warm parchment cream, coastal Anglo-Saxon and early-medieval Welsh countryside, sea-mist rolling off chalk and flint cliffs, low green hills with old drystone field boundaries and lichened standing stones, occasional ravens aloft and sheep grazing below, aged and slightly smoke-stained page corners, small knotwork border motifs in at least one corner of the plate, gentle diffused storm-light`

> **Shared cast / world-notes** (additions for the Visitation cycle, building on the Three Ordeals cast): a **visiting abbot** in a slightly darker undyed-brown wool habit with a wide leather belt, carrying a small leather satchel and a folded vellum letter sealed with red wax; a **resident prior** of the local house in a paler undyed habit, slightly younger, with ink-stained fingers; a **village stonemason** in a leather apron with a small wooden mallet and an iron chisel hung at the belt; a **bell-founder** in a soot-stained tunic with a small lifting-ring of plaited rope. The familiar cast remains welcome (Welsh shepherd with crooked ashplant and lean hound; Anglo-Saxon monk-scribe; Viking-era seafarer at the longship's prow; raven on a standing stone). Wooden or carved-stone signboards in the plate are welcome — rendered in simple Anglo-Saxon runes or in uneven scratched Insular capitals; never modern lettering.

---

**Prompt 1 — The Abbot at the Gate (Hero):**
`A coastal early-medieval English monastery seen at the hour just after dawn from a low rise in the path, a small turf-and-stone chapter-house and round-towered chapel inside a low drystone enclosure, the sea visible to the right beyond chalk cliffs with one beacon-fire on a far headland, a visiting abbot in a darker brown wool habit with a wide leather belt approaching the lichened wooden gate of the monastery on foot — leather satchel at his hip, a folded vellum letter sealed with red wax held lightly in his left hand, his right hand raised in a small acknowledging gesture toward an unseen gatekeeper — a resident prior in a paler undyed habit just stepping out of the gatehouse to receive him with a deferential nod, a raven settling on the gatepost above them, a small flock of sheep grazing on the slope outside the wall, sea-mist curling around the cliff-base, the cobbled path between them strewn with a few wind-blown leaves, a small carved wooden sign nailed to the gatepost reading "ᚠᛖᚱᚾ" (Ffern) in worn Anglo-Saxon runes, atmosphere of arrival, courtesy, a long-anticipated visit at last, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, Howard Pyle Norse illustration and Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of dawn-rose ochre moss-green slate-grey woad-blue and warm parchment cream, knotwork border in the upper-left corner, aged page corners --ar 16:9 --s 250 --v 6.1`

**Prompt 2 — The House Before the Visitation:**
`A wide cutaway view of a small early-medieval English coastal monastery shown as if drawn on a single saga-plate, three buildings of the enclosure rendered with their roofs cut away to show their interiors as in a Hereford-Mappa-Mundi-style illustration — at the back the great timber chapter-hall full of small carved monks at long benches reading from vellum codices (representing the on-chain layer, well-watched and many-eyed), in the middle a stone scriptorium where a single hooded scribe is illuminating an ornate elliptic-curve initial in gold-leaf ink across two facing pages (representing the mathematics layer, narrow and beautiful), in the foreground a small leaning timber workshop with one open shutter where a single tired craftsman is bent over a workbench of chisels, gouges, awls, and a half-finished wooden tool, a coil of rope in the corner, an open drawer with one chisel quietly slipped to the bottom of the box (representing the operator-side helpers, used daily and least watched), a small flock of sheep grazing in the yard between the three buildings, a Welsh shepherd leaning on a crooked ashplant watching from a corner with a slightly thoughtful expression, a raven aloft above the workshop's chimney, sea visible behind the chapter-hall through a gap in the wall, atmosphere of normal daily work and a slight foreboding, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre umber moss-green slate-grey and warm parchment cream, the workshop slightly more brightly lit than the other two buildings to draw the eye, knotwork border running the full bottom edge of the plate --ar 16:9 --s 250 --v 6.1`

**Prompt 3 — The Reading in the Chapter-House:**
`The interior of a small early-medieval timber-and-stone chapter-house at mid-morning, a long oak table down the centre with a single shaft of light falling through a high narrow window, the visiting abbot in a darker brown wool habit standing at the head of the table reading aloud from a folded vellum letter held in both hands, the resident prior seated at the abbot's right in a paler undyed habit listening with hands folded, three other monks of the resident house seated along the benches with quills and small wax tablets, a single carved wooden cross on the wall behind the abbot, a small fire burning low in a stone hearth at the side, the abbot's leather satchel set on the bench beside him with the broken red wax seal of the letter just visible upon it, the prior's expression quietly attentive — neither defensive nor stricken, more like a man hearing a list of small things he half-suspected, a raven perched outside on the window-ledge looking in, faint motes of dust caught in the shaft of window-light, a small Insular knotwork panel illuminated on the chapter-house wall behind the prior in iron-red and ochre, atmosphere of careful private hearing, the kindness of a properly-shaped letter being read in the proper place, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of warm ochre lamp-amber slate-grey moss-green and warm parchment cream, knotwork border in the lower-right corner, aged page corners --ar 16:9 --s 250 --v 6.1`

**Prompt 4 — The Five Marks on the Plan:**
`A close framed view of a small wooden table at the side of the chapter-house with an unrolled hand-drawn vellum plan of the monastery enclosure laid flat across it, on the plan the abbot's hand has already placed five small dabs of red ochre at five different points along the outline of the workshop's outer wall — each dab numbered in tiny Insular capitals "I" "II" "III" "IV" "V" — the abbot's hand still holding the small ochre-stick paused above the fifth mark, the resident prior leaning over the plan with one finger pointing thoughtfully at the third mark, his face partly in shadow, an inkpot, a small horn-flask of red ochre, two quills, and a folded copy of the letter on the table beside the plan, a single carved wooden ruler measuring the wall's length, a beam of light from the window catching the gold-leaf ornament of the plan's compass-rose in the upper-left corner, a small folded list lying beside the plan headed "ᛞᛁᛋᚳᛟᚻᚹᛖᚱᛁᛖᛋ" (discoveries) in worn runes with five sub-headings beneath, no other figures in the plate to keep the focus on the marks themselves, atmosphere of careful enumeration, the slow honest moment between hearing and beginning to repair, 2D hand-drawn illustration in the style of an Insular illuminated manuscript, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of warm parchment cream iron-red ochre moss-green and slate-grey, selective gold-leaf accent only on the compass-rose, small knotwork panels around two corners of the plan --ar 16:9 --s 250 --v 6.1`

**Prompt 5 — The Five Masons at Work:**
`A wide outdoor scene at the section of the workshop's outer wall where the five marks were placed, mid-morning sun, five different craftsmen working simultaneously along the wall each at their own marked breach — the first mason chiselling a fresh stone to fit a low gap (a wooden sign stuck in the turf beside him reading "I·II — ᚷᚪᛏᛖ" for "gate"), the second a tall woman in a leather apron carefully setting an ornate iron-banded reliquary-box into a deeper square hollow lined with woollen cloth (a sign reading "III — ᚱᛖᛚᛁᚳ" for "relic"), the third craftsman fitting a heavy oak crossbar with iron studs into a low wicket gate (a sign reading "IV — ᚳᚪᚱᛏ" for "cart"), the fourth a smaller older mason with a chalk-stick marking a careful series of permitted boundary-lines on the wall around a postern gate (a sign reading "V — ᚱᚩᚪᛞ" for "road"), and a fifth craftsman a few paces back stretching a heavy braided rope between two fence-posts as a final symbolic boundary (a sign reading "·ᛋᚪᚾᛞᛒᚩᚳᛋ·" for "sandbox"), all five in undyed wool tunics and leather aprons, two of the craftsmen sharing a quiet practical word, the resident prior walking slowly along the wall observing each repair in turn with hands folded behind his back, a sheepdog dozing in a pool of sun at the wall's foot, a raven on a standing stone watching, sea visible in the distance, drystone wall snaking off into the middle ground with one or two earlier repairs already mortared, atmosphere of competent quiet work being done by many hands at once, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre umber iron-red moss-green slate-grey and warm parchment cream, gentle diffused mid-morning light, small knotwork border running the top edge of the plate --ar 16:9 --s 250 --v 6.1`

**Prompt 6 — The Five Stones Set into the Wall:**
`A close framed view of the same workshop's outer wall later the same day, the five repairs now finished and visible side by side along a single span of wall — each repair rendered as a slightly-different stone or fitting clearly distinct in colour and texture from the older masonry but cleanly seated and properly mortared, on the leftmost stone a small carved Insular knotwork sigil framing the runes "I·II"; on the next stone a darker square block with a small iron-and-bronze reliquary-handle visible on its face sealed with hot wax bearing "III"; on the next a stout wicket gate with new oak crossbar marked "IV" in burnt notation; on the next a series of small chalk-and-ochre markers along the wall-edge labelled "V" in scratched capitals; and on the far right a small additional plaque at the base of the wall labelled "·ᛋᚪᚾᛞᛒᚩᚳᛋ·" for the startup sandbox guard, the prior standing at the wall with one hand resting flat against it looking quietly satisfied, the visiting abbot a few paces back making a small note in a wax tablet, a low evening light beginning to colour the stones warmly, a single sheep grazing at the wall's foot, sea-mist beginning to drift in from the cliff edge, atmosphere of completed careful work that is already being absorbed into the ordinary appearance of the wall, 2D hand-drawn illustration in the style of an Insular illuminated manuscript, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of warm stone-cream ochre iron-red moss-green slate-grey and warm parchment cream, gentle late-afternoon light, knotwork border in the bottom-right corner --ar 16:9 --s 250 --v 6.1`

**Prompt 7 — Three Bells Raised into the Tower:**
`A vertical-leaning view of a small round-towered chapel-tower above the monastery, three bells of different sizes being hoisted by ropes through the open louvred bell-chamber at the top — the largest bell at the bottom of the lift cast in dark iron with a small relief of a sheaf of wheat (a wooden tag tied to its yoke reading "ᛋᛁᚷᚾ" for "sign", representing signing_paused), the middle bell smaller and brighter cast in copper-tin alloy with a relief of a leaping fish or salmon (tag reading "ᛖᚷᚱᛖᛋ" for "egress"), the topmost bell smallest and brightest with an open scrollwork pattern around its rim and a relief of a small carved window or eye (tag reading "·ᚱᛈᛟᛚ·" for "RPC policy"), a bell-founder in a soot-stained tunic standing in the bell-chamber guiding each bell into its place with one gloved hand, a younger novice standing in the tower-stair holding a coil of plaited rope feeding upward through the louvre, the resident prior watching from the chapel doorway below with hands folded, the visiting abbot a few paces away observing the work but not interfering, a small flock of crows wheeling above the tower as if responding to the work, a single carved wooden notice nailed to the chapel doorframe reading "·ᛒᛖᚳᚪᚾ·" for "beacons" in scratched Insular capitals, sea visible to the right with a beacon-fire on a far headland echoing the bells, atmosphere of three independent bells each able to be sounded by the same single rope at the abbot's window, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of iron-grey copper-bronze warm-amber slate-grey and warm parchment cream, selective warm-amber accent only on the bells' rims and the bell-founder's hot iron, knotwork border running the full top edge --ar 16:9 --s 250 --v 6.1`

**Prompt 8 — Five Notices Nailed to the Church Door:**
`A close framed view of the heavy oak chapel door at the end of the monastery's small chapel, five small folded vellum notices nailed in a neat horizontal row across the upper half of the door each fixed by a single iron nail and each carrying only a few short lines of careful scratched-ink Insular capitals — the first notice carrying the heading "ᚷᚪᛏᛖ·ᚪ" (gate-A) and a single cross-mark of severity, the second "ᚷᚪᛏᛖ·ᛒ" (gate-B), the third heavily underlined with a triple cross-mark and headed "ᚱᛖᛚᛁᚳ" (relic) — the gravest, the fourth "ᚳᚪᚱᛏ" (cart), the fifth "ᚱᚩᚪᛞ" (road), each notice has a single thin gold-leaf inked line drawn from its lower edge curling along the door-frame and into the shadowed interior of the chapel where on a low wooden lectern just inside the open door rests a much larger unrolled vellum codex (the retrospective itself) on which a single illuminated initial letter is visible — gold-leaf threads from all five notices converging on this codex, a raven standing on the door's iron knocker eyeing the notices, a small Welsh shepherd-boy in the porch reading the third notice carefully with a finger raised to his lips, a soft band of evening light slanting through the porch from the right, the chapel doorframe carved with a worn Ruthwell-Cross-style border of vine-and-bird interlace, atmosphere of public minimal disclosure that explicitly points the reader inward to the longer codex inside, 2D hand-drawn illustration in the style of an Insular illuminated manuscript, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of oak-brown ochre iron-grey moss-green and warm parchment cream, selective gold-leaf accent only on the five threads converging on the codex, knotwork border in the upper-left corner --ar 16:9 --s 250 --v 6.1`

**Prompt 9 — The Mappa Mundi with the Fourth Lock Marked:**
`A hand-drawn early-medieval architectural mappa-mundi rendered as a single saga-plate in the manner of the Hereford Mappa Mundi, this plate showing not a literal world but a stack of seven horizontal bands stretched across the vellum like the levels of a great hill-fort in cross-section — at the bottom the deepest layer "ᚳᛖᛖᛈ" (Keep, Layer 1) shown as a small stone watchtower with a teal-glowing window for the silicon root, above it "ᛈᚪᚷᛖ" (Page, Layer 2) shown as an illuminated codex with an elliptic-curve drawn across its spread for the BN254 mathematics layer, above that "ᚻᚪᛚᛚᛋ" (Halls, Layer 3) shown as ten small stylised buildings for the on-chain contracts, then "ᚹᚪᛏᚳᚻ" (Watch, Layer 4-prime) shown as a watchtower with a brazier for the WAVS off-chain compute layer, then prominently and freshly inked in slightly bolder lines a NEW band labelled "·ᛚᚩᚳᚳ·IV·" (Lock IV) shown as the very wall and bell-tower of the monastery from earlier scenes — five distinct stones along the wall and three bells in the tower clearly visible at small scale — above that "ᛋᚻᛁᛈ" (Ship, Layer 5) shown as a small longship for IBC and mesh, then "ᛋᚳᚪᛚᛞ" (Skald, Layer 6) shown as a monk-scribe with a quill for the MCP layer, and at the top "ᚳᚩᚢᚾᚳᛁᛚ" (Council, Layer 7) shown as a small ring of thirteen standing stones for the DAO; thin gold-leaf interlace threads connecting each band to the next; a hand has just finished inking the new Lock IV band with the quill still resting at the top edge of that band, the ink slightly brighter than the surrounding bands, a single small label near the new band reading "·ᚹᚪᛚᛚᛋ·ᚪᚾᛞ·ᛒᛖᛚᛚᛋ·" (walls and bells), the whole plate framed in a thick braided ochre-and-iron-red Insular interlace border with a single eight-pointed compass-knot in the upper-right corner, atmosphere of an architectural diagram drawn as if it were a sacred map, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate, pen-and-ink underdrawing with earth-pigment watercolour wash on heavily aged cream vellum with visible fold-creases, muted palette of ochre umber iron-red moss-green woad-blue slate-grey and warm parchment cream, selective gold-leaf accent only on the connecting threads between bands and on the new Lock IV band's upper border --ar 16:9 --s 250 --v 6.1`

**Prompt 10 — The Ledger Entry at Dusk:**
`The interior of the monastery's stone scriptorium at dusk, a single hooded scribe seated at a sloped writing-desk lit by a small oil-lamp whose flame is steady and warm, the scribe finishing a long entry in a heavy leather-bound ledger book — the visible page headed in a delicately illuminated capital "V" (for Visitatio) and dense with a small clear half-uncial hand recording the visit's findings and the repairs made, five small marginal sketches in the page's outer margin showing in miniature the five sealed walls and three small bells and a bound vellum codex at the bottom, the scribe's quill paused at the very last line as he writes a final closing word, an inkpot of iron-gall ink and a small horn-flask of red ochre on the desk beside him, a folded copy of the abbot's letter set respectfully to one side, a single beam of last sunlight from a high window catching the gold-leaf in the illuminated initial, a ginger cat curled asleep on a cushion at the corner of the desk, the open chapel door visible beyond the scribe's shoulder showing a faint glow of the abbot already departing along the cliff path, atmosphere of the proper recording of a kindness that has been answered by a kindness, 2D hand-drawn illustration in the style of an Insular illuminated manuscript, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of warm lamp-amber dusk-blue ochre slate-grey and warm parchment cream, selective warm-amber accent only on the lamp flame and the gold-leaf initial, small knotwork colophon in the bottom-right corner of the plate containing a tiny stylised claw --ar 16:9 --s 250 --v 6.1`

**Prompt 11 — The Abbot Departing at the Harbour (Closing):**
`A wide closing scene at dusk showing the same coastal monastery from earlier but now seen from a low rise on the harbour path looking back, the visiting abbot riding away on a small chestnut pony along the cliff path with his leather satchel over his shoulder and the chapel-tower visible behind him on a higher slope — the three bells now in place in the tower's open louvres catching the last warm light — the resident prior standing at the monastery gate raising one hand in a small respectful farewell, a single beacon-fire just being lit by an unseen hand on the far headland echoing the bells, sea calm as glass reflecting layered peach slate-blue and lilac, a single clinker-built longship moored peacefully at the harbour pier with sail furled, a Welsh shepherd with a lean hound and a crooked ashplant walking the cliff path toward the abbot from the opposite direction with a slight nod of greeting as they pass, a raven settling on a small carved wooden sign at the path-junction reading "·ᚹᚪᛚᛚᛋ·ᚪᚾᛞ·ᛒᛖᛚᛚᛋ·" (walls and bells) in uneven Insular capitals, faint illuminated interlace motifs in the dusk-clouds above the chapel-tower as if the sky itself were the closing page of a manuscript, atmosphere of work completed, an old kindness answered with care, the watch continuing in both directions, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of dusk-peach slate-blue woad-indigo warm lamp-amber heather-purple and warm parchment cream, selective warm amber accent only on the beacon-fire and the bells in the tower, small knotwork colophon in the bottom-right corner containing a tiny stylised claw --ar 16:9 --s 250 --v 6.1`
