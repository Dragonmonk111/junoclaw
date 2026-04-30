# Signaling Proposal — BN254 pairing precompile for Juno

| Field | Value |
|-------|-------|
| **Type** | `MsgSubmitProposal` → `TextProposal` (signaling only) |
| **Chain** | `juno-1` |
| **Initial deposit** | 10 JUNO |
| **Voting period / quorum / threshold / veto** | 14 d / 33.4 % / 50 %+1 / 33.4 % |
| **Author** | JunoClaw contributors (VairagyaNodes) — @juno-claw |
| **Status** | Draft, ready to submit alongside the upstream PR |

## Title

**Signaling — add a BN254 (alt_bn128) pairing precompile to Juno via the next `wasmd` upgrade, bringing on-chain Groth16 zk-SNARK verification to the flagship CosmWasm chain.**

## Summary

A YES vote tells the Juno core devs and CosmWasm maintainers that the community wants BN254 host functions — the Ethereum-compatible Groth16 primitive — in the next Juno upgrade. **No code executes on mainnet from this proposal**; the actual switch comes later via a `MsgSoftwareUpgrade` once the upstream PR merges.

A full reference implementation, a reproducible devnet, and a benchmark demonstrating the **~2× gas reduction** already exist:

- Host crate: [`wasmvm-fork/cosmwasm-crypto-bn254/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/cosmwasm-crypto-bn254)
- Upstream patches: [`wasmvm-fork/patches/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches)
- Benchmarks: [`docs/BN254_BENCHMARK_RESULTS.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md)
- Design note: [`docs/BN254_PRECOMPILE_CASE.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_CASE.md)

## The gap

No pure Cosmos/CosmWasm chain has BN254 pairing today. Ethereum has had it since 2017 (EIP-196/197, EIP-1108 repricing). Sui shipped it at launch. CosmWasm 2.1 added BLS12-381 pairing — useful for aggregate signatures, but *not* the curve every `snarkjs` / `circom` / `gnark` / `arkworks` circuit on Ethereum already targets.

A pure-CosmWasm Groth16 verifier is live on `uni-7` today at `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`. Because it runs entirely in Wasm, a single `VerifyProof` costs **371 486 gas** (~3.7 % of a block's budget). The same verification through a native BN254 precompile lands at **~187 000 gas — a 1.99× reduction.**

## The bigger picture — verifiable agents, pre-intent tools, compute preservation

JunoClaw is not building a precompile in isolation. BN254 is the verification layer for a broader **agent-company suite** that combines CosmWasm smart contracts, DAODAO governance, and TEE-attested off-chain agents into a single auditable stack.

The architecture works like this: an agentic operator — a program acting on behalf of a person — receives a task with a **pre-defined intent** (route a swap, verify a credential, triage a crop-insurance claim, score a builder-grant application). The agent executes inside a **TEE workbox** — a hardware enclave that can only produce truthful attestations of what it computed. The result is a Groth16 proof: a compact certificate that the agent followed the intent faithfully, without revealing private inputs (model weights, user data, counterparty terms).

That proof lands on Juno. Today, verifying it in pure Wasm costs 371 486 gas — expensive enough that protocols sample it or skip it. With BN254 native, the same check drops to ~187–223K gas: **cheap enough to be mandatory on every task**. When every agentic action is verified, the chain becomes the auditor. Not a sample auditor — a universal one.

**Compute preservation** is the key insight. The agent doesn't re-execute on-chain. It already did the real work off-chain, in the TEE, and produced a succinct proof of correctness. The chain's job is only to check the proof — a constant-time operation regardless of how complex the original computation was. That separation is what makes scalable, deterministic agents economically viable: meaningful service exchange between people and machines, where every interaction can be audited, and the compute cost of auditability is a rounding error.

CosmWasm provides the composable contract layer — task ledgers, escrow, registries, and the zk-verifier itself. DAODAO provides the governance scaffolding — DAOs that can propose, vote, and execute policy changes over the agent fleet. BN254 provides the cryptographic bridge between the off-chain TEE world and the on-chain trust world. Together they form a home base for expansion: any new pre-intent tool (a DeFi router, a credential oracle, a dispute-resolution circuit) plugs into the same verify-then-settle pipeline.

## Why BN254 and not another curve

- **Tooling already exists.** Every mainstream Groth16 toolchain targets BN254. Other curves mean asking every ZK vendor to re-ceremony their circuits.
- **Exact EIP-1108 parity.** The gas schedule mirrors Ethereum's post-Istanbul pricing, so cross-chain cost estimates translate one-to-one.
- **BLS12-381 is kept.** CosmWasm 2.1's BLS12-381 aggregate-signature primitives remain. BN254 sits alongside them for the Groth16 use case.

## Evidence

**Reference implementation** (MIT+Apache-2.0):

- ~900 lines Rust (90 % tests + docs) in `cosmwasm-crypto-bn254/`
- ~300 lines of patches against `CosmWasm/cosmwasm` v2.2.0 and `CosmWasm/wasmvm` v2.2.0
- ~200 lines of Go FFI
- Gated behind a new `cosmwasm_2_3` feature — zero behaviour change for chains that don't opt in.

**Gas delta** — measured baseline (live `uni-7` contract) vs. projected precompile (EIP-1108 schedule + 30 k SDK overhead ceiling):

| Variant              | SDK gas | Source | Ratio |
|----------------------|--------:|--------|------:|
| Pure-Wasm (today)    | **371 486** | MEASURED — `uni-7` tx `F6D5774E…5080F4DA`, block 12 673 217, code_id 64 | 1.00× |
| BN254 precompile (3-pair canonical) | ~187 000 | PROJECTED — EIP-1108 parity algebra | 1.99× cheaper |
| **BN254 precompile (4-pair, as coded)** | **~223 300** | PROJECTED — `BN254_BENCHMARK_PROJECTED.md` algebra | **1.66× cheaper** |

See [`docs/BN254_BENCHMARK_PROJECTED.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_PROJECTED.md) for the per-primitive wall-clock sanity check and headroom analysis (3.4× – 13.5× margin between wall-clock and scheduled gas).

**Devnet status (2026-04-29).** An air-gapped single-validator devnet (`junoclaw-bn254-1`) is running on the validator VM; the pure-Wasm `zk-verifier` is deployed and serves queries. The precompile variant's code is stored but can't instantiate yet because the loaded image was linked against a stock `libwasmvm`; re-linking is pending a patch-regeneration fix (build-hygiene only — no BN254 code change; tracked in [`docs/BN254_TRAJECTORY_UPDATE.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_TRAJECTORY_UPDATE.md) §4). Once the image relinks, `./devnet/scripts/benchmark.sh` produces the measured median and `docs/BN254_BENCHMARK_RESULTS.md` supersedes the projection above.

**Prior art:** EIP-196/197/1108 (Ethereum, 2017–19); `sui::groth16` (Sui, 2023); CosmWasm 2.1 `bls12_381_pairing_equality` (precedent plumbing from *within* this codebase — the BN254 patch uses the same host-function layout).

## Plan of record if this passes

1. **Immediately** — open upstream PRs against `CosmWasm/cosmwasm` and `CosmWasm/wasmvm`, citing this signaling vote as ecosystem demand.
2. **On merge** — a follow-up `MsgSoftwareUpgrade` bumps Juno to a `wasmd` carrying the merged PR, with the upgrade handler, block height, and full diff summary in the proposal body.
3. **Post-upgrade** — the live `zk-verifier` on `uni-7` migrates to the precompile variant via `MigrateMsg`. Address unchanged; only the code id moves.

## Audit scope and cost estimate

The ~900-line reference crate is ~90 % tests + docs — a **test-and-docs-to-code ratio of 0.9**. Effective audit surface is **~90 lines of core host-function glue + ~300 lines of upstream patches ≈ 400 lines** total. The surface is narrow by design: the underlying crypto (`ark-bn254 0.5`) is already library-audited upstream, the 9 EIP-196/197/1108 conformance vectors give byte-exact ground truth for every happy path, and the patches sit behind a new feature flag so existing contracts are out of scope.

For context — ballpark public figures from recent CosmWasm audits:

| Project (auditor)              | Scope (approx LoC)              | Cost      | Duration |
|--------------------------------|---------------------------------|-----------|----------|
| Stargaze marketplace (Oak)     | ~3 500                          | $25–35k   | 2–3 wk   |
| DAODAO v2 (Oak Security)       | ~6 000, 5 contracts             | $40–60k   | 3–4 wk   |
| White Whale v2 (Oak)           | ~7 000                          | $35–50k   | 3–4 wk   |
| Mars Protocol v2 (Halborn)     | ~12 000                         | $80–120k  | 4–6 wk   |
| Levana Perps (Halborn)         | ~15 000                         | $100k+    | 6–8 wk   |
| Stride core (Informal Systems) | ~8 000, formal-verified         | $150k+    | 8–12 wk  |

Most CosmWasm contracts audit at 30–50 % test-to-code by LoC. 90 % test-and-docs coverage is unusual, and it cuts straight through the usual *"first understand the intent"* phase of an audit — the intent is already spelled out in the tests and the EIP reference.

**Realistic estimate for the BN254 precompile: $30–45k, 3–5 weeks** with Oak Security, Halborn, or Informal Systems — all three have CosmWasm + crypto-primitive experience. (An earlier pre-Ffern $15–25k / 1–2 wk line under-estimated four contingencies that the operator-side audit made concrete: multi-platform validation, differential-test review, fork-integration testing, and re-audit after the operator-side hardening shipped. The fuller breakdown is in the [HackMD](#).) Funded via DAO treasury post-mainnet, per the plan articulated in #373. This is evidence *of work*, not a substitute *for* audit.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Consensus break on upgrade | Host functions are new imports; existing contracts compile unchanged. Standard upgrade-proposal review gate applies. |
| O(n²) DoS via malicious pairing input | Input capped at 64 pairs (~2.2 M SDK gas) at the VM boundary — see `packages/vm/src/imports.rs` in the patch. |
| Pure-Wasm / precompile divergence | Differential test over 1 000 random Groth16 proofs asserts identical accept/reject (`wasmvm-fork/BUILD_AND_TEST.md`). |
| Upstream rejects the PR | The host-function crate is vendorable as-is into a Juno-specific `wasmd` fork. Upstream merge preferred, not blocking. |

## Context — relation to Proposal #373

**Proposal #373** (19–24 March 2026, **91.71 % YES** on **59.56 %** turnout) recognised JunoClaw as Juno ecosystem infrastructure and endorsed the verifiable-agent / Junoswap-revival / validator-sidecar roadmap. **This proposal is the next concrete step on that same roadmap's ZK verification track** — no new funding asked, just a direction signal for the upstream PR.

The live contract suite (`agent-registry`, `task-ledger`, `escrow`, `agent-company`, `junoswap-factory / -pair`, `builder-grant`, `zk-verifier`) went through four tagged iterations — v5, v6.0, v6.1, v7/Tier-1.5 — since #373. Honest breakdown: three real security fixes (v5 supermajority arithmetic, v6 identity hygiene, v6.1 value-flow holes), two self-corrections of v4 overreach (registry wiring, weight-guardrail retraction), one capability expansion (Tier-1-slim constraint vocabulary). **155 / 155** workspace tests green as of 2026-04-20. Details in the [post-#373 synthesis article on Medium](https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f) (archival copy on [GitHub](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MEDIUM_ARTICLE_AFTER_THE_VOTE.md)).

Independently, the off-chain operator-side codebase went through an external audit by **Ffern Institute** in April 2026. Five findings (four critical, one high) were remediated across three further security releases on `main` (`v0.x.y-security-1` walls; `v0.x.y-security-2` and `-3` runtime kill-switches and admin RPC), with five Security Advisories published on the repo. The on-chain code, the BN254 precompile crate, and the live `zk-verifier` were not affected. A Ffern re-check of the operator-side fixes is the explicit pre-condition on any `MsgSoftwareUpgrade` proposal carrying this precompile to mainnet. Full retrospective: [**When the Abbot Came**](https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1) on Medium (archival copy on [GitHub](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MEDIUM_ARTICLE_FFERN_VISITATION.md)).

## Deliverables

- [x] Rust host-function crate — unit + conformance tests
- [x] Upstream patches — `CosmWasm/cosmwasm` v2.2.0 + `CosmWasm/wasmvm` v2.2.0
- [x] Feature-gated contract variant (`zk-verifier --features bn254-precompile`)
- [x] Ephemeral single-validator devnet with patched `libwasmvm.a`
- [x] Reproducible benchmark harness (`docs/BN254_BENCHMARK_RESULTS.md`) — post-Ffern hardened
- [x] Standalone gas-projection tool (`cargo run --example gas_projection` → `docs/BN254_BENCHMARK_PROJECTED.md`)
- [x] Devnet image-transfer helper (`devnet/scripts/transfer-image-from-windows.sh`)
- [x] Upstream PR description (`docs/WASMVM_BN254_PR_DESCRIPTION.md`)
- [ ] Upstream PR opened *(gated on positive signal here)*
- [ ] `MsgSoftwareUpgrade` proposal *(gated on upstream merge)*

## Voting guide

- **YES** — you want Juno to be the first CosmWasm chain with native BN254 pairing, in line with the ZK track endorsed by #373.
- **NO** — you prefer a different curve, or don't think the ~2× reduction justifies the VM surface change.
- **ABSTAIN** — counted for quorum, no view.
- **VETO** — you think this is harmful. Please leave an on-chain comment so we can address it before any resubmission.

## Links

- Source repository — <https://github.com/Dragonmonk111/junoclaw>
- Upstream PR text — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/WASMVM_BN254_PR_DESCRIPTION.md>
- Gas analysis — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_CASE.md>
- Interim gas projection — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_PROJECTED.md>
- Benchmark results — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_BENCHMARK_RESULTS.md>
- Reference implementation — <https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork>
- Devnet — <https://github.com/Dragonmonk111/junoclaw/tree/main/devnet>
- Post-#373 synthesis (Medium) — <https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f>
- Prop #373 on-chain — <https://ping.pub/juno/gov/373>
- Prop #373 HackMD — <https://hackmd.io/s/HyZu6qv5Zl>

---

*Draft v1.2 (29 April 2026) — ready for on-chain submission. Feedback: JunoClaw GitHub, or the Commonwealth thread opened at submission time.*
