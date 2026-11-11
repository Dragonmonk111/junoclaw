# ZK Readout Spec — J-Lens deterministic readout proofs

Status: **v0 live on g1** (2026-11-11). Pure-Wasm verifier path.
Precompile path (bn254 host functions) is the later gas optimization.

## What shipped

| Piece | Where | On-chain |
|---|---|---|
| Circuit `jlens-readout` (DIM=8 fixture) | `circuits/jlens-readout` | — |
| Off-chain prover `gen-proof` | `circuits/jlens-readout/src/bin/gen-proof.rs` | — |
| Generic Groth16/BN254 verifier contract | `contracts/zk-verifier` (code_id 30) | `juno1wwvm5yqd4sl2vyj6tdgxsq5kem0hq7knml5c809u6thu2ygxtmjsmr3uhw` |
| VK stored | `StoreVk` tx `FB0A77A5…` | height 5,428,945 |
| Proof verified | `VerifyProof` tx `6323403E…` | height 5,429,036 (`last_verify.verified=true`) |
| Tampered inputs rejected | `proof verification failed` | 339,512 gas used |

## The statement (v0)

Public inputs (allocation order):
1. `merkle_root` — batch tree the readout leaf is committed under
2. `bank_commitment` — H(bank direction ‖ threshold)
3. `score_out` — recorded dot-product score
4. `integrity_out` — recorded integrity (SCALE − score)

Private witness: activation vector `a[0..8]`, bank direction `d[0..8]`,
threshold, Merkle path + bits.

Constraints:
1. `leaf = H(a)` walks `path` to `merkle_root` (MiMC, same gadget family as
   sensor-safety/intent-safety)
2. `bank_commitment = H(d ‖ threshold)`
3. `score_out = Σᵢ aᵢ·dᵢ` (one R1CS constraint per product)
4. `integrity_out = SCALE − score_out`

## Honest scope — what v0 does NOT prove

- **Not full inference.** The proof covers the deterministic readout
  arithmetic only, never the forward pass.
- **Not threshold/detection comparison.** `score >= threshold` needs a
  range gadget (next increment).
- **Not the canonical leaf format.** Production must pin `leaf` to the
  `jlens.integrity.v1` canonical int8 bytes — currently a native Fr leaf
  inside the circuit, not the on-chain activation_root byte encoding.
- **DIM=8 fixture.** Real probe projections are wider; DIM is a const.
- **Not zero-knowledge-by-need.** Activations/bank are private witnesses
  but they are public artifacts anyway — the value is succinctness
  (verify in ~ms without re-running the probe), not hiding.
- **Verification backend**: pure-Wasm arkworks (~340k gas on g1). The
  `bn254-precompile` build (~77k, host pairing) requires the patched
  wasmvm — deploy when the chain advertises `cosmwasm_2_3`.

## Next increments (ordered)

1. Range gadget → detection flag as public output (`score >= threshold`)
2. Pin leaf to `jlens.integrity.v1` canonical bytes (bridge to the
   anchored activation_root)
3. Court fast-path: `resolve_with_proof` accepting a valid ZK verify
   instead of committee votes
4. Multi-concept readout (detection set), production DIM
5. Precompile build when g1 wasmvm advertises `cosmwasm_2_3`

## Repro

```bash
# artifacts
cargo run -p jlens-readout-circuit --bin gen-proof   # vk/proof/inputs b64

# on-chain (contract-admin driver, wallet-store backed)
npx tsx src/contract-admin.ts --wallet attest-gw-test \
  --exec <verifier> --msg '{"store_vk":{"vk_base64":"..."}}'
npx tsx src/contract-admin.ts --wallet attest-gw-test \
  --exec <verifier> --msg '{"verify_proof":{"proof_base64":"...","public_inputs_base64":"..."}}'
npx tsx src/contract-admin.ts --query <verifier> --qmsg '{"last_verify":{}}'
```
