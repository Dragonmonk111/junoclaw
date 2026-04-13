# zk-verifier — Groth16 BN254 PoC for CosmWasm

Proof-of-concept contract demonstrating **Groth16 zero-knowledge proof verification** over the **BN254 elliptic curve** in pure CosmWasm (no precompile required).

## Purpose

This contract exists to answer one question: **How expensive is BN254 pairing verification in pure Wasm?**

The answer makes the case for a BN254 precompile in Juno:

| Approach | Gas Cost | Relative |
|----------|----------|----------|
| SHA-256 hash check (current JunoClaw) | ~200K | 1x |
| Groth16 with BN254 precompile | ~187K | 0.9x |
| **Groth16 in pure CosmWasm (this contract)** | **~5–10M** | **25–50x** |

## Architecture

```
┌─────────────────────────────────┐
│          zk-verifier            │
│                                 │
│  StoreVk(vk_base64)            │  Admin stores the verification key once
│  VerifyProof(proof, inputs)     │  Anyone can submit proofs for verification
│  VkStatus() → has_vk, size     │  Query: is a VK loaded?
│  LastVerify() → bool, height   │  Query: last verification result
│                                 │
│  ┌──────────────────────┐       │
│  │  ark-groth16 0.5     │       │  Pure Rust BN254 pairing computation
│  │  ark-bn254 0.5       │       │  No C dependencies, no precompile
│  └──────────────────────┘       │
└─────────────────────────────────┘
```

## Build

```bash
# Build for native testing
cargo +stable build -p zk-verifier

# Build for CosmWasm deployment (wasm32)
cargo +stable build -p zk-verifier --target wasm32-unknown-unknown --release

# Optimize (requires wasm-opt)
wasm-opt -Oz --strip-debug --strip-producers \
  target/wasm32-unknown-unknown/release/zk_verifier.wasm \
  -o zk_verifier_optimized.wasm
```

## Test

```bash
# Run all 9 tests
cargo +stable test -p zk-verifier
```

### Test matrix

| Test | What it proves |
|------|---------------|
| `test_instantiate` | Contract initializes correctly |
| `test_store_vk_unauthorized` | Only admin can store VK |
| `test_verify_without_vk_fails` | Verification requires a stored VK |
| `test_full_groth16_verify` | End-to-end: setup → prove → verify (x²=y circuit) |
| `test_invalid_vk_bytes_rejected` | Garbage bytes rejected as VK |
| `test_wrong_public_input_rejected` | Correct proof + wrong inputs = rejected |
| `test_tampered_proof_rejected` | Bit-flipped proof bytes = rejected |
| `test_mismatched_vk_rejects_valid_proof` | Valid proof against wrong VK = rejected |
| `test_benchmark_verification_time` | Native CPU timing + estimated CosmWasm gas |

## Generate test proof data

For on-chain deployment, generate serialized VK + proof + public inputs:

```bash
cargo +stable run -p zk-verifier --example generate_proof
```

This outputs a JSON file to your system temp directory (e.g. `/tmp/groth16_proof.json` or `C:\Users\<you>\AppData\Local\Temp\groth16_proof.json`). Override with `PROOF_OUTPUT=./proof.json`.

## Deploy to uni-7

```bash
cd wavs/bridge
npx tsx src/deploy-zk-verifier.ts            # full deploy
npx tsx src/deploy-zk-verifier.ts --dry-run   # offline validation only
```

Full deploy requires `WAVS_OPERATOR_MNEMONIC` in `wavs/.env`. The script:
1. Uploads the optimized WASM
2. Instantiates the contract
3. Stores the verification key
4. Submits and verifies a Groth16 proof
5. Reports gas costs for each step

## Circuit

The PoC uses a trivial **SquareCircuit**: prove knowledge of `x` such that `x² = y`, where `y` is the public input.

```
Private witness: x = 3
Public input:    y = 9
Constraint:      x * x = y
```

For JunoClaw production use, the circuit would encode SHA-256 hash chain verification:
```
SHA256(COMPONENT_ID || task_type || data_hash) == attestation_hash
```

## Dependencies

- **ark-bn254 0.5** — BN254 curve implementation
- **ark-groth16 0.5** — Groth16 proving system
- **ark-ec 0.5** — Elliptic curve traits
- **ark-ff 0.5** — Finite field arithmetic
- **ark-serialize 0.5** — Canonical serialization
- **cosmwasm-std 2.2** — CosmWasm runtime

All arkworks crates compile to `wasm32-unknown-unknown` with default features.

## Related

- [`docs/BN254_PRECOMPILE_CASE.md`](../../docs/BN254_PRECOMPILE_CASE.md) — Full gas analysis and precompile proposal
- [CosmWasm Crypto API issue #751](https://github.com/CosmWasm/cosmwasm/issues/751) — BN254 listed as "Bonus Points"
- [EIP-196](https://eips.ethereum.org/EIPS/eip-196) / [EIP-197](https://eips.ethereum.org/EIPS/eip-197) — Ethereum BN254 precompiles
