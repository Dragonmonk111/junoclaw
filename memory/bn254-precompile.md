# `memory/bn254-precompile.md`

## Summary (3 lines)

We forked `wasmvm` to expose three BN254 host functions (`bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality`) so that Groth16 verification on Juno drops from ~371K SDK gas (pure-arkworks) to ~203K SDK gas (precompile). The patches live in `wasmvm-fork/patches/v2.2.7/` (10 numbered files) and target `cosmwasm` v2.2.7 + `wasmvm` v2.2.4. Track B is the forward-port to `cosmwasm` v3.0.x + `wasmvm` v3.0.4 driven by Juno PR #1202 pinning v3.

## Key facts

| Item | Value |
|---|---|
| Speedup | **1.823×** (370,600 → 203,266 SDK gas, σ=0 across 5 samples) |
| Differential test | 1000 random proofs, identical accept/reject across both backends (`wasmvm-fork/BUILD_AND_TEST.md`) |
| Devnet | `junoclaw-bn254-1`, container `junoclaw-bn254-devnet` |
| Pure contract code_id | 1 |
| Precompile contract code_id | 2 |
| Patches | `wasmvm-fork/patches/v2.2.7/{00..09}-*.patch` |
| Track B target tag | `cosmwasm` v3.0.1 (pinned by `wasmvm` v3.0.4 → pinned by Juno v30 via PR #1202) |
| Track B day-1 drift | 7 CLEAN / 2 3-way-OK / 1 reanchor (`01-cosmwasm-std.imports.rs.patch`) |
| Tests passing | 22/22 `cosmwasm-crypto-bn254`; 311+ `cosmwasm-vm` |

## Full context

### What

Three host functions that mirror the EIP-196/EIP-197 precompiles of Ethereum:

- `bn254_add(input_ptr: u32, out_ptr: u32) -> u32` — point addition on BN254 G1.
- `bn254_scalar_mul(input_ptr: u32, out_ptr: u32) -> u32` — scalar multiplication on G1.
- `bn254_pairing_equality(input_ptr: u32) -> u32` — multi-pair pairing equality check (returns 1 if pairing equality holds, 0 otherwise).

Surface gated behind the `cosmwasm_2_3` capability — calling on a chain that doesn't enable it surfaces as a "missing import" at contract load. Standard cw capability-gating.

### Why

Groth16 verification involves 4 pairings + a linear combination of G1 points (the `vk_x` accumulation). Each pairing in arkworks (pure-Wasm) costs ~80K SDK gas on Juno because BN254 field arithmetic is ~3× slower than secp256k1 in Wasm. With a host-side native impl, each pairing drops to ~34K SDK gas (~2.4× speedup per pairing).

Aggregate: 370,600 → 203,266 = 1.82× system-level speedup. Tested by deploying the same .wasm built with and without the `bn254-precompile` cargo feature side-by-side on the patched devnet and asking each to verify 1000 random proofs.

### Where

- **Patches:** `wasmvm-fork/patches/v2.2.7/`. 10 numbered patches against cosmwasm 2.2.7 + wasmvm 2.2.4. README.md in that directory describes each.
- **Reference impl:** `wasmvm-fork/cosmwasm-crypto-bn254/`. New crate added by patch 09. ~52K bytes of arkworks-backed BN254 ops with subgroup checks and constant-time guards.
- **Guest-side shim:** `wasmvm-fork/cosmwasm-std-bn254-ext/`. Contract-callable wrappers around the host fns; pre-allocates output Regions and handles error codes.
- **Drop-in to a contract:** `cargo build --features bn254-precompile` against any contract that depends on `cosmwasm-std-bn254-ext`. The dispatch is feature-gated so the same source compiles to either backend.

### Status

- ✅ **Track A** (cosmwasm v2.2.7 + wasmvm v2.2.4): patches applied, devnet running, gas measured, differential test green.
- 🚧 **Track B** (cosmwasm v3.0.1 + wasmvm v3.0.4): day-1 baseline check complete (drift report at [`track-b-forward-port.md`](./track-b-forward-port.md)). 7/10 patches CLEAN; needs ~1.5-2 days more focused work.
- ⏳ **Upstream issues:** drafts in `docs/UPSTREAM_ISSUE_DRAFTS.md`. Issue 1 paste-block ready in `docs/CMW_ISSUE1_PASTE.md`. Awaiting user go-ahead to open.

## Cross-references

- [`docs/BN254_PRECOMPILE_CASE.md`](../docs/BN254_PRECOMPILE_CASE.md) — long-form pitch / case study.
- [`docs/BN254_PRECOMPILE_INDEX.md`](../docs/BN254_PRECOMPILE_INDEX.md) — index of all BN254 docs.
- [`docs/ADR-001-BN254-PRECOMPILE.md`](../docs/ADR-001-BN254-PRECOMPILE.md) — formal architectural-decision record.
- [`wasmvm-fork/BUILD_AND_TEST.md`](../wasmvm-fork/BUILD_AND_TEST.md) — devnet bring-up + differential-test runbook.
- [`wasmvm-fork/patches/v2.2.7/README.md`](../wasmvm-fork/patches/v2.2.7/README.md) — patch-set manifest.
- [`docs/HACKMD_BN254_PROPOSAL.md`](../docs/HACKMD_BN254_PROPOSAL.md) — governance-prop markdown body.
- [`docs/CMW_ISSUE1_PASTE.md`](../docs/CMW_ISSUE1_PASTE.md) — paste-ready CosmWasm issue body.
- [`memory/track-b-forward-port.md`](./track-b-forward-port.md) — sibling memory file: Track B status and drift map.
- [`contracts/zk-verifier/DETERMINISTIC_AUDIT.md`](../contracts/zk-verifier/DETERMINISTIC_AUDIT.md) — consumer of the precompile; performance numbers reproduced there.

---

*Apache-2.0.*
