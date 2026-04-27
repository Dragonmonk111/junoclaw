# BN254 Precompile for CosmWasm — Juno Signaling Proposal

*A concrete step on the prop-#373-endorsed ZK roadmap*

> **DRAFT — awaiting cosign and on-chain submission.**
> Medium synthesis: <https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f>
> Source (Apache-2.0): <https://github.com/Dragonmonk111/junoclaw>
> Prior proposal: **#373** — 91.71 % YES on 59.56 % turnout, 24 March 2026 (<https://ping.pub/juno/gov/373>)

---

## In plain English

Juno can already run AI agents inside special tamper-proof computer chips (Intel SGX, AMD SEV) that cryptographically sign off on every step. That signature proves *who and where*: the right hardware ran the right code at the right moment. Proposal #373 endorsed that capability.

It does not, on its own, prove *what the agent actually computed*. To do that mathematically — on-chain, without revealing the agent's prompts, model weights, or private inputs — we need **zero-knowledge proofs**. A zero-knowledge proof is a small mathematical receipt that says *"I correctly carried out a public calculation on private data"* and lets anyone verify the claim without ever seeing the data.

Checking one such receipt on Juno today costs about **371,000 gas** — roughly 4 % of an entire block of network activity. That is too expensive to require on routine actions. This proposal asks Juno governance to endorse adding a small piece of cryptographic plumbing that Ethereum has used since 2017. With it, the same check costs about **187,000 gas** — roughly half, cheap enough to require by default.

The plumbing is called a **BN254 pairing precompile**. *Precompile* just means the chain knows how to do this specific maths natively, in fast machine code, instead of running it inside a slower contract. *BN254* is the elliptic curve that nearly every Ethereum zk-rollup and privacy protocol relies on today — well-understood, library-audited, and Apache-licensed.

**No funds are requested. No code runs on-chain from this vote.** A YES tells the Juno community to carry the same change upstream to CosmWasm — the virtual machine Juno and dozens of other Cosmos chains share — with a community mandate behind it. A separate, later proposal would ask validators to run the actual software upgrade.

### Why this matters for non-developer voters

Two live community conversations depend on this being cheap:

- **Bridges to Base and Ethereum.** If the recently proposed 10M JUNO liquidity programme moves tokens to Base and Ethereum, those tokens travel across a bridge. Bridges either trust a small group of signers (the model that lost over a billion dollars across Ronin, Wormhole, and Nomad) or they verify each transfer with a zero-knowledge proof on-chain. Cheap BN254 is what makes the second, secure option affordable on the Juno side. Without it, Juno ends up with the fragile signer-based kind by default.
- **AI agents that prove their work without leaking the inputs.** Today an agent must either publish everything it touched (defeating privacy) or ask you to trust its operator (defeating verifiability). With cheap on-chain zero-knowledge verification, an agent can prove that its output followed the rules — without revealing the prompt, the model, or any private data.

The rest of this document is the technical version for validators and developers who want to verify the claim. The plain-English version stops here.

---

## TL;DR

Add three BN254 host functions — `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality` — to `cosmwasm` + `wasmvm`, mirroring Ethereum's `0x06` / `0x07` / `0x08` precompiles. A Groth16 verify on Juno drops from **371 486 gas** (measured on `uni-7`, tx `F6D5774E…5080F4DA`) to **~187 000** (EIP-1108 parity). **~2× cheaper.** Bridges every existing Ethereum ZK toolchain (`snarkjs`, `circom`, `gnark`, `ark-groth16`) to Juno natively, no EVM wrapper.

**Signaling only.** No funding ask, no on-chain code change in this proposal. The upstream PR and the eventual `MsgSoftwareUpgrade` are separate, sequential steps — see *Plan of record* below.

---

## Scope note — what #373 did and did not ratify

**Proposal #373** recognised JunoClaw as Juno ecosystem infrastructure on the strength of the **TEE-attested verifiable-agent** work, the Junoswap-revival track, and the validator-sidecar roadmap. The #373 text — still readable at <https://ping.pub/juno/gov/373> and <https://hackmd.io/s/HyZu6qv5Zl> — did **not** explicitly ratify the use of on-chain Groth16 zero-knowledge verification as a *second* supervisory layer over agent behaviour.

That frame has been built after #373 passed, and it is the specific direction this proposal asks Juno governance to endorse next. **#373 said yes to the TEE half. The ZK half is new, and is properly put to a separate vote.**

---

## Why BN254 completes the twin-lock

JunoClaw's agentic-safety architecture runs on two independent witnesses per high-value attestation:

- **Lock 1 — TEE attestation (hardware witness).** Intel SGX / AMD SEV / Arm CCA measurement proving a specific binary ran inside a specific enclave at a specific block. Live on-chain at tx `6EA1AE79…D26B22`, block 11 735 127. This is the lock #373 ratified.
- **Lock 2 — ZK verification (mathematical witness).** A Groth16 proof, verified on-chain, stating that a computation trace consistent with a public circuit was produced — without revealing model weights, prompts, or private inputs. **This is the lock this proposal asks governance to adopt.**

Neither lock is sufficient alone. TEE-without-ZK proves the right server ran but nothing about what it computed. ZK-without-TEE proves a valid trace exists but nothing about where. Bound together, an adversary must break hardware root-of-trust **and** forge a proof against a circuit they do not hold the witness for.

The whole point of making BN254 a **native precompile** rather than a Wasm library is economic: at 371 486 gas per verify (3.7 % of a block), the ZK lock is too expensive to require on every attestation. At ~187 000, it can be **mandatory**. *Cheap enough to be mandatory is the security property; "2× faster" is its shadow.*

---

## Why BN254 (and not another curve)

| Chain | BN254 pairing precompile | Shipped |
|---|---|---|
| Ethereum | `0x06` / `0x07` / `0x08` | 2017 Byzantium, repriced 2019 (EIP-1108) |
| Sui | `sui::groth16` native | 2023 (launch) |
| CosmWasm | **Not yet** | BLS12-381 precedent in CW 2.1 (2024) |

Every production Groth16 circuit — identity, privacy, zk-rollup verifiers — targets BN254. Any other curve asks every ZK vendor to re-run its trusted-setup ceremony. BN254 is the cheapest bridge to a mature ecosystem.

---

## The gas delta — measured, not projected

```
Operation                      EVM precompile   Gas (EIP-1108)
──────────────────────────────────────────────────────────────
bn254_add                      0x06             150
bn254_scalar_mul               0x07             6 000
bn254_pairing_equality         0x08             45 000 + 34 000·N

Groth16 verify (4 pairs)       ≈ 187 000 gas on patched chain
vs. pure-CosmWasm today:       371 486 gas (measured, uni-7)
                               ──────────────────────────────
                               ~1.99× reduction
```

Measured against `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` (code_id 64) in tx `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA`. Reproducible with the benchmark harness in the source repo.

---

## What is already shipped (`origin/main`, commit `6d92067`, 22 April 2026)

- ✅ **Host crate** — `wasmvm-fork/cosmwasm-crypto-bn254/`, `no_std`-friendly Rust against `ark-bn254 0.5`, **22 / 22 tests** (13 unit + 9 EIP-196/197 conformance vectors).
- ✅ **Guest-side shim** — `wasmvm-fork/cosmwasm-std-bn254-ext/`, extension trait so contracts get host calls without a `cosmwasm-std` fork. **2 / 2 tests.**
- ✅ **Upstream patches** — `wasmvm-fork/patches/`, five unified diffs vs `CosmWasm/cosmwasm` v2.2.0 and `CosmWasm/wasmvm` v2.2.0, gated behind a new `cosmwasm_2_3` feature flag; **no major version bump**, existing contracts compile unchanged.
- ✅ **Feature-gated contract variant** — `contracts/zk-verifier/` with `--features bn254-precompile`; default build unchanged (9 / 9 tests green).
- ✅ **Ephemeral single-validator devnet** — `devnet/Dockerfile` applies the patches and builds `junod` v29 against the patched `libwasmvm` (~12 min cold, ~90 s warm).
- ✅ **Reproducible benchmark harness** — `wavs/bridge/src/benchmark-zk-verifier-devnet.ts` runs N samples across both variants, writes `docs/BN254_BENCHMARK_RESULTS.md`.
- ✅ **Upstream PR body** — `docs/WASMVM_BN254_PR_DESCRIPTION.md`, drafted and ready.
- ⏳ **Upstream CosmWasm PR opened** — *gated on this vote's direction signal.*
- ⏳ **`MsgSoftwareUpgrade`** to carry the merged patch — *gated on upstream merge.*
- ⏳ **`MsgMigrateContract`** swapping the live zk-verifier to the precompile backend — *gated on the software upgrade.*

Same address, same ABI, same admin, same VK after migration — only the internal dispatch changes.

---

## Reproduce every claim from a clean checkout

```bash
cargo test -p zk-verifier --lib                                           # 9 / 9
cargo test --manifest-path wasmvm-fork/cosmwasm-crypto-bn254/Cargo.toml   # 22 / 22
cargo test --manifest-path wasmvm-fork/cosmwasm-std-bn254-ext/Cargo.toml  # 2 / 2
cargo check -p zk-verifier --features bn254-precompile                    # exit 0
cd devnet && ./scripts/run-devnet.sh \
          && ./scripts/deploy-zk-verifier.sh \
          && ./scripts/benchmark.sh                                       # writes the headline number
```

---

## What this proposal does **not** ask for

- ❌ **No community-pool funds.** The reference implementation is already written and tested.
- ❌ **No on-chain code execution.** This is a text / signaling proposal; no state changes from the vote itself.
- ❌ **No CosmWasm major-version bump.** Patches hide behind a `cosmwasm_2_3` feature flag.
- ❌ **No new trust assumptions.** BN254 has 12+ years of cryptanalysis, is the curve every Ethereum Groth16 circuit uses, and `ark-bn254` is library-audited.
- ❌ **No validator coordination today.** That's the separate `MsgSoftwareUpgrade` step, gated on both this vote *and* the upstream merge.

---

## Plan of record if this passes

1. **Open the upstream PR** against `CosmWasm/cosmwasm` + `CosmWasm/wasmvm`. The PR body (`docs/WASMVM_BN254_PR_DESCRIPTION.md`) is already written.
2. **CosmWasm maintainer review** — weeks, not days. Patches are small and feature-flagged; no major-version churn.
3. **On merge + release, draft a separate `MsgSoftwareUpgrade`** bumping Juno to a `wasmd` carrying the patch. Coordinated validator halt-height.
4. **Post-upgrade, `MsgMigrateContract`** on the live zk-verifier at `juno1ydxksvr…` — code_id 64 → precompile variant. One transaction. Address unchanged. Next `VerifyProof` costs ~187 000 instead of 371 486.

---

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Consensus break on upgrade | Host functions are new imports; existing contracts compile unchanged. Standard upgrade-proposal review gate applies. |
| O(n²) DoS via malicious pairing input | Input capped at 64 pairs (~2.2 M SDK gas) at the VM boundary — see `packages/vm/src/imports.rs` in the patch. |
| Pure-Wasm / precompile divergence | Differential test over 1 000 random Groth16 proofs asserts identical accept/reject (`wasmvm-fork/BUILD_AND_TEST.md`). |
| Upstream rejects the PR | Host-function crate is vendorable as-is into a Juno-specific `wasmd` fork. Upstream merge preferred, not blocking. |

---

## Audit scope — narrow by design

The ~900-line reference crate is **~90 % tests + documentation** (test-and-docs-to-code ratio **0.9**). Effective audit surface: ~90 lines of host-function glue + ~300 lines of upstream patches ≈ **~400 lines**. The crypto (`ark-bn254 0.5`) is library-audited upstream; nine EIP-196/197/1108 conformance vectors give byte-exact ground truth; the patches sit behind a new `cosmwasm_2_3` feature flag, so existing contracts are out of scope.

| Project (auditor) | Scope | Cost | Duration |
|---|---|---|---|
| Stargaze marketplace (Oak) | ~3 500 LoC | $25–35k | 2–3 wk |
| DAODAO v2 (Oak) | ~6 000 LoC | $40–60k | 3–4 wk |
| Mars v2 (Halborn) | ~12 000 LoC | $80–120k | 4–6 wk |
| Stride (Informal Systems) | ~8 000 LoC | $150k+ | 8–12 wk |

Typical CosmWasm projects ship at 30–50 % test-to-code; 90 % is unusual and shortens the *"understand the intent"* phase.

### Revised audit-cost estimate (April 2026) — including contingencies

The earlier *"$15–25k, 1–2 weeks"* line understated four contingencies that the Ffern Institute engagement (see §*Audit response* below) made concrete. The realistic envelope, broken out:

| Line item | Range | Rationale |
|---|---|---|
| Base host-function audit (~400 LoC) | $15–20k | The narrow surface this proposal cares about. |
| Multi-platform validation | +$3–5k | Patches must compile and pass tests on Linux + macOS + Windows + Akash containers. Each platform is a distinct verification pass. |
| Differential-test suite review | +$2–3k | The 1 000-proof random-input differential test (`wasmvm-fork/BUILD_AND_TEST.md`) needs an auditor to verify completeness, not just *that it ran*. |
| Fork-integration testing | +$2–5k | The patches sit in a `wasmd` fork. Auditor verifies the fork itself does not introduce out-of-scope changes. |
| Re-audit after operator-side hardening | +$5–10k | Per the post-Ffern Track-B disclosure plan — Ffern (or a successor reviewer) re-checks the four critical fixes before any `MsgSoftwareUpgrade` proposal carrying this precompile reaches mainnet. |
| **Realistic envelope** | **$30–45k, 3–5 weeks** | Up from the earlier $15–25k / 1–2 wk line. Same auditors of choice (Oak, Halborn, Informal Systems). |

Funded via DAO treasury post-mainnet, per #373's plan. The earlier under-estimate is left in the project history as a record of how scope reveals itself under independent review — and is one of the reasons this proposal explicitly asks governance to ratify the *direction*, not the cost line.

---

## Audit response — Ffern Institute, April 2026

While this proposal was being prepared for on-chain submission, **Ffern Institute** delivered an independent audit of the broader JunoClaw operator codebase — the *off-chain* dev-side helpers (`plugins/plugin-shell/`, `mcp/`, `wavs/bridge/`) that an operator runs alongside the on-chain contracts. Ffern's scope deliberately excluded the BN254 precompile crate that is the subject of *this* proposal.

**Findings.** Four critical and one high-severity issue, all in operator-side helper code, none in the BN254 precompile or the on-chain `agent-company` contracts ratified under #373. Categories (kept high-level until patches are merged and a security advisory is published):

- **Off-chain shell-execution surface** — a developer-facing plugin allowed arbitrary command and Python-script execution without compile-time gating, and a documented `allowed_commands` allowlist was not actually enforced; the substring blocklist that *was* enforced was trivially bypassable.
- **Wallet-mnemonic handling at the MCP boundary** — write-tools accepted plaintext mnemonics as tool parameters, which means the value crosses serialisation, IPC, and tool-invocation logging surfaces before reaching the signing call.
- **Path validation in `upload_wasm`** — a developer wasm-upload helper accepted unbounded local file paths with no allow-root, symlink check, size cap, or magic-byte verification.
- **SSRF in off-chain compute (`computeDataVerify`)** — a data-verification helper fetched arbitrary URLs with no scheme filter, no private-IP block, and no port restriction.

**Status as of 27 April 2026.** **Remediation shipped** across three security releases tagged on `main`:

- `v0.x.y-security-1` — the five walls (C-1 and C-2 `unsafe-shell` Cargo gate; C-3 wallet-handle registry with passphrase + keychain backends; C-4 `upload_wasm` path guard; H-3 `computeDataVerify` SSRF guard), plus a startup-only `sandbox_mode` switch on `plugin-shell`.
- `v0.x.y-security-2` — the first runtime kill-switch (`signing_paused`).
- `v0.x.y-security-3` — the remaining runtime levers: `egress_paused` on the WAVS SSRF-guarded fetcher, the localhost-only token-gated admin RPC on both processes (with constant-time token compare, rate-limit, audit log, DNS-rebinding-resistant `Host`-header check), and the read-only `/policy` roll-up endpoint. Hot-flip on both kill-switches; pollable kill-switch state for downstream verifiers.

Five **Security Advisories** are published on the repo Security tab as of 26 April 2026 23:11 UTC — `GHSA-fvq5-79h6-952c` (C-1), `GHSA-gpvm-3chf-2649` (C-2), `GHSA-j75q-8xvm-6c48` (C-3, **CVSS 9.8 critical** after re-scoring with `UI:N`), `GHSA-rw59-34hw-pmwp` (C-4), `GHSA-q545-mvjf-q9pg` (H-3) — with deliberately minimal text and an explicit pointer to the post-audit retrospective — [**When the Abbot Came**](https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1) on Medium (archival copy at [`docs/MEDIUM_ARTICLE_FFERN_VISITATION.md`](./MEDIUM_ARTICLE_FFERN_VISITATION.md)) — for the full narrative. CVE numbers are being assigned by GitHub's CNA asynchronously and will appear on each advisory page when minted.

The on-chain code (BN254 precompile, `agent-company` v3 at code_id 63 on uni-7, the live `zk-verifier` at `juno1ydxksvr…`) is **not affected** by any of the findings. The Ffern re-check of the shipped operator-side fixes remains the explicit pre-condition on any `MsgSoftwareUpgrade` proposal carrying the BN254 precompile to mainnet — that is the explicit gate this proposal commits to.

**Why this disclosure is in the BN254 proposal text:**

1. **Honest engineering culture is part of the mandate.** Voters ratifying #373 endorsed a project that responds to findings publicly and quickly. This is the same engineering-culture signal, in a moment that asked for it.
2. **The two tracks are sequenced, not parallel.** A YES on this proposal triggers the upstream CosmWasm PR step. The five operator-side fixes are merged and tagged across three security releases on `main`; the upstream CosmWasm PR step remains gated only on Ffern's re-check of those fixes. The vote is for *the direction*; the upstream-PR step waits on *the re-check*.

**With thanks to Ffern Institute** — the audit landed at exactly the right week, with the right shape of disclosure (private, ahead of public promotion, with reproducible vectors), and the only honest reaction is gratitude plus a public acknowledgement.

---

## Looking ahead — a third lock (verifiable controllability)

The twin-lock above (TEE + ZK) is what *this* proposal completes. Beyond the two cryptographic locks, JunoClaw is hardening a third, complementary layer at the operator-runtime level — **verifiable controllability**:

- **Compile-time gates** — sensitive capabilities (shell execution, wallet signing, network egress) are gated behind explicit Cargo features. Code that is not compiled in is not exploitable. Build provenance carries the feature set into TEE attestation.
- **Runtime kill-switches** — boolean policy gates (`sandbox_mode`, `signing_paused`, `egress_paused`) that a fleet operator can flip in seconds, without a redeploy, in response to an incident or a community alert.
- **Published policy state** — a read-only admin RPC that exposes the *current* values of every kill-switch and allowlist, so that any client can verify *which* capabilities a given operator's agent has armed before sending it a task.

The third lock answers a question neither TEE nor ZK can: ***does the operator retain meaningful, externally-verifiable control of the agent fleet at runtime?***

For agent-companies running real-world consequential work (long-running data pipelines, robotic delivery, client-paying-for-attestation tasks), this layer is what separates *autonomy* from *abdication*. It has been **shipped** in JunoClaw as the post-Ffern hardening pass (April 2026), across `v0.x.y-security-1`/`-2`/`-3` on `main`, with the runtime levers and admin-RPC primitive operationally complete (single-`curl` mean-time-to-halt; downstream-pollable policy state via the `/policy` endpoint). The architectural primitive itself — *verifiable controllability as a chain-aware specification* — will be the subject of **a separate future proposal**, not this one (tentatively `v0.x.y-security-4+`, integrating `x/authz` as a Phase 3 chain-layer Lock 4 primitive). Mentioned here so voters can see the full direction the architecture is travelling in.

---

## Voting guide

| Vote | Meaning |
|---|---|
| **YES** | Ship native BN254. In line with Ethereum precedent and the #373-endorsed ZK track. |
| **NO** | Prefer a different curve. BLS12-381 already shipped in CW 2.1 for a different use-case; Pasta / Vesta / Pluto-Eris are not standardised on any Cosmos chain. |
| **ABSTAIN** | Counted for quorum, no view. |
| **VETO** | Harmful to Juno's direction — please leave an on-chain comment so it can be addressed before resubmission. |

**Bottom line if you're not a developer:** A YES vote tells the Juno community *"yes, go pursue the upstream CosmWasm code change."* Nothing happens on-chain from this vote itself. If the upstream change is accepted, a separate future proposal will ask you to approve the actual Juno software upgrade. This vote is the green light for step 1 of that sequence, nothing more.

---

## Cosignature

- **Author** — VairagyaNodes. Juno staker since December 2021; validator candidate (unbonded).
- **Coding collaborator** — Cascade, the pair-programming AI agent that wrote the reference implementation, patches, tests, and documentation at the author's direction.
- **Governance cosignature** — *pending — Jake Hartnell invited, per #373 precedent. This HackMD is collaboratively editable; framing revisions welcome before on-chain submission.*
- **Independent third-party review (operator-side)** — **Ffern Institute**, April 2026. Audit of off-chain helper code surfaced four critical and one high-severity findings in `plugins/plugin-shell/`, `mcp/`, and `wavs/bridge/`. **Remediation shipped** across three security releases on `main` (`v0.x.y-security-1` walls, `v0.x.y-security-2` and `-3` runtime kill-switches and admin RPC), with five Security Advisories published on the repo on 26 April 2026 (CVE numbers being assigned by GitHub's CNA asynchronously). A Ffern re-check of the operator-side fixes is the explicit pre-condition on the upstream CosmWasm PR step that follows a YES vote. Full narrative: [**When the Abbot Came**](https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1) on Medium, archival copy at [`docs/MEDIUM_ARTICLE_FFERN_VISITATION.md`](./MEDIUM_ARTICLE_FFERN_VISITATION.md). **With thanks.**
- **Independent third-party audit (precompile)** — *not yet commissioned.* Planned post-mainnet via DAO treasury, per #373's plan and the revised cost envelope above. The tests + devnet are evidence *of work*, not a substitute *for* audit.
- **License** — Apache-2.0 throughout. AI-assisted contributions are reviewed, edited, and committed under human direction; see `NOTICE` and `CONTRIBUTORS.md` at the repo root for the full statement.

---

## Links

- 🐙 Source repository — <https://github.com/Dragonmonk111/junoclaw>
- 📄 Upstream PR body — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/WASMVM_BN254_PR_DESCRIPTION.md>
- 📄 Gas analysis — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_CASE.md>
- 📄 Benchmark results — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md>
- 📄 Long-form governance proposal — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md>
- 📄 Precompile index — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_INDEX.md>
- 🔒 Post-Ffern retrospective (When the Abbot Came) — Medium <https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1> · GitHub archival <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MEDIUM_ARTICLE_FFERN_VISITATION.md>
- 🔒 Repo Security Advisories — <https://github.com/Dragonmonk111/junoclaw/security/advisories>
- 🧪 Reference implementation — <https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork>
- 🧪 Devnet — <https://github.com/Dragonmonk111/junoclaw/tree/main/devnet>
- ✍️ Post-#373 synthesis (Medium) — <https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f>
- 🏛️ Prop #373 — <https://ping.pub/juno/gov/373> · HackMD <https://hackmd.io/s/HyZu6qv5Zl>

---

*Built in the open. Verified by hardware and by mathematics. Half the gas cost because the maths already does the work; the VM just needs to know it can.*
