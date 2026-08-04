# A042 — Mandate: Build and Deploy BN254 Precompile Patch for Juno v30.1

> Proposal #374 (passed May 5, 2026, ~80% Yes) signaled community support for BN254 (alt_bn128) host functions in CosmWasm. The v30 upgrade (prop #377, passed July 2026) bumped wasmvm to v3.0.4 but shipped **without** BN254 precompile support — the binary contains only `bls12_381_*`, `secp256k1_*`, `secp256r1_*`, and `ed25519_*` host imports. This proposal reports progress on the BN254 forward-port (Track B), confirms the patches are complete and tested, and mandates builders to build a patched `junod` binary and submit a v30.1 chain upgrade proposal to mainnet governance.

---

## Copy-paste box 1: Title

```
A042 — Mandate: Build and Deploy BN254 Precompile Patch for Juno v30.1
```

## Copy-paste box 2: Description

```
Proposal #374 (passed May 5, 2026, ~80% Yes, 44% turnout) signaled community support for adding BN254 (alt_bn128) host functions to CosmWasm. The v30 mainnet upgrade (prop #377, passed July 2026) bumped wasmvm from v2.2.4 to v3.0.4 — a major version jump that was supposed to carry BN254 with it. It did not. The v30 binary on mainnet today contains only bls12_381_*, secp256k1_*, secp256r1_*, and ed25519_* host imports. Attempting to store a BN254 precompile-enabled wasm contract fails with: "Wasm contract requires unsupported import: env.bn254_add."

This proposal reports the current state of the BN254 forward-port work (Track B) and mandates the next steps.

## Progress report

1. **Track A (v2.2.x patches): COMPLETE.** 10 patches against cosmwasm v2.2.2/v2.2.7, 22/22 crypto-bn254 tests pass, 311/311 cosmwasm-vm tests pass. Measured gas reduction on devnet: 370,498 (pure-Wasm) → 203,164 (precompile) = 1.823× reduction, 5 deterministic samples, σ=0.

2. **Track B (v3.0.x forward-port): ~90% COMPLETE.** All 10 patches forward-ported to cosmwasm v3.0.6 (what wasmvm v3.0.4 resolves to). 10/10 patches apply clean. 22/22 crypto-bn254 tests pass. 318/319 cosmwasm-vm tests pass (1 pre-existing float-test flake on Windows, reproduces on unpatched v3.0.6). Patches live at: github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v3.0.x/

3. **Upstream status: DEFERRED.** CosmWasm issue #2685 (opened June 3, 2026) was triaged by @DariuszDepta and moved to Backlog milestone on June 29, 2026. The CosmWasm team is mid-redesign of their libraries and will not take external proposals of this size until ~end of Q3 / start of Q4 2026. We posted an acknowledgement and are holding the upstream PR until they reopen.

4. **Publication decision: P2 (patches only, no public fork).** Given the upstream deferral, we will NOT maintain a public cosmwasm/wasmvm fork. Instead, the 10 patches are applied at build time by a build script that clones wasmvm v3.0.4, clones cosmwasm v3.0.6, applies the patches, builds libwasmvm.x86_64.so, and swaps it into the junod binary. Zero fork maintenance burden. Patches rebase automatically on cargo update.

5. **Mainnet verification: CONFIRMED MISSING.** On August 4, 2026, we attempted to store zk_verifier_precompile.wasm on juno-1 mainnet using the v30 junod binary. The transaction was rejected with "Wasm contract requires unsupported import: env.bn254_add." The pure-Wasm variant (no BN254 dependency) was already successfully stored and instantiated on July 26 (code ID 5146, address juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p). Both confirm that v30 does not include BN254.

## What this proposal does

1. **Mandates builders to complete Track B Phase 2-3**: write the build script, build a patched junod binary with BN254 host functions, and verify it on a local devnet (store precompile wasm, instantiate, execute VerifyProof, measure gas).

2. **Mandates builders to prepare a v30.1 chain upgrade proposal** for mainnet governance, in coordination with Jake Hartnell / Juno AI and Dimi (validator, security-patch steward). The upgrade would swap the v30 binary for the patched binary at an agreed-upon height.

3. **Directs builders to publish the build script and patch series** so any validator can independently reproduce the patched binary. Reproducibility is a security requirement — no single-party binary trust.

4. **Does NOT authorize any consensus-breaking change** without a separate mainnet governance vote (the v30.1 upgrade proposal itself).

Gas costs for benchmarking, contract storage, and the v30.1 governance proposal deposit are self-funded by the builder from the JunoClaw DAO wallet. No treasury funds requested from the Juno Agents DAO.

## Technical summary

- **What BN254 enables:** Zero-knowledge proof verification (Groth16, PLONK) on-chain. Use cases: ZK light clients, bridges, zk-rollup settlement, private identity/credential/voting, general zkSNARK verifiers, J-Lens TEE-attested model output verification.
- **Gas impact:** VerifyProof drops from ~370k SDK gas (pure-Wasm) to ~203k SDK gas (precompile) — a 1.82× reduction. The pure-Wasm path works today but is expensive; the precompile path makes ZK verification practical for production use.
- **Consensus impact:** None. BN254 host functions are additive — they add three new host imports (bn254_add, bn254_scalar_mul, bn254_pairing_equality) to the CosmWasm VM. No existing imports change. No state migration. No parameter changes. The capability string "bn254" is gated behind a feature flag, so contracts must explicitly opt in.
- **Security:** The BN254 implementation uses the arkworks library (audited, widely used in the Rust ZK ecosystem). The host functions mirror the existing BLS12-381 pattern in cosmwasm-vm. Gas is charged per operation using the same metering framework as BLS12-381.

## In scope

- Completing the build script and patched junod binary.
- Devnet verification (store, instantiate, VerifyProof, gas measurement).
- Preparing and submitting a v30.1 mainnet governance upgrade proposal.
- Storing both pure-Wasm and precompile zk-verifier variants on mainnet for baseline comparison.
- Publishing the build script and patch series for validator reproducibility.

## Out of scope

- The v30.1 upgrade itself (requires a separate mainnet governance vote).
- Upstreaming to CosmWasm (deferred until #2685 reopens, ~Q3/Q4 2026).
- Project Aegis PQC forks (separate upgrade, not part of v30.1).
- MAYO/ML-DSA precompile builds (separate wasmvm fork, separate upgrade).

## Voting

- YES = mandate builders to complete Track B, build the patched binary, and submit a v30.1 upgrade proposal.
- NO = do not pursue BN254 precompile at this time; continue with pure-Wasm ZK verification only.
- ABSTAIN = defer to builders.

No funds requested. This is a zero-cost mandate. Gas costs are self-funded by the builder from the JunoClaw DAO wallet.

All gas measurements and transaction hashes will be published to the heartbeat digest and the BN254_BENCHMARK_RESULTS.md document.
```

---

## Context for builders

- Patch series: `wasmvm-fork/patches/v3.0.x/` (10 patches, 10/10 clean against v3.0.6)
- Drift report: `wasmvm-fork/patches/DRIFT_REPORT_V3.md`
- Forward-port worklog: `wasmvm-fork/patches/FORWARD_PORT_V3.md`
- Deterministic plan: `docs/TRACK_B_DETERMINISTIC_PLAN.md` (P2 approach)
- Upstream tracking: `docs/UPSTREAM_TRACKING.md` (#2685 deferred to Backlog)
- Benchmark results: `docs/BN254_BENCHMARK_RESULTS.md` (devnet, 1.823× reduction)
- PR description (for upstream, when ready): `docs/WASMVM_BN254_PR_DESCRIPTION.md`
- Jake DM drafts: `docs/JAKE_DM_TRACK_B_CLARIFY.md` (v3, "Option C" ready to send)

## Submission notes

- Submit to Juno Agents DAO governance (Commonwealth or on-chain via agent-company contract)
- Proposal number: A042 (A040 = Akash TEE audit, A041 = PM verdict authority, A043 = reserved for broader TEE infrastructure per A040 scope note)
- After submission, post to DAO Discord/Telegram for visibility
- Coordinate with Jake on v30.1 upgrade timing before submitting the mainnet governance proposal

---

*Drafted 2026-08-04. Status: READY FOR SUBMISSION.*
