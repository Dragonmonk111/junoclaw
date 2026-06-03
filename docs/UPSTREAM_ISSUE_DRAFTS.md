# Upstream GitHub Issue Drafts

**Status:** PUBLISHED 2026-06-03. Issue 1 = [CosmWasm/cosmwasm#2685](https://github.com/CosmWasm/cosmwasm/issues/2685). Issue 2 = [CosmWasm/wasmvm#735](https://github.com/CosmWasm/wasmvm/issues/735). Forward-ported now spans **three** parallel patch series, all green:

- `wasmvm-fork/patches/v2.2.2/` — audit reference, matches `wasmvm` v2.2.4 / Juno mainnet today (10/10 CLEAN, 22/22 + 311/311 PASS)
- `wasmvm-fork/patches/v2.2.7/` — latest 2.2.x (10/10 CLEAN; only 2 Cargo.toml patches differ from v2.2.2)
- `wasmvm-fork/patches/v3.0.x/` — latest **v3.0.6** (10/10 CLEAN, 22/22 + 318/319 PASS; the single failure is a pre-existing floating-point compile-test on vanilla v3, unrelated to BN254)

Gas measured empirically at **1.823× reduction** on devnet (370,600 → 203,266 SDK gas per Groth16 verify, 5 samples σ = 0). Wasmvm-side Go-wrapper patches were **dropped** in the v3 series after confirming BLS12-381 itself has no Go wrappers in either v2.2.x or v3.x — the absence is intentional across the codebase, not a 2.2.x-only oversight. Issue 2 now scopes to a single confirmation question + the two side-findings worth flagging.
**Companion:** [`POST_VOTE_EXECUTION_PLAN.md`](./POST_VOTE_EXECUTION_PLAN.md) Phase 1, [`memory/track-b-forward-port.md`](../memory/track-b-forward-port.md) day-2.5 worklog
**Targets:** `CosmWasm/cosmwasm`, `CosmWasm/wasmvm`

## Publish sequence (do these in order)

1. **Open Issue 1** (`CosmWasm/cosmwasm`). Copy the body from the fenced ` ```markdown ` block under `## Issue 1` below. Use the title shown there. Do NOT pre-apply labels — let maintainers do that.
2. **Capture the URL** (e.g. `https://github.com/CosmWasm/cosmwasm/issues/NNNN`). Paste it into `_private/upstream_threads.md` (gitignored) as `Issue 1: <url>`. If `_private/` doesn't exist, just keep the URL in your terminal scrollback for step 3.
3. **Edit Issue 2's body in this file** before pasting it: replace the placeholder `[CosmWasm/cosmwasm#XXXX](TBD-after-issue-1-published)` (appears once, in the Context section) with `[CosmWasm/cosmwasm#NNNN](https://github.com/CosmWasm/cosmwasm/issues/NNNN)`. Do this edit here, in markdown, then copy — do not edit in the GitHub UI.
4. **Open Issue 2** (`CosmWasm/wasmvm`). Same procedure as step 1.
5. **Capture Issue 2's URL** the same way.
6. **Edit Issue 1 once on GitHub** to add the Issue 2 cross-link as a single appended line at the bottom of the body: `Companion VM-side issue: <issue-2-url>`. This is the only allowed in-browser edit.
7. **Telegram FYI to Dimi** — see `## After publishing` below for the wording template.

No Twitter / Discord broadcast until at least one substantive maintainer reply on either issue. A silent issue + a public broadcast reads as a stalled project.

---

## Why issues, not PRs

A cold PR forces maintainers into a choice between accepting code they didn't shape or rejecting work that was already done. An issue invites them to shape the work — which makes the eventual PR a confirmation of an agreement, not a request for one.

These drafts are deliberately **short**, **concrete**, and **end with a question**. They are not pitch decks. They do not link 14 documents. They cite the on-chain mandate, the measured numbers, and the design — and ask one question.

When ready to publish, copy the body verbatim into the GitHub issue UI. Do not edit in the browser; edit here, paste once.

---

## Issue 1 — `CosmWasm/cosmwasm`

**Repo:** `https://github.com/CosmWasm/cosmwasm`
**Title:** `Proposal: BN254 (alt_bn128) host functions for Groth16 verification`
**Labels (suggested by maintainer, do not pre-apply):** `enhancement`, `crypto`, `discussion`
**Cross-reference:** issue [#751](https://github.com/CosmWasm/cosmwasm/issues/751) (Crypto API meta — lists BN254 as "Bonus Points")

### Body

```markdown
## Context

Juno governance approved [proposal #374](https://ping.pub/juno/gov/374) on May 5, 2026 — a community signaling proposal in favour of adding BN254 (alt_bn128) host functions to the CosmWasm VM. Final tally was ~80% Yes / 22% Abstain / 0.003% No-with-Veto over a 44% turnout.

The proposal is signaling-only — the actual code lives in this repo, and so does the decision about whether to merge it. We're opening this issue to surface the design, the gas methodology, and the measured numbers, and to ask: **is the shape acceptable before we open a PR?**

This issue is the upstream half of [issue #751](https://github.com/CosmWasm/cosmwasm/issues/751) ("BN254 pairing primitives — Bonus Points").

## What we're proposing

Three new host functions in `cosmwasm-vm`, with guest-side declarations in `cosmwasm-std`, behind a new `cosmwasm_2_3` feature flag:

| Host function              | Ethereum precompile | Input    | Output | SDK gas (proposed)  |
|----------------------------|---------------------|----------|--------|---------------------|
| `bn254_add`                | `0x06` ECADD        | 128 B    | 64 B   | 150                 |
| `bn254_scalar_mul`         | `0x07` ECMUL        | 96 B     | 64 B   | 6,000               |
| `bn254_pairing_equality`   | `0x08` ECPAIRING    | 192·N B  | bool   | 45,000 + 34,000·N   |

Byte layouts and gas constants lifted from EIP-196 / EIP-197 / EIP-1108 so existing Groth16 tooling (`snarkjs`, `circom`, `gnark`, `ark-groth16`) targets CosmWasm without adaptation.

The new chain capability string is `bn254`. Existing contracts compile unchanged.

## Why this matters concretely

The immediate user is the JunoClaw `zk-verifier` contract running on Juno mainnet today. We've now measured both paths end-to-end on a single-validator devnet running the patched chain binary, 5 samples per variant, σ = 0:

| Path                  | Gas per `VerifyProof` |
|-----------------------|----------------------:|
| Pure-Wasm (arkworks)  |               370,600 |
| BN254 precompile      |           **203,266** |

That's a **1.823× reduction** (167,334 SDK gas saved per call), which is the threshold between "we sample-verify proofs" and "we verify every proof." For an agent-based use case, that's the difference between optional and universal auditing.

Full per-run table (txhashes, block heights, gas wanted, gas used) and the original projection it confirms within ~9% are in [BN254_BENCHMARK_RESULTS.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md). The measurement is reproducible from a clean checkout in one command:

```bash
bash devnet/scripts/reproduce-benchmark.sh
```

## What's already written

We have:

- **A standalone `no_std`-friendly crate** (`cosmwasm-crypto-bn254`) using `ark-bn254 0.5`. 22/22 tests pass (13 unit + 9 EIP-196/197/1108 conformance vectors); criterion benchmarks included; non-default-features build compiles into the cosmwasm-vm Wasm target.
- **Two parallel patch series**, both 10/10 clean:
  - [`wasmvm-fork/patches/v2.2.2/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v2.2.2) — pinned to the cosmwasm tag that wasmvm v2.2.4 / Juno mainnet consume; tests pass (22/22 crypto-bn254, 311/311 cosmwasm-vm).
  - [`wasmvm-fork/patches/v2.2.7/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v2.2.7) — forward-port to the latest 2.2.x tag; only 2 `Cargo.toml` patches differ (workspace inheritance).
- **Verification tooling** in the same directory: [`check-baseline.sh`](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/check-baseline.sh) (fast `git apply --check` per patch), [`rebase-track-a.sh`](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/rebase-track-a.sh) (full clone-apply-test loop).
- **A separate companion issue** opened on `CosmWasm/wasmvm` for the VM-side wiring: [link to be added].
- **An ADR** documenting the decision context: [ADR-001-BN254-PRECOMPILE.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/ADR-001-BN254-PRECOMPILE.md).
- **A draft PR description** (the body of the future PR): [WASMVM_BN254_PR_DESCRIPTION.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/WASMVM_BN254_PR_DESCRIPTION.md).

We're not opening the PR yet. We want to confirm the shape first.

## What we'd like to confirm before opening a PR

1. **ABI shape.** Three host functions, named `bn254_add` / `bn254_scalar_mul` / `bn254_pairing_equality`, behind feature flag `cosmwasm_2_3`. Acceptable, or would you prefer a different module/feature name?
2. **Gas schedule.** EIP-1108 constants × 100 (matching wasmd's default `gas_per_op` multiplier and the existing BLS12-381 path). Is the methodology sound, or do you want a different derivation?
3. **Capability string.** New chain capability `bn254`. Acceptable name?
4. **Empty-pairing semantics.** EIP-197 says empty pairing input returns `Ok(true)`. We follow this. Confirm preference?
5. **Subgroup checks.** G1 has cofactor 1 on BN254 (no subgroup check needed). G2 needs `is_in_correct_subgroup_assuming_on_curve` after `is_on_curve`. We do both. Confirm correctness expectations?
6. **Scope.** This proposal is intentionally **minimal** — three host functions, no `bn254_hash_to_curve`, no `bn254_signature_verify`. Hash-to-curve we'd raise in a follow-up if there's interest. Acceptable scope?

## Non-goals (explicit)

- We're not adding `bn254_hash_to_g1` or `_g2`. Domain separation is a separate conversation.
- We're not adding BN254-based BLS aggregate signatures — `bls12_381_pairing_equality` already covers that case on a stronger curve.
- We're not bumping `cosmwasm-std`'s major version — the new methods live behind `cosmwasm_2_3`.

## On the Juno governance signal

The Juno proposal was a community-level "yes, we want this on our chain" — it does not bind your decision in any way. We treat the proposal as evidence that there is real demand from at least one Cosmos chain, not as authority over the upstream codebase.

If the upstream answer is "not in this shape," the Juno proposal still serves: we'd discuss alternatives in this thread and adapt.

## Pinging once, politely

cc @ethanfrey @webmaster128 — happy to take this in any direction you prefer. We'd rather wait two weeks for guidance than open a PR you'd reject.

---

**Repo (for reference, not as a request):** [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) — Apache-2.0 throughout. The `wasmvm-fork/` directory contains the crate, the patches, and the build recipe.
```

---

## Issue 2 — `CosmWasm/wasmvm`

**Repo:** `https://github.com/CosmWasm/wasmvm`
**Title:** `Question: should BN254 host functions also expose Go-side wrappers, or follow the BLS12-381 precedent of staying VM-internal?`
**Labels (suggested):** `question`, `crypto`, `discussion`
**Cross-reference:** companion issue on `CosmWasm/cosmwasm` (link added once Issue 1 is published)

### Body

```markdown
## Context

This is the wasmvm-side companion to [CosmWasm/cosmwasm#2685](https://github.com/CosmWasm/cosmwasm/issues/2685) — a proposal to add BN254 (alt_bn128) host functions to CosmWasm, motivated by Juno governance proposal [#374](https://ping.pub/juno/gov/374) (passed 2026-05-05, ~80% Yes / 22% Abstain / 0.003% No-with-Veto).

The cosmwasm-side issue covers the host-function ABI, the Rust crate, the gas schedule, and the **measured 1.823× gas reduction** (370,600 → 203,266 SDK gas per Groth16 verification, 5 samples σ = 0). The patches there target three cosmwasm tags in parallel — v2.2.2 (audit baseline), v2.2.7 (latest 2.2.x), and **v3.0.6** (latest v3) — all 10/10 CLEAN.

**This wasmvm-side issue is now smaller in scope than originally drafted.** When we tried to author Go-side wrappers (mirroring the BLS12-381 pattern we expected), the patches failed because **BLS12-381 itself has no Go-side wrappers in either `wasmvm` v2.2.4 or `wasmvm` v3.0.4**. We dropped the wrapper patches and would like to confirm that this absence is intentional design before deciding whether to contribute the wrappers ourselves.

## The single question

Is the absence of Go-side wrappers for BLS12-381 (and by extension, BN254) **intentional design** — i.e. "these primitives are for use by Wasm contracts via host-function imports only, never by direct Go callers in `x/wasm`-adjacent code" — or is it a not-yet-done that you'd accept a contribution for?

We're happy with either answer:

- **"Intentional, leave it out":** then the BN254 work is **cosmwasm-only**. Issue 1 is the entire upstream surface and this issue closes once that's confirmed.
- **"Not-yet-done, contribution welcome":** then we'd open a follow-up wasmvm PR with the BLS12-381 + BN254 Go wrappers in one symmetrical change. The Go-side surface for BN254 alone would look like:

  ```go
  // internal/api/bn254.go (new)
  func Bn254Add(a, b []byte) ([]byte, error) { ... }
  func Bn254ScalarMul(point, scalar []byte) ([]byte, error) { ... }
  func Bn254PairingEquality(input []byte) (bool, error) { ... }
  ```

  with matching C-ABI shims in `libwasmvm/src/` and exports in `lib_libwasmvm.go`. We have a draft of this from our v2.2.0-era attempt; it'd take ~1 day to refresh against v3.0.4.

## What's already shipping (no wasmvm change required)

For either answer, the existing patches stand:

- The cosmwasm-side host functions work end-to-end. A Wasm contract on a chain running the patched cosmwasm-vm (and any wasmvm version that consumes it, including stock v3.0.4) gets the 1.823× gas reduction today.
- The Juno mainnet binary (Track A) is the consumer. It compiles against patched `cosmwasm-crypto-bn254` and stock `wasmvm`. No wasmvm-fork is in our publication path.
- All tests pass on v3.0.6: 22/22 crypto-bn254 + 318/319 cosmwasm-vm (the single failure is a pre-existing floating-point compile-test on vanilla v3, unrelated to BN254 — reproducible by reverting the patches).

## Side-findings worth flagging (independent of the BN254 work)

Two issues surfaced while rebasing the patches in 2026 that are unrelated to BN254 itself but affect any source build of `wasmvm` in this period.

### Finding 1 — `wasmer-vm` 4.3.7 × Rust ≥ 1.81: `__rust_probestack` linker error (v2.2.x only)

`wasmer-vm` 4.3.7 (transitive dep on the v2.2.x line) emits inline assembly referencing `__rust_probestack`, which `compiler-builtins` removed in Rust 1.81 ([rust-lang/rust#126985](https://github.com/rust-lang/rust/pull/126985), Sept 2024). Source builds of `wasmvm` v2.2.x fail at link time on Rust ≥ 1.81:

```
rust-lld: error: undefined symbol: __rust_probestack
  >>> referenced by libcalls.rs:668
  >>>     wasmer_vm-*.rcgu.o:(wasmer_vm_probestack) in archive libwasmer_vm-*.rlib
```

Validators running pre-built `libwasmvm.x86_64.so` are unaffected. Anyone source-building `wasmvm` v2.2.x on a fresh dev machine in 2026 will hit this wall.

Workaround we use on v2.2.x: pin Rust 1.78.0 via `rust-toolchain.toml` in our patch set. Real fix on the v2.2.x line would be either bumping `wasmer-vm` to a version that uses `__llvm_stack_probe`, or carrying an in-tree shim. **The v3.x line already resolves this** — v3.0.4's `wasmer-vm` is newer and the 1.82 toolchain compiles cleanly. We pin Rust 1.82 in our v3.0.x patch set.

This is independent of the BN254 work; we'd flag it the same way if we were rebasing any other patch set against `wasmvm` v2.2.x.

### Finding 2 — BLS12-381 has no Go-side wrappers in either v2.2.x or v3.x

This is the same observation that motivated the single question above. Restated as a finding: when we surveyed `lib_libwasmvm.go` and `internal/api/bindings.h` across `wasmvm` v2.2.0, v2.2.4, and v3.0.4, no exported Go function corresponds to a BLS12-381 host operation. The host functions are reachable from Wasm contracts via cosmwasm-vm's import wiring, but not from Go code that links `libwasmvm` directly.

If this is intentional, we'd just like a one-line confirmation so we can document it for downstream consumers (we've had several questions about it on the Juno developer Discord).

## Why we're keeping the issue scope tight

The original draft of this issue proposed full FFI shims, Go wrappers, public-surface exports, and a CI plan — mirroring how a BLS12-381 PR *would* have looked if BLS12-381 itself had wrappers. After confirming it doesn't, we'd rather ask the design question first than spec-up code you may not want.

If the answer is "contribute the wrappers," we'll open a separate issue with the full FFI proposal and a draft PR.

## Pinging once, politely

cc @webmaster128 @ethanfrey — happy to take this in either direction. The cosmwasm-side patches stand on their own; this is a polish question.

---

**Repo (for reference, not a request):** [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) — the live patch series:

- [`wasmvm-fork/patches/v2.2.2/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v2.2.2) — audit reference, matches `wasmvm` v2.2.4 pin / Juno mainnet today.
- [`wasmvm-fork/patches/v2.2.7/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v2.2.7) — forward-port to latest 2.2.x tag.
- [`wasmvm-fork/patches/v3.0.x/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v3.0.x) — forward-port to latest **v3.0.6**.

All three series are 10/10 CLEAN with `git apply --check`. Each has its own README manifest. The dropped Go-wrapper attempts (the ones referenced in Finding 2) are kept at `wasmvm-fork/patches/wasmvm.*.patch.dropped` for the audit trail. Verification harnesses: [`check-baseline.sh`](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/check-baseline.sh) (fast `git apply --check` per patch) and [`apply-and-test-v3.ps1`](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/apply-and-test-v3.ps1) (full clone-apply-test loop on v3.0.6, ~3 minutes from a warm cache).
```

---

## After publishing

1. Capture the issue URLs in `_private/upstream_threads.md` (gitignored).
2. Update Issue 1 to link Issue 2 (and vice versa) — a one-line edit.
3. Send Dimi a brief Telegram FYI: "Opened upstream issues. Links: ..., .... No ask, just keeping you in the loop."
4. Do NOT post the issue links on Twitter/Discord yet. Wait for at least one substantive maintainer comment before broadcasting — otherwise a silent issue gets misread as a stalled project.

## Pacing

- **Day of publishing:** issues live, FYI to Dimi & Jake (DM only).
- **Day +7:** if no engagement, do nothing. Maintainers are busy; a week of silence is normal.
- **Day +14:** if still no engagement, post a single follow-up comment per issue: "Following up — happy to hear thoughts on the shape, or to wait if you'd prefer." That's the entire comment.
- **Day +21:** if still no engagement, escalate via the Layer.xyz / Juno Discord (Jake Hartnell can poke if asked nicely; he endorsed prop #373 and seeded the TEE milestone).

The escalation chain matters more than the issue text. Maintainers are humans; humans respond to repeated, polite, low-pressure pings, and resent loud or impatient ones. Default to lower-pressure than feels comfortable.

---

*These drafts are versioned. If we revise them after maintainer feedback, the older version stays in git history as a record of what was originally proposed.*
