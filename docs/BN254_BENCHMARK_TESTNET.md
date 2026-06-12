# ZK-Verifier Benchmark Results (Testnet)

> Chain: uni-7 | Contract: juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2 | Date: 2026-06-12T11:50:43.085Z

## Configuration

| Parameter | Value |
|-----------|-------|
| Gas price | 0.075ujunox |
| Samples | 3 |
| VK size | 396 chars (base64) |
| Proof size | 172 chars (base64) |

## Results

| Sample | Gas Used | Time (ms) | TX Hash |
|--------|----------|-----------|---------|
| 1 | 371129 | 3184 | 9BBE6A9D7978E57EB97E54A6D30FA09319DF744DDD8843D59696D7DCC9CE7A6B |
| 2 | 371129 | 3228 | 975EFCDC0AD28A522F36B53C2D261074992034EC904A192C7C7BED23363455F5 |
| 3 | 371129 | 3202 | D6AE8F318C25B3C3C034B6E2A8693ACD5356651D45C23384716C0B428386160A |

## Summary

- **Average gas**: 371129
- **Average time**: 3205ms
- **Est. cost per verify**: ~0.0278 ujunox

## Notes

- Pure wasm verifier (no BN254 precompile on uni-7)
- Compare with devnet precompile numbers when available
