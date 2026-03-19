# JunoClaw Closes the TEE Gap — Hardware-Attested Proofs on Juno

## The First WAVS Component Executed Inside an Intel SGX Enclave on Cosmos

---

**TL;DR** — We ran a WebAssembly verification component inside an Intel SGX hardware enclave, submitted the attestation hash to a live CosmWasm contract on Juno testnet, and proved it on-chain. Proposal 4 on `agent-company` is the first hardware-attested WAVS result in the Cosmos ecosystem.

---

### What happened

On March 17, 2026, JunoClaw's WASI verification component executed inside an Intel SGX Trusted Execution Environment on an Azure DCsv3 confidential VM. The component:

1. Received trigger data matching an `outcome_verify` task from proposal 4
2. Computed a SHA-256 attestation hash over the proposal's question, resolution criteria, and market ID
3. Returned a signed `VerificationResult` from inside the enclave
4. The attestation was submitted to the `agent-company` contract on Juno testnet (uni-7)

The entire pipeline — from hardware enclave to on-chain proof — completed in a single session.

### The proof

| Field | Value |
|-------|-------|
| **Chain** | Juno testnet (uni-7) |
| **Contract** | `juno1k8dxll...stj85k6` |
| **Proposal** | 4 |
| **Task type** | `outcome_verify` |
| **Attestation TX** | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| **Data hash** | `9d0f7354205de1fcaa41a8642ee704ed8e6201bdf8e4951b36923499a7367a3b` |
| **Attestation hash** | `945a53c5c1aab2e99432e659d47633da491fffc399d95cbce66b8e88fae5c0e8` |
| **Block** | 11,735,127 |
| **Hardware** | Intel SGX (Azure Standard_DC2s_v3) |
| **SGX devices** | `/dev/sgx_enclave` + `/dev/sgx_provision` |

Anyone can verify this on-chain by querying the contract:

```
junod query wasm contract-state smart juno1k8dxll...stj85k6 \
  '{"get_attestation":{"proposal_id":4}}'
```

### Why this matters

**Trust without trusted parties.** When a DAO proposal passes and needs off-chain verification — fetching data, running computations, checking outcomes — someone has to do that work. The question is: how do you trust the result?

Before today, JunoClaw had three trust levels:

1. **Manual attestation** (proposal 2) — a human runs the check, submits the hash. You trust the human.
2. **Autonomous local operator** (proposal 3) — a daemon watches the chain, computes hashes automatically. You trust the machine it runs on.
3. **TEE hardware attestation** (proposal 4) — a WASI component runs inside an SGX enclave. The hardware itself guarantees the code wasn't tampered with. You trust the silicon.

Level 3 is what WAVS (WebAssembly Verifiable Services) was built for. Jake Hartnell, co-founder of Juno and architect of WAVS at Layer.xyz, confirmed this is exactly how it's supposed to work: "WAVS TEEs already work — you just need to run WAVS inside a TEE."

That's what we did.

### The stack

```
┌─────────────────────────────────────────┐
│  Azure DCsv3 VM (Intel SGX hardware)    │
│  ┌───────────────────────────────────┐  │
│  │  Docker + WAVS operator stack     │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  SGX Enclave                │  │  │
│  │  │  ┌───────────────────────┐  │  │  │
│  │  │  │ junoclaw_wavs_component│  │  │  │
│  │  │  │ (WASI / wasm32-wasip2)│  │  │  │
│  │  │  └───────────────────────┘  │  │  │
│  │  └─────────────────────────────┘  │  │
│  │  wavs-cli exec                    │  │
│  │  --device /dev/sgx_enclave        │  │
│  │  --device /dev/sgx_provision      │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
         │ attestation_hash
         ▼
┌─────────────────────────────────────────┐
│  Juno testnet (uni-7)                   │
│  agent-company contract                 │
│  submit_attestation { proposal_id: 4 }  │
│  TX: 6EA1AE79...D26B22                  │
└─────────────────────────────────────────┘
```

**Components:**
- **WASI component** — Rust, compiled to `wasm32-wasip2`, implements `wavs:operator/wavs-world@=2.1.0`
- **WAVS runtime** — `ghcr.io/lay3rlabs/wavs:1.5.1` Docker image with `wavs-cli exec`
- **SGX enclave** — Intel Software Guard Extensions on Azure DC-series hardware
- **Bridge daemon** — TypeScript + CosmJS, submits attestation to Juno
- **Contract** — CosmWasm `agent-company` on uni-7, stores attestations permanently

### The journey

This didn't happen in one step. Here's the path:

**Phase 1–3** (March 13–15): Built the WASI component, deployed four CosmWasm contracts, wrote the bridge daemon.

**Phase 4** (March 16): Proved end-to-end on testnet with manual and autonomous attestations.

**Phase 4b** (March 16): Built a local operator that watches the chain, auto-detects executed proposals, computes hashes locally, and submits. Proven on proposal 3.

**Phase 5** (March 17): Deployed the WAVS operator stack on Azure DCsv3 with Intel SGX, executed the component inside the enclave, submitted the hardware-attested result for proposal 4.

**Phase 6** (March 17 — same day): Deployed the WAVS operator stack to Akash Network. Three containers (operator + aggregator + IPFS) running on decentralized compute at `http://provider.akash-palmito.org:31812`. Chain health confirmed: `Cosmos chain [cosmos:juno_testnet] is healthy`. Cost: US$7.85/month. No Azure, no AWS — fully decentralized infrastructure.

**Phase 7** (March 18): Chain Intelligence Module shipped. 6 autonomous verification workflows live on Akash: Swap Verification, Sortition, Outcome Verification, Governance Watch, Migration Watch, and dedicated JUNOX/USDC swap monitoring. Azure ChainIntelligenceVM deployed with warg component registry (systemd-managed, auto-publishes `junoclaw:verifier v0.1.0` on boot at `145.132.96.212:8090`). Akash deployment updated to pull components from Azure registry. Governance proposal finalized — 8 parts including Junoswap v2 code deep-dive and Genesis → 13 Buds architecture. Everything shipped in 5 days.

### What's next

- **Juno governance proposal** — Signaling proposal going live on juno-1 mainnet. Sent to Jake Hartnell for review. No code execution, no community pool funds — recognition of JunoClaw as Juno ecosystem infrastructure.
- **Genesis buds into 13** — WeightChange proposal distributes governance to 13 DAO members. Genesis loses voting power. DAO self-governs.
- **Validator sidecar proposal** — DAO asks validators to run WAVS operator sidecars for TEE-grade distributed attestation.
- **$JClaw token** — Soulbound trust-tree credentials. Genesis airdrop to initial Juno devs, top validators, and contributors. Each holder can "bud" once, passing the credential forward.

### Open source

Everything is public:
- **GitHub**: [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)
- **Contract**: `juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6`
- **TEE TX**: `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22`
- **Akash Operator (LIVE)**: http://provider.akash-palmito.org:31812
- **WAVS Component Registry**: http://145.132.96.212:8090
- **Junoswap Factory**: `juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh`
- **Junoswap JUNOX/USDC**: `juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98`

---

*JunoClaw is an agentic DAO on Juno. Proposals pass, WAVS verifies, the chain remembers.*
