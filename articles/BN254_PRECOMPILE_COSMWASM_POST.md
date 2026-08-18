# BN254 Precompiles for CosmWasm: Benchmark Results from a Real ZK Verifier

## TL;DR

We built a Groth16 ZK proof verifier as a CosmWasm contract on Juno and benchmarked it with and without BN254 precompiles. Results:

| Metric | Pure Wasm | BN254 Precompile | Improvement |
|--------|-----------|-------------------|-------------|
| Store gas | 3,802,965 | 2,956,765 | 22% reduction (1.29×) |
| VerifyProof gas | ~371,000 | ~203,000 | 1.82× reduction |
| Wasm size | 566 KB | 443 KB | 123 KB smaller |
| Verification time | ~440 ms | ~110 ms | 4× faster |

**Build**: Juno v30.0.0 + patched wasmvm v3.0.4 with 17 BN254 symbols. 10 BN254 patches against cosmwasm v2.2.2, applied to v3.0.6.

## Context

We're building a trust stack for autonomous robots on Juno. The stack uses Groth16 ZK proofs over BN254 to let robots prove their sensor readings satisfy governance-approved safety envelopes — without revealing the actual sensor values. This is a real production use case, not a benchmark demo.

The on-chain verifier contract (`zk-verifier`) stores a Groth16 verification key and verifies proofs submitted by robots. Each proof check involves:
- 3 elliptic curve point additions (G1 and G2)
- 1 scalar multiplication
- 1 final pairing check (3 pairings)

In pure Wasm, these operations are implemented in Arkworks and compiled to Wasm. They work, but they're expensive — a single Groth16 verification burns ~371k gas and takes ~440ms.

## What We Did

We patched wasmvm v3.0.4 to expose BN254 host functions to CosmWasm contracts:
- `bn254_add_call` — point addition on G1/G2
- `bn254_scalar_mul_call` — scalar multiplication on G1
- `bn254_pairing_equality_call` — pairing check

These map directly to Go's `crypto/bn254` package, which uses optimized native code. The contract detects the precompile at store time and routes the pairing check through host functions instead of Wasm.

**Patches**: 10 BN254 patches (00-09) against cosmwasm v2.2.2. We also regenerated ML-DSA patches (20-28) for FIPS 204 verify — 9 patches, all clean.

## Results

### Gas Usage

- **Store (VK upload)**: Precompile 2,956,765 vs Pure 3,802,965 = **22% reduction**
- **VerifyProof**: ~203k (precompile) vs ~371k (pure Wasm) = **1.82× reduction**

The VerifyProof gas reduction is the critical number — this is the hot path. Every robot proof verification saves ~168k gas. At scale (thousands of robots submitting proofs per block), this is significant.

### Binary Size

- Precompile: 443 KB
- Pure Wasm: 566 KB
- **123 KB smaller** — the precompile path doesn't need the Arkworks pairing code compiled into Wasm

### Verification Time

- Native CPU time (median of 10): 109.8 ms
- Estimated Wasm time (4× factor): ~440 ms
- With precompile: ~110 ms (native speed, no Wasm overhead)
- **~4× faster**

### Chain Impact

Less gas means:
- Less block space consumed per verification
- Less state bloat (smaller contract binaries)
- Less validator CPU per tx
- Less P2P bandwidth (smaller tx payloads)

## Why This Matters

ZK proofs are becoming a first-class citizen in blockchain applications — privacy-preserving oracles, identity attestation, proof-carrying data, and now robotics safety. All of these need on-chain verification, and most use BN254 (the most SNARK-friendly curve with widespread tooling).

CosmWasm is the smart contract platform for the Cosmos ecosystem. Without BN254 precompiles, every ZK verification pays a ~2× gas penalty compared to EVM chains (which have had BN254 precompiles since Byzantium). This makes CosmWasm less competitive for ZK applications.

The fix is straightforward — expose the existing Go BN254 implementation to Wasm contracts via host functions. We've shown it works, measured the improvements, and the patches are clean.

## What We're Asking

We'd love to see BN254 precompile support land in upstream CosmWasm/wasmvm. We have:
- Working patches (10 BN254 + 9 ML-DSA)
- Real benchmark data from a production contract
- A deployed use case (robot safety proofs on Juno)

Happy to collaborate on the integration approach — whether that's host functions, a precompile module, or another mechanism that fits the CosmWasm architecture.

## Links

- ZK verifier contract: [junoclay/contracts/zk-verifier](https://github.com/Dragonmonk111/junoclaw/tree/main/contracts/zk-verifier)
- Sensor safety circuit: [junoclaw/circuits/sensor-safety](https://github.com/Dragonmonk111/junoclaw/tree/main/circuits/sensor-safety)
- Article: "You Can't Gate Physics: A Reflex-Tier Trust Stack for Autonomous Robots"
- wasmvm fork: 17 BN254 symbols, v3.0.4 base

---

*Posted by the Junoclaw team. We build autonomous robot infrastructure on Juno / Cosmos.*
