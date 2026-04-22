# zk-verifier — precompile build

This contract ships with two build flavours. The default build is what
already runs on Juno uni-7 at
`juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` and
uses pure-Wasm arkworks for the full Groth16 verification (~371 486
gas). The `bn254-precompile` feature flag produces a second .wasm that
delegates the pairing check and the public-input linear combination to
the three BN254 host functions added by `wasmvm-fork/` (~187 000 gas
target — a ~2× reduction matching EIP-1108).

## Quick build recipe

```bash
# Default build (works on any CosmWasm 2.x chain).
cargo build --release --target wasm32-unknown-unknown -p zk-verifier
# -> target/wasm32-unknown-unknown/release/zk_verifier.wasm

# Precompile build (requires a chain with the BN254 host functions
# registered — see ../../wasmvm-fork/patches/).
cargo build --release --target wasm32-unknown-unknown -p zk-verifier \
    --features bn254-precompile
# -> target/wasm32-unknown-unknown/release/zk_verifier.wasm
#    (same output path — differentiate at deploy time with a suffix)
```

Rename the precompile artefact immediately so the two flavours are
distinguishable downstream:

```bash
cp target/wasm32-unknown-unknown/release/zk_verifier.wasm \
   target/wasm32-unknown-unknown/release/zk_verifier_bn254_precompile.wasm
```

## How the feature flag changes behaviour

Every other message shape stays identical. The only runtime difference
is inside `execute_verify_proof`, where `bn254_backend::verify_groth16`
picks between:

| Feature flag        | Backend                                                                 |
|---------------------|-------------------------------------------------------------------------|
| **off** (default)   | `ark_groth16::Groth16::<Bn254>::verify_proof` — exactly today's path.   |
| **on**              | 4-pair `bn254_pairing_equality` + host-side lincomb (ECMUL + ECADD).    |

Both paths must produce bit-identical accept/reject decisions for any
input; the differential test in `wasmvm-fork/BUILD_AND_TEST.md` is the
acceptance criterion.

## Deploying the precompile variant

1. Upload to a chain running the patched `wasmvm` (see
   `../../devnet/` for an ephemeral single-validator harness):
   ```bash
   junod tx wasm store zk_verifier_bn254_precompile.wasm \
       --from admin --gas auto --gas-adjustment 1.3 -y
   ```
2. Instantiate with the same `InstantiateMsg` as the default variant.
3. Store the same verification key via `StoreVk`.
4. Run `VerifyProof` with a proof produced by the same circuit. A
   successful verification alongside a **lower** recorded gas figure is
   the demonstration the governance proposal points at.

## What happens on a non-patched chain

If the precompile .wasm is uploaded to a stock CosmWasm chain (no host
functions registered), contract instantiation fails at load time with
`Error: unresolved import "bn254_add"`. This is the correct and
intended behaviour — it's the VM saying "this capability isn't
available on this chain," and it prevents the contract from silently
falling back to a less-secure path.

## Why both backends live in one crate

Two reasons:

1. **Testing.** The default path is exercised by every `cargo test -p
   zk-verifier` run; the precompile path is exercised by the devnet
   differential test. If they were separate crates, it would be harder
   to keep their deserialization logic in sync.
2. **Migration story.** Once Juno ships the precompile via governance,
   the existing contract instance can be migrated to the precompile
   build without a code-id change — the `MigrateMsg` is unchanged.

## Related files

- `src/bn254_backend.rs` — dispatch implementation (both paths)
- `src/contract.rs` — single call site into the backend
- `../../wasmvm-fork/cosmwasm-std-bn254-ext/` — the guest-side shim crate
- `../../wasmvm-fork/patches/` — the upstream diffs that make the precompile
  .wasm loadable
- `../../docs/BN254_PRECOMPILE_CASE.md` — gas analysis
- `../../docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md` — on-chain proposal draft
  (created in a later step of the work plan)
