# WAVS TEE Integration — Research Findings

## 1. Does WAVS Infrastructure Exist on Juno Testnet (uni-7)?

**Short answer: WAVS doesn't need to be "deployed" on a chain. It runs alongside it.**

WAVS is an **external operator node** that connects to chains via RPC/gRPC. The WAVS node:
- Monitors chain events (triggers)
- Runs off-chain WASI components
- Submits results back on-chain

The `wavs.toml` config supports arbitrary Cosmos chains:

```toml
[default.chains.cosmos.juno_testnet]
chain_id = "uni-7"
bech32_prefix = "juno"
rpc_endpoint = "https://juno-testnet-rpc.polkachu.com"
grpc_endpoint = "https://juno-testnet-grpc.polkachu.com:443"
gas_price = 0.075
gas_denom = "ujunox"
```

WAVS already supports **Cosmos event triggers** natively — the docs show Neutron as an example, but any CosmWasm chain works. We configure WAVS to point at uni-7 and it works.

**What we DO need to deploy on uni-7:**
- Service manager contract (CosmWasm, from avs-toolkit) — defines operator set
- Submission handler contract — receives verified results (or we adapt our existing contracts)
- Our existing agent-company/task-ledger/escrow contracts already emit the events WAVS would trigger on

**Conclusion: We are the builders. We deploy the avs-toolkit CosmWasm contracts on uni-7 ourselves. No waiting.**

---

## 2. TEE Hardware — Detailed Options

### What TEE Means in the WAVS Context

TEE = Trusted Execution Environment. The WAVS node runs inside a hardware-isolated enclave.
The enclave produces a **cryptographic attestation** proving:
- The exact code that ran (hash of WASI component)
- The data it processed
- That no one (not even the machine owner) could have tampered with execution

Jake's statement "WAVS TEEs already work — just run WAVS inside a TEE" means:
the WAVS Docker runtime can run inside a TEE enclave today.

### Hardware Options

#### A. Intel SGX (Software Guard Extensions)
- **What**: Application-level enclaves on Intel CPUs
- **Available on**: Intel Xeon E3 v6+, Xeon Scalable (Ice Lake, Sapphire Rapids)
- **NOT on**: Consumer i5/i7/i9 (SGX removed since 11th gen)
- **Memory limit**: 128MB–512MB EPC (Enclave Page Cache) depending on CPU
- **Software**: Gramine, Occlum, or Intel SGX SDK
- **Pros**: Most mature TEE. Well-documented. Battle-tested.
- **Cons**: Limited enclave memory. Requires SGX-enabled BIOS. Side-channel attacks documented (but mitigated in newer silicon).
- **Cost**: Dedicated servers with SGX: ~$50–150/month (OVH, Hetzner)

#### B. Intel TDX (Trust Domain Extensions)
- **What**: VM-level isolation (full VM runs in a trust domain)
- **Available on**: 4th/5th Gen Intel Xeon Scalable (Sapphire Rapids, Emerald Rapids)
- **Pros**: Full VM isolation (not just app-level). Larger memory. No EPC limits.
- **Cons**: Newer, less ecosystem support. Requires TDX-enabled hypervisor.
- **Cost**: Cloud only for now (Azure, GCP)

#### C. AMD SEV-SNP (Secure Encrypted Virtualization — Secure Nested Paging)
- **What**: VM-level memory encryption + attestation on AMD EPYC
- **Available on**: AMD EPYC 7003 (Milan), 9004 (Genoa)
- **Pros**: No enclave size limits. Full VM encryption. Good performance.
- **Cons**: Different attestation model than SGX. Less AVS ecosystem adoption.
- **Cost**: Dedicated EPYC servers: ~$80–200/month

#### D. AWS Nitro Enclaves
- **What**: Isolated VM partitions on AWS EC2 instances
- **Available on**: Most EC2 instance types (c5, m5, r5, c6i, etc.)
- **Pros**: Easiest to set up. No special hardware needed. AWS handles attestation infra. Nitro Attestation Document (signed by AWS).
- **Cons**: AWS-specific. Not truly decentralized (trust AWS). Monthly cost.
- **Cost**: ~$30–100/month (t3.medium to c5.xlarge)
- **Best for**: Testnet and early production

#### E. Azure Confidential Computing
- **What**: DCsv2/DCsv3 VMs (Intel SGX) or DCasv5/ECasv5 (AMD SEV-SNP)
- **Pros**: Production-grade. SGX enclaves with large EPC. Azure Attestation service built in.
- **Cons**: Azure-specific pricing.
- **Cost**: ~$50–200/month

#### F. Google Cloud Confidential Computing
- **What**: Confidential VMs with AMD SEV or Intel TDX
- **Pros**: Easy provisioning. Standard VM experience.
- **Cons**: Attestation less mature than AWS/Azure.
- **Cost**: ~10% premium over standard VMs

### Recommendation for JunoClaw

| Phase | Hardware | Why |
|-------|----------|-----|
| **Development** | Local (no TEE) | WAVS supports non-TEE mode for dev. Attestation simulated. |
| **Testnet** | AWS Nitro Enclave OR local SGX | Cheapest real TEE. ~$30/month. |
| **Mainnet** | Dedicated SGX/TDX server + cloud backup | Decentralized. Own hardware = no cloud trust. |

---

## 3. Path to Running Own Hardware

### Option A: Buy a Server (Best for Mainnet)

**Recommended spec for a WAVS TEE operator node:**
- CPU: Intel Xeon E-2388G or Xeon w3-2423 (SGX capable)
- RAM: 32GB+ ECC
- Storage: 500GB NVMe SSD
- Network: 1Gbps
- **Estimated cost**: $800–1500 (refurbished server or mini-server)

**Specific products:**
- Intel NUC Pro (Xeon, SGX-capable): ~$600–800
- Supermicro SYS-510T-ML (Xeon E-2300 series): ~$1200
- Dell PowerEdge R350 (Xeon E-2300): ~$1500 new, ~$800 refurb

**Setup steps:**
1. Enable SGX in BIOS (usually under Security → SGX settings)
2. Set SGX memory to maximum (typically 64MB or 128MB)
3. Install Ubuntu 22.04 LTS (best SGX support)
4. Install Intel SGX driver + PSW (Platform Software)
5. Install Gramine (LibOS for running Docker in SGX)
6. Run WAVS Docker image inside Gramine-SGX
7. Verify attestation with `gramine-sgx-get-token`

### Option B: Rent a Dedicated Server

**OVH:**
- Advance-1 (Intel Xeon E-2386G, SGX capable): ~€70/month
- Rise-1 (Intel Xeon E-2388G): ~€50/month
- **Check**: Not all OVH servers have SGX. Must confirm before ordering.

**Hetzner:**
- AX41-NVMe (AMD Ryzen, NO SGX): ❌
- AX102 (Intel Xeon, check SGX): ~€130/month
- Hetzner's Intel dedicated servers are hit-or-miss for SGX.

**Equinix Metal:**
- Bare metal with SGX: c3.small.x86 (~$0.50/hr)
- Best for production. True bare metal. SGX guaranteed.

### Option C: Hybrid (Recommended)

**Testnet now:** Run WAVS locally without TEE (development mode).
**Testnet with TEE:** AWS Nitro Enclave (~$30/month) or Azure DCsv2 (~$50/month).
**Mainnet:** Own dedicated SGX server (OVH ~€70/month or buy hardware ~$1000 one-time).

### Path Timeline

```
Week 1-2: Dev mode (local, no TEE, attestation simulated)
   └── Build WASI component + deploy contracts
Week 3:   AWS Nitro Enclave for real TEE attestation on testnet
   └── Validate attestation flow end-to-end
Week 4+:  Evaluate own hardware for mainnet
   └── Order SGX server or rent dedicated
```

---

## 4. Key Technical Insight from Jake

"WAVS TEEs already work. You just run WAVS inside a TEE."

This means:
1. The WAVS runtime (Docker) runs inside an SGX enclave (via Gramine) or Nitro Enclave
2. The WASI component runs inside the WAVS runtime → double isolation
3. Attestation is produced by the TEE hardware and included in the operator's signed result
4. The on-chain verifier checks: (a) operator signature valid, (b) attestation valid, (c) result matches quorum
5. **No custom TEE integration code needed** — WAVS handles it

For development and testnet, we run WAVS in non-TEE mode first, then flip `use_tee = true` when we deploy to TEE hardware. The WASI component code is identical either way.
