# Upstream GitHub Issue Drafts

**Status:** READY TO PUBLISH after Phase 0 completes (patches rebased + gas measured)
**Companion:** [`POST_VOTE_EXECUTION_PLAN.md`](./POST_VOTE_EXECUTION_PLAN.md) Phase 1
**Targets:** `CosmWasm/cosmwasm`, `CosmWasm/wasmvm`

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

The immediate user is the JunoClaw `zk-verifier` contract running on Juno mainnet today. Pure-Wasm Groth16 verification (arkworks) costs **370,719 SDK gas** per `VerifyProof`, measured on `uni-7` and reproduced on devnet (see [BN254_BENCHMARK_RESULTS.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md)).

Projected with the precompile path: **~187,000 SDK gas** for a 3-pair circuit, **~223,000** for a 4-pair circuit. That's a ~2× reduction, which is the threshold between "we sample-verify proofs" and "we verify every proof." For an agent-based use case, that's the difference between optional and universal auditing.

We will replace the projection with a measured number from a local devnet running the patched chain binary before opening the PR.

## What's already written

We have:

- **A standalone `no_std`-friendly crate** (`cosmwasm-crypto-bn254`) using `ark-bn254 0.5`. It includes 9 conformance tests, criterion benchmarks, and a non-default-features build that compiles into the cosmwasm-vm Wasm target.
- **Three patches** against `cosmwasm` `v2.2.0` ([here](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches)) — currently being rebased onto the latest tag.
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
**Title:** `Proposal: BN254 VM integration for cosmwasm-crypto-bn254 host functions`
**Labels (suggested):** `enhancement`, `crypto`, `discussion`
**Cross-reference:** companion issue on `CosmWasm/cosmwasm` (link added once Issue 1 is published)

### Body

```markdown
## Context

This is the wasmvm-side companion to [CosmWasm/cosmwasm#XXXX](TBD-after-issue-1-published) — a proposal to add BN254 (alt_bn128) host functions to CosmWasm, motivated by Juno governance proposal [#374](https://ping.pub/juno/gov/374) (passed 2026-05-05, ~80% Yes).

The cosmwasm-side issue covers the host-function ABI, the Rust crate, and the gas schedule. This issue covers what we'd add **here** in `wasmvm`: CGo FFI shims, Go-side wrappers, and the surface that `x/wasm` consumers call.

## What we'd add to wasmvm

### `libwasmvm/src/bn254_ffi.rs` (new)

Three `#[no_mangle] pub extern "C" fn` shims, one per host function, following the existing `ByteSliceView` → `UnmanagedVector` convention used for BLS12-381:

```rust
pub unsafe extern "C" fn bn254_add(
    a: ByteSliceView,
    b: ByteSliceView,
    error_msg: Option<&mut UnmanagedVector>,
) -> UnmanagedVector { ... }
```

Same shape for `bn254_scalar_mul` and `bn254_pairing_equality`. Each routes into `cosmwasm-crypto-bn254` (the new crate added in the cosmwasm-side PR).

### `internal/api/bn254.go` (new)

Go-side wrappers around the C shims, returning `([]byte, error)` in the conventional way.

### `lib_libwasmvm.go` (patch)

Public-surface functions exported to `x/wasm` consumers, mirroring the BLS12-381 entry points.

## Why split into two PRs

The cosmwasm-side PR adds the host functions and the crate. The wasmvm-side PR (this one) wires them through to Go. Splitting the change matches the existing repo structure — the BLS12-381 work landed in two PRs for the same reason.

The wasmvm PR depends on the cosmwasm PR (we'd open the wasmvm PR with a blocking note pointing at the cosmwasm one).

## Test plan

- `make build-rust` — compile the C shims
- `make test` — Go FFI sanity checks
- A reproducible end-to-end gas measurement on a single-validator devnet running the patched binary, comparing to the existing 370,719 SDK gas pure-Wasm baseline (see [BN254_BENCHMARK_RESULTS.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md))

The differential test (1,000 random Groth16 proofs through both the pure-Wasm verifier and the precompile-backed verifier, assert identical accept/reject) is in the cosmwasm-side PR but is reproducible from this side too via the devnet recipe.

## What we'd like to confirm

1. **Naming.** `bn254_add` / `bn254_scalar_mul` / `bn254_pairing_equality` for the C shim symbols. Match the cosmwasm-side names verbatim. Acceptable?
2. **CI matrix.** Should the new crate appear in `make test`'s default suite, or behind a feature flag matching cosmwasm-side `cosmwasm_2_3`?
3. **Gas-cost reporting.** Do you want the wasmvm-side benchmark numbers in the PR description, or only the cosmwasm-side numbers (since gas accounting lives in cosmwasm-vm)?
4. **Track-target preference.** Our patch set is currently rebased onto `cosmwasm` v2.2.2 (the version `wasmvm` v2.2.4 pins, i.e. the baseline Juno mainnet ships today). Would you prefer the upstream PR against `v3.x main` directly, with a backport branch for the v2.2.x line, or against `v2.2.x` first with a forward-port to `main` afterwards?

## Side-findings worth flagging (independent of the BN254 work)

While rebasing the patches onto `wasmvm` v2.2.4 we hit two compatibility issues that are independent of the BN254 work itself but affect any source build of `wasmvm` v2.2.x in 2026. Surfacing them here in case they're useful even if the BN254 proposal stalls.

### Finding 1 — `wasmer-vm` 4.3.7 × Rust ≥ 1.81: `__rust_probestack` linker error

`wasmer-vm` 4.3.7 (transitive dep) emits inline assembly that references `__rust_probestack`, which `compiler-builtins` removed in Rust 1.81 ([rust-lang/rust#126985](https://github.com/rust-lang/rust/pull/126985), Sept 2024). Result: source builds of `wasmvm` v2.2.x fail at link time on Rust ≥ 1.81 with:

```
rust-lld: error: undefined symbol: __rust_probestack
  >>> referenced by libcalls.rs:668
  >>>     wasmer_vm-*.rcgu.o:(wasmer_vm_probestack) in archive libwasmer_vm-*.rlib
```

Validators running pre-built `libwasmvm.x86_64.so` are unaffected. Anyone source-building `wasmvm` v2.2.x on a fresh dev machine in 2026 will hit this wall.

Workaround we use: pin Rust 1.78.0 via `rust-toolchain.toml` in our patch set. Real fix would be either bumping `wasmer-vm` to a version that uses `__llvm_stack_probe` instead, or carrying a small in-tree patch that defines `__rust_probestack` as a shim.

This is independent of the BN254 work — we'd flag it the same way if we were rebasing any other patch set against `wasmvm` v2.2.x.

### Finding 2 — v2.2.4 doesn't surface BLS12-381 at the cgo layer

The original BN254 wasmvm patches we authored against `v2.2.0` added `Bn254Add` / `Bn254ScalarMul` / `Bn254PairingEquality` Go-side wrappers in `lib_libwasmvm.go` and matching C-ABI shims in `internal/api/bindings.h`, modelled on the BLS12-381 surface. When we tried to apply them to `v2.2.4` they failed cleanly — inspecting `v2.2.4`'s `lib_libwasmvm.go` and `internal/api/bindings.h` showed BLS12-381 itself has no Go-side wrappers in this branch. The pattern those patches were copying lives on `v3.x main`, not on the `v2.2.x` line.

This is good for our v2.2.x track — it shrinks Track A to cosmwasm-only — but we wanted to flag it because it surprised us. Is the absence intentional (e.g. "BLS12-381 is for in-VM use only, never for direct Go callers"), or is it pending a follow-up that didn't make the v2.2.x branch? If the former, we'd happily restrict the BN254 PR to `v3.x main` only and skip the wasmvm-side wrappers on the older branch. If the latter, we can include the wrappers in the v2.2.x backport when we get there.

## Pinging once, politely

cc @webmaster128 @ethanfrey — happy to adapt the FFI shape to whatever pattern fits cleanest with the existing wasmvm codebase. The implementation is mechanical once the cosmwasm side is agreed.

---

**Repo (for reference, not a request):** [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) — the live v2.2.2-rebased patches are at [`wasmvm-fork/patches/v2.2.2/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v2.2.2) (10 numbered patches with a [README manifest](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/v2.2.2/README.md)). The dropped Go-wrapper attempts are kept at `wasmvm-fork/patches/wasmvm.*.patch.dropped` for the audit trail described in Finding 2. The verification harness [`wasmvm-fork/patches/rebase-track-a.sh`](https://github.com/Dragonmonk111/junoclaw/blob/main/wasmvm-fork/patches/rebase-track-a.sh) re-applies and re-tests the patch set from a clean checkout in ~2 minutes.
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
