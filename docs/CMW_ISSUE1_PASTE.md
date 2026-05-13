# CosmWasm/cosmwasm Issue 1 — paste-ready block

*Open at [`https://github.com/CosmWasm/cosmwasm/issues/new`](https://github.com/CosmWasm/cosmwasm/issues/new). Choose "Open a blank issue." Paste the title and body below verbatim. Do **not** pre-apply labels — let maintainers do that.*

*This file is a clean extraction of `UPSTREAM_ISSUE_DRAFTS.md` §Issue 1, ready for one-click copy. The canonical source remains `UPSTREAM_ISSUE_DRAFTS.md`; if you edit this file you should also propagate to the canonical source.*

---

## Title

```
Proposal: BN254 (alt_bn128) host functions for Groth16 verification
```

## Body (paste this into the GitHub issue textarea)

````markdown
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
````

---

## After posting

1. Capture the URL the GitHub UI gives you (e.g. `https://github.com/CosmWasm/cosmwasm/issues/NNNN`).
2. Open `UPSTREAM_ISSUE_DRAFTS.md` and edit Issue 2's body — replace the placeholder `[CosmWasm/cosmwasm#XXXX](TBD-after-issue-1-published)` with the real URL. Do **not** open Issue 2 yet (still waiting for at least one substantive maintainer reply per the pacing in `UPSTREAM_ISSUE_DRAFTS.md` §Pacing).
3. Telegram FYI to Dimi: "Opened upstream issue. Link: [URL]. No ask, just keeping you in the loop."
4. Do **not** post on Twitter / Discord. Wait for at least one substantive maintainer comment before broadcasting.

---

*Apache-2.0. This file is the click-once paste vehicle for Issue 1; the canonical source is `UPSTREAM_ISSUE_DRAFTS.md`.*
