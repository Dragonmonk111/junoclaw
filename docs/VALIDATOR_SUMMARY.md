# JunoClaw — Validator Summary

> Short version for validators who don't read long proposals.
> This is the TL;DR you share in Discord, Telegram, and DMs.

---

## One Line

JunoClaw revives Junoswap with AI-verified swaps, runs 10 autonomous chain monitoring workflows on Akash, and governs everything through on-chain DAO proposals — all built on Juno, all open source.

---

## What It Does (30 seconds)

- **Junoswap v2 is back.** Factory + 2 trading pairs deployed on uni-7. Every swap verified by an off-chain agent in ~9 seconds (3 blocks at ~3s each). The agent recomputes the math independently and attests correctness on-chain. If the swap was manipulated, it flags it.

- **10 chain monitoring workflows running autonomously.** Swap verification, whale detection, governance attack detection, contract migration watchdog, IBC channel health, pool health, price attestation, sortition, outcome verification, data verification. No human in the loop.

- **Hardware-attested.** The verification code runs inside Intel SGX enclaves (TEE). The hardware signs the output — not just the operator. Proven on testnet: TX `6EA1AE79...` on uni-7.

- **Runs on Akash.** Three containers (WAVS operator + aggregator + IPFS) on decentralized compute. US$7.85/month. No AWS. No single point of failure.

- **DAO governed.** 13-member DAO with 51% quorum for normal proposals, and 67% supermajority for constitutional proposals (`CodeUpgrade` + `WeightChange`). Built on the same budding model as Juno's own fairdrop — distribute power, don't hoard it.

---

## What's Being Asked

This is a **signaling proposal**. No code execution. No community pool funds.

A yes means builders are shipping on Juno and the community sees it. That's it.

---

## Verify It Yourself

| What | Link |
|------|------|
| Code | https://github.com/Dragonmonk111/junoclaw |
| TEE Attestation TX | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` (uni-7) |
| Contract | `juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6` (uni-7) |
| WAVS Operator | http://provider.akash-palmito.org:31812 |
| Junoswap Factory | `juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh` (uni-7) |
| License | Apache 2.0 |
| Tests | 34 passing |

---

## Who

VairagyaNodes — staking Juno since December 30th, 2021. Validator. Builder.

---

## Discord / Telegram Copypaste

```
JunoClaw — signaling proposal on juno-1

Revives Junoswap with AI-verified swaps. 10 autonomous chain monitoring 
workflows running on Akash ($7.85/mo). TEE hardware-attested. DAO governed 
with 67% supermajority for constitutional proposals (CodeUpgrade + WeightChange). No funds requested.

All code + proofs: github.com/Dragonmonk111/junoclaw
TEE proof TX: 6EA1AE79... (uni-7)
WAVS operator: provider.akash-palmito.org:31812
```
