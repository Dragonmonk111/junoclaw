# Upstream tracking for Aegis / JunoClaw PQC work

Last updated: 2026-06-22

## Why this exists

Project Aegis and JunoClaw currently depend on experimental forks:

- `Dragonmonk111/cometbft` (aegis-phase-cf-hybrid)
- `Dragonmonk111/cosmos-sdk` (aegis-phase-d3-hybrid)
- `junoclaw/wasmvm-fork` (patches for BN254 + MAYO + ML-DSA host functions)

This file tracks the upstream issues and PRs where we are trying to land (or align with) the underlying changes so that the fork maintenance burden shrinks over time and the PQC work stays compatible with new Juno / CosmWasm / CometBFT releases.

## Active upstream items

| # | Repo | Item | Title / topic | Status | Why it matters | Next action | Next check |
|---|------|------|---------------|--------|----------------|-------------|------------|
| 1 | CosmWasm/cosmwasm | Issue #2685 | Proposal: BN254 (alt_bn128) host functions for Groth16 verification | Open | BN254 precompile is the ZK verification path in JunoClaw. Landing it upstream removes the need to maintain a custom `cosmwasm-std-bn254-ext` crate and a patched wasmvm. | Wait for maintainer feedback on (a) ABI shape, (b) `cosmwasm_2_3` feature flag, (c) gas schedule. Open PR only after shape is confirmed. | After CosmWasm v3.0.6 release or maintainer reply. |
| 2 | CosmWasm/wasmvm | Issue #735 | Question: should BN254 host functions also expose Go-side wrappers, or follow the BLS12-381 precedent of staying VM-internal? | Open | Companion to cosmwasm#2685. The Go-side wiring in wasmvm determines whether our `do_bn254_verify` host function can be upstreamed as a clean patch or needs a different integration pattern. | Coordinate with wasmvm maintainers; update `docs/WASMVM_BN254_PR_DESCRIPTION.md` once direction is clear. | After cosmwasm#2685 feedback. |
| 3 | CosmosContracts/juno | PR #1202 | v30 upgrade | Open (merge pending) | v30 bumps Cosmos SDK, CometBFT, IBC-Go and wasmd/wasmvm versions. Our Aegis fork patches (hybrid consensus, hybrid accounts, MsgRotateConsKey) must be rebased onto the final v30 base. | Monitor merge; run patch-applicability checks with `wasmvm-fork/patches/check-baseline.sh`; rebase Aegis branches onto v30 tags once merged. | After PR #1202 merges to `main`. |

## How we track

1. **GitHub notifications** — watch the three upstream items above.
2. **Weekly patch-applicability check** — run `wasmvm-fork/patches/check-baseline.sh` and a similar check for the CometBFT / Cosmos SDK forks to see if upstream releases still apply clean.
3. **Release notes scan** — review CosmWasm, wasmvm, CometBFT, Cosmos SDK, and wasmd release notes for changes that touch:
   - Public-key / signature interfaces
   - P2P transport / secret connection
   - Validator set / consensus pubkey handling
   - VM host-function registration
   - Capability / feature flags
4. **Update this file** — whenever an item changes status, add a dated entry under the item and bump the `Last updated` line.

## Decisions pending

- **ML-DSA upstreaming path**: The Project Aegis consensus migration (hybrid Ed25519+ML-DSA-44) is currently a fork-first strategy. Upstreaming will depend on NIST standardization momentum and on-chain gas measurements. No separate upstream issue exists yet; this file will be updated once a tracking issue or discussion is opened.
- **MAYO app-layer status**: MAYO remains a JunoClaw app-layer feature. Upstreaming is not currently planned because MAYO is not a finalized NIST standard and the implementation is intentionally a CosmWasm contract, not a chain-level change.
- **BN254 vs BLS12-381**: We are waiting for upstream guidance on whether to expose the pairing-friendly curve as a public host function or keep it internal. Our preference is a public, capability-gated host function (`bn254_add`, `bn254_scalar_mul`, `bn254_pairing`) so that any contract can use it.

## Notes

- The cosmwasm team is currently in a maintenance-transition period. We explicitly chose to open an issue for discussion before submitting a PR so the design can be shaped with maintainers rather than forcing a rework later.
- The v30 PR (#1202) already incorporates our earlier review feedback on `voting-snapshot` (sparse delegator bug, retention window default, prune interval). Tracking it ensures our Aegis patches land cleanly on top of those fixes.
