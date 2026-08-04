# ZK-Verifier Benchmark Results (Mainnet)

> Chain: juno-1 | Contract: juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p | Date: 2026-08-04T17:33:30.584Z

## Configuration

| Parameter | Value |
|-----------|-------|
| Gas price | 0.075ujuno |
| Samples | 3 |
| Variant | pure-Wasm (no BN254 precompile) |
| VK size | 396 chars (base64) |
| Proof size | 172 chars (base64) |

## Results

| Sample | Gas Used | Time (ms) | TX Hash |
|--------|----------|-----------|---------|
| 1 | 430696 | 3599 | 908EBC35F6DD573FF154AF1CA793C0969177FD23FA772CB93C537E7ADE91153E |
| 2 | 430477 | 3448 | 5228BA557B5FEA562D3DCA9C64F03D9725AA1312B3742AE395D6DEC3B2453668 |
| 3 | 431326 | 3542 | 060BB16C8148B98E9080B0F0036E881A9B1D291A8C6D6097D964B670E14D534A |

## Summary

- **Average gas**: 430833
- **Min gas**: 430477
- **Max gas**: 431326
- **Average time**: 3530ms
- **Est. cost per verify**: ~0.0323 ujuno

## Context

- Pure-Wasm Groth16 verifier (no BN254 host-function dependency)
- Baseline for comparison against BN254 precompile variant (Track B, post v30.1 upgrade)
- Expected precompile gas: ~203,000 (1.82x reduction from devnet benchmarks)
