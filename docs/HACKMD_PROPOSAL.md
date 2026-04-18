# JunoClaw — Verifiable AI Agents with TEE-Attested Junoswap Revival on Juno

*Signaling Proposal for Juno Network (juno-1)*

> **✅ LIVE ON MAINNET — Proposal #373**
> Vote: https://ping.pub/juno/gov/373
> TX: `FAE98E6EAD6C23440FF614DE5973FA8D9A109FF68CDA6991031E1D8598DB3C9C`
> Submitted: March 19, 2026

---

## Summary

JunoClaw is an open-source agentic AI platform built natively on Juno Network. This signaling proposal asks the community to recognize JunoClaw as Juno ecosystem infrastructure and endorse the deployment of its contracts on mainnet.

**This proposal does not execute code. This proposal does not request community pool funds.**

> *"Who knows, AI might actually be able to get JunoSwap working."* — Jake Hartnell, Juno Telegram, March 18 2026

---

## What Exists Today (Testnet uni-7)

Everything described below is deployed, tested, and verifiable on Juno testnet (uni-7) as of March 18, 2026.

| Component | Status | Reference |
|-----------|--------|-----------|
| agent-company v3 contract | Live, 5 proposals executed | Code ID 63, [contract](https://testnet.ping.pub/juno/account/juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6) |
| Junoswap v2 factory | Live | Code ID 61, [contract](https://testnet.ping.pub/juno/account/juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh) |
| Junoswap v2 pair (JUNOX/USDC) | Live | Code ID 60, [contract](https://testnet.ping.pub/juno/account/juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98) |
| Junoswap v2 pair (JUNOX/STAKE) | Live | Code ID 60, [contract](https://testnet.ping.pub/juno/account/juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s) |
| TEE hardware attestation | Proven (Intel SGX) | [TX 6EA1AE79...](https://testnet.ping.pub/juno/tx/6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22) |
| Akash infrastructure | 4 containers (software mode) | ~US$8.76/month, 63.77 AKT funded |
| warg-registry on Akash | Self-publishing, verified | Zero cloud dependency |
| WASI verification component | Published (494KB) | [ghcr.io/dragonmonk111/warg-registry](https://github.com/Dragonmonk111/junoclaw/pkgs/container/warg-registry) |
| Content hash (component) | `sha256:b40d3fcaf40e...` | Matches across Azure TEE + Akash deployments |
| Unit tests | 34 passing | Contracts + governance + sortition |
| Source code | Apache 2.0 | [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |

---

## Three Integrations

### 1. Junoswap for Agents (Clean Rewrite)

The original Junoswap was abandoned. JunoClaw includes **Junoswap v2** — a clean rewrite from scratch. Two contracts, Apache 2.0 licensed. This is **not** a fork of the original Junoswap code and is **not** affiliated with the original Junoswap team.

- **Factory** (code ID 61): Pair creation, fee config (30 bps default), duplicate protection, governance integration via agent-company contract reference
- **Pair** (code ID 60): XYK constant-product AMM. Fee deducted from offer side. Return = `offer_after_fee × return_reserve / (offer_reserve + offer_after_fee)`. Slippage protection via `min_return`. Every swap emits `wasm-swap` event with 12 attributes for WAVS verification
- **Size**: 438 lines (pair) + 209 lines (factory). 34 unit tests. Auditable in an afternoon.

The factory is wired into the agent-company DAO governance contract. Every swap triggers a WAVS verification event. The operator independently recomputes the expected output and attests correctness on-chain.

### 2. WAVS TEE Integration (Proven)

[WAVS](https://layer.xyz) (Layer.xyz) provides hardware-attested off-chain compute for Cosmos chains. Founded by Ethan Frey (CosmWasm creator) and Jake Hartnell (Juno co-founder).

JunoClaw's 494KB WASI verification component has been **proven to run inside an Intel SGX enclave** on an Azure DCsv3 instance. When running inside a TEE:
- The code that ran is the code that was published (no tampering)
- The attestation is hardware-grade, not just a signature from a key

**Proof**: Proposal 4 on uni-7 is the first hardware-attested WAVS result submitted to a Cosmos chain. [TX 6EA1AE79...](https://testnet.ping.pub/juno/tx/6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22), block 11735127.

> **Status**: TEE capability is proven. The Azure VM used for this proof has been deleted. TEE attestation in production will come from **validator sidecars** — see Architecture below.

### 3. Akash Decentralized Infrastructure

The operator coordination layer runs on [Akash Network](https://akash.network) — a permissionless, decentralized compute marketplace. Four containers handle event monitoring, component publishing, aggregation, and content storage:

| Container | Role |
|-----------|------|
| WAVS operator | Monitors Juno events, executes WASI verification component |
| WAVS aggregator | Collects and batches attestation results |
| warg-registry | Publishes and serves the verification component (`junoclaw:verifier v0.1.0`) |
| IPFS | Content-addressed storage for component artifacts |

- **Cost**: ~US$8.76/month
- **Budget**: 63.77 AKT (covers ~3-5 months)
- **No single company can shut it down**

The registry self-publishes on every container start. Zero centralized cloud dependency. Confirmed working as of March 18, 2026.

> **Important distinction**: Akash does not currently offer TEE-capable instances (SGX/SEV). The Akash deployment handles **coordination and verification logic in software mode**. Hardware-grade attestation requires TEE hardware, which is the role of validator sidecars in the target architecture.

### Target Architecture: Hybrid (Akash + Validator TEE)

The production design separates concerns:

- **Akash** → Always-on infrastructure: registry, aggregator, IPFS, event monitoring. Cheap, decentralized, censorship-resistant.
- **Validator sidecars** → TEE attestation: Each participating validator runs the WASI component inside their own SGX/SEV hardware. Attestations are distributed across the validator set — no single point of trust.

This means:
1. No single operator holds the keys to verification
2. TEE attestations come from multiple independent hardware enclaves
3. Infrastructure (Akash) and attestation (validators) are independently decentralized
4. If Akash goes down, validators still attest. If a validator goes down, others cover.

The validator sidecar docker-compose is ready. The WASI component is the same 494KB binary — it runs identically on Akash (software) and inside a validator's SGX enclave (hardware-attested).

---

## WAVS Verification Workflows

The WAVS operator runs **5 autonomous verification workflows** — no human in the loop. Each workflow follows the same pattern: trigger event on Juno → operator catches it → WASI component verifies → attestation hash submitted back to contract.

| # | Workflow | Trigger | What It Verifies |
|---|---------|---------|-----------------|
| 1 | **Swap Verification** | `wasm-swap` event on any Junoswap pair | Recomputes XYK math, checks invariant k, measures price impact, flags manipulation (>5% slippage) |
| 2 | **Sortition (Random Jury)** | `wasm-sortition_request` event | Fetches drand beacon randomness, submits on-chain. Contract runs Fisher-Yates shuffle with SHA-256 sub-randomness. Deterministic, verifiable. |
| 3 | **Outcome Verification** | `wasm-wavs_push` event (data tasks) | Verifies resolution criteria against external sources. TEE guarantees code wasn't tampered with. |
| 4 | **Governance Watch** | `wasm-wavs_push` event (governance) | Monitors proposals for anomalies — unusual voting patterns, quorum manipulation, rapid-fire submissions. |
| 5 | **Migration Watch** | `wasm-wavs_push` event (migration) | Detects contract migration events. Verifies new code_id against known-good hashes. Flags unauthorized migrations. |

Every attestation is permanent and queryable on-chain.

---

## Governance Architecture: Genesis → 13 Buds

JunoClaw uses a "budding" governance model designed to prevent centralization:

1. **Genesis phase** (current): Genesis address holds 100% weight. Used only for initial deployment and setup.
2. **Budding**: Genesis submits a `WeightChange` proposal distributing weight to 13 initial DAO members ("buds"). Each bud receives 769/10000 weight. Genesis retains 3/10000 (symbolic) plus wasmd admin key (emergency-only, e.g. critical bug migration).
3. **Self-governance**: Normal proposals require 51% quorum (7 of 13 buds). Constitutional proposals (`CodeUpgrade` and `WeightChange`) require 67% supermajority (9 of 13 buds). Further members can be added via subsequent `WeightChange` proposals.

**Once the 13 buds are active, Genesis loses voting power. The DAO self-governs. No single actor can push changes unilaterally.**

The genesis address for mainnet deployment: `juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m`

---

## If This Proposal Passes

Passing signals that the Juno community recognizes JunoClaw and endorses the following plan:

1. **Genesis deploys mainnet contracts**: agent-company, Junoswap factory + pairs, escrow, task-ledger, agent-registry
2. **Genesis wires Akash infrastructure**: Operator coordination, registry, aggregator, IPFS — all on Akash (software mode, live today)
3. **Genesis distributes TEE attestation**: Validator sidecar proposal — interested validators run the WASI component inside their own TEE hardware, producing independent hardware-grade attestations
4. **Genesis buds into 13**: DAO governance begins, Genesis loses voting power
5. **Ongoing**: DAO governs itself — proposals, constitutional changes (`CodeUpgrade` + `WeightChange`), new members, WAVS verification

The timeline is with Genesis. This is a seed planted in Juno soil, not a corporate roadmap.

---

## What We Are Asking For

1. Recognize JunoClaw as Juno ecosystem infrastructure
2. Support the Junoswap revival through verifiable agent verification
3. Endorse Akash as the decentralized coordination layer for JunoClaw's operator infrastructure
4. Acknowledge the constitutional governance framework (`CodeUpgrade` + `WeightChange`) with 67% supermajority
5. Support the validator sidecar proposal — distributing TEE attestation across Juno's validator set for hardware-grade trust

---

## Risks and Limitations

Transparency requires stating what this is not:

- **Not audited**: The contracts have 34 unit tests but have not undergone a formal third-party security audit. An audit is planned post-mainnet deployment using DAO funds or community support.
- **Testnet only**: Everything described is on uni-7. Mainnet deployment follows if this proposal passes.
- **Memory-based registry**: The warg component registry uses in-memory storage. If the Akash container restarts, the component is re-published automatically from the baked-in WASM binary (494KB, ~10s startup). The content hash (`sha256:b40d3fca...`) is deterministic.
- **No liquidity yet**: Junoswap v2 pairs exist but have no real liquidity. Liquidity provision is a post-deployment community effort.
- **Single operator**: Currently one WAVS operator instance on Akash. The architecture supports multiple operators, and the validator sidecar proposal (item 5 above) would distribute attestation across the validator set.
- **Solo developer**: JunoClaw is built by one person. The 13-bud governance model is designed to fix this — the DAO can onboard contributors, fund audits, and distribute responsibility.
- **Experimental**: This is bleeding-edge infrastructure. Use at your own risk. The Apache 2.0 license applies.

---

## Who Is Proposing

**VairagyaNodes** — Juno staker since December 30, 2021. Validator candidate (unbonded, position 6). Built JunoClaw as a solo developer contribution to the Juno ecosystem.

- Validator: [VairagyaNodes on ping.pub](https://ping.pub/juno/staking/junovaloper1...)
- GitHub: [Dragonmonk111](https://github.com/Dragonmonk111)
- Discord: Active in Juno Discord (validator role)

---

## Links

| Resource | Link |
|----------|------|
| Source code (Apache 2.0) | https://github.com/Dragonmonk111/junoclaw |
| TEE Attestation TX (uni-7) | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| Agent-company contract (uni-7) | `juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6` |
| Junoswap Factory (uni-7) | `juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh` |
| Junoswap Pair JUNOX/USDC (uni-7) | `juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98` |
| Junoswap Pair JUNOX/STAKE (uni-7) | `juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s` |
| WASI Component (GHCR) | https://github.com/Dragonmonk111/junoclaw/pkgs/container/warg-registry |
| WAVS (Layer.xyz) | https://layer.xyz |
| Akash Network | https://akash.network |

---

## Status Log

| Date | Milestone |
|------|-----------|
| Mar 13 | Contracts deployed on uni-7 (agent-company, junoswap factory + pairs) |
| Mar 15 | WAVS operator live on Akash (3 containers) |
| Mar 16 | TEE hardware attestation proven (Intel SGX on Azure DCsv3) |
| Mar 17 | Docker image pushed to GHCR, Azure VM deleted, governance proposal drafted |
| Mar 18 | warg-registry self-publishing on Akash (4 containers), Akash-to-Akash confirmed |

---

*This document is the canonical reference for the JunoClaw governance proposal. Discussion and feedback welcome.*
