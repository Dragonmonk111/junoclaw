---
title: Builders
description: Build on JunoClaw — CosmWasm contracts, gRPC interfaces, on-chain post-quantum and ZK verification.
---

JunoClaw runs **CosmWasm** contracts behind a **Cosmos-style gRPC** interface. If you've built on Juno, most of what you know carries over. The differences are in consensus and cryptography, not in the contract model.

## What's available today (devnet)

| Surface | Status |
|---|---|
| CosmWasm store / instantiate / execute / query | Running |
| gRPC `BroadcastTx` | Running |
| gRPC `GetTx`: height, code, gas, events | Running |
| gRPC `Simulate`: full execution on a scratch overlay, real gas, zero writes | Running |
| Bank + wasm queries via Cosmos query dispatch | Running |
| Fee floor | `min_gas_price = 0.001ujclaw` (enforced at `check_tx`) |
| Tx auth | secp256k1, plus **hybrid secp256k1 + MAYO** accounts (committed on devnet) |
| In-contract MAYO-1/2/3/5 and Groth16 verification | Running |
| IBC: BLS light client, ICS-20 to a local Osmosis | Demonstrated on devnet |

## Things that work differently

- **Strict sequence admission.** The mempool rejects both stale and future nonces, so bursts from one sender have to arrive in order.
- **Finality is instant.** A tx included in a finalized block is final. You don't need confirmation counts.
- **Block time is deterministic.** It comes from genesis time and the consensus view, at about 1 s per block.
- **Wallet integrations are not ready yet.** Keplr/cosmjs compatibility through a CometBFT-RPC gateway is planned for G1→G2. Use gRPC for now.

## Measured gas reference

| Operation | Gas |
|---|---|
| MAYO-2 verify (in-contract) | ~309k |
| MAYO-3 verify | ~400k |
| MAYO-5 verify | ~726k |
| Hybrid secp256k1 + MAYO spend | ~110k |

## Get started

1. Read the [lightpaper](/docs/lightpaper/) and [security model](/docs/reference/security/).
2. Browse the contracts and tooling: [`Dragonmonk111/junoclaw`](https://github.com/Dragonmonk111/junoclaw).
3. Browse the chain: [`Dragonmonk111/layer-sdk` @ `commonware`](https://github.com/Dragonmonk111/layer-sdk/tree/commonware).
4. Say what you want to build in [Telegram](https://t.me/junoclaw) or on the [Buzz](https://buzz.junoclaw.xyz) dev channel. Builder access to the G1 testnet is arranged directly.

Code is dual-licensed **MIT OR Apache-2.0**.
