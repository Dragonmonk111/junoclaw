# Plan: TEE Production Deployment for the Sealed Signer

> A033 authorized the sealed signer as DAO signing infrastructure. M2.1 proved cross-platform determinism. The invoke API prototype passed E2E on uni-7. The remaining question is: **where does the TEE run, who pays for it, and how do we get there?** This document explores every option, ranks them by cost/ease/practicality, and proposes a DAO-funded path forward.

---

## 1. What We Need

A single TEE-capable compute instance that runs the WAVS Docker container with:

- **WAVS aggregator/runtime** — listens for on-chain events, dispatches to components
- **Sealed signer WASM component** — `junoclaw_sealed_signer.wasm` (signs Cosmos txs)
- **Invoke server** — `invoke-server.ts` HTTP endpoint for off-chain signing requests
- **Bridge** — TypeScript bridge that polls chain and broadcasts transactions

**Resource requirements (minimal):**
- CPU: 1-2 vCPU (wasmtime signing is sub-second, low frequency)
- RAM: 512 MB - 1 GB (WAVS Docker stack + wasmtime)
- Disk: 10 GB (Docker images, WAVS data, sealed blob)
- Network: public IP with open port for invoke API + RPC to Juno

**Security requirements:**
- TEE hardware: AMD SEV-SNP, Intel TDX, or Intel SGX
- Key generated inside TEE, never leaves
- Sealed blob persisted (AES-256-GCM)
- Attestation proof available for DAO verification

**One instance is sufficient** for both Moultbook and Junoclaw. The sealed signer is a single Cosmos address serving both applications. Multiple instances would only be for HA redundancy, not capacity.

---

## 2. All Options Explored

### Option A: Akash Network (Cheapest, Requires Provider Enablement)

**Status**: The confidential compute software stack is built and merged (AEP-83, provider PR #396, console PR #3365). Tenants request `params.tee: cpu` in their SDL. Providers with `tee/type: cpu` attribute bid. Workloads run in Kata Containers micro-VMs on SEV-SNP or TDX hardware.

**Problem**: Zero providers currently advertise `tee/type` attributes. No mainnet provider has installed Kata Containers + enabled TEE in BIOS.

**How to unblock**:

1. **Incentivize an existing Akash provider** to enable TEE on their hardware. Many providers already run AMD EPYC or Intel Xeon Scalable processors that support SEV-SNP or TDX in BIOS. They need to:
   - Enable SEV-SNP or TDX in BIOS
   - Install Kata Containers runtime + register as Kubernetes RuntimeClass
   - Set `tee/type: cpu` attribute via provider inventory service
   - Restart their provider service

2. **Target providers with AMD EPYC hardware** (SEV-SNP is easier to enable than TDX — no special kernel needed on recent Ubuntu):
   - Overclock (Akash core team) — runs AMD EPYC in Equinix datacenters, already audited
   - Any community provider with AMD EPYC Milan/Genoa

3. **Bounty approach**: DAO offers a 3-month deployment commitment at a fixed monthly rate to the first provider that enables TEE. This guarantees revenue for the provider while they invest in setup.

**Estimated cost**: $15-30/month (based on comparable Akash small instance pricing + 10-19% TEE premium)

**Pros**:
- Cheapest option by 4-8x
- Paid in AKT/crypto (no fiat banking needed)
- Decentralized — aligns with DAO ethos
- No KYC or account requirements
- Already integrated with our WAVS operator stack

**Cons**:
- No provider currently offers TEE
- Requires outreach + incentive to unblock
- Kata Containers adds ~5-10% overhead
- Attestation verification workflow is new

**Effort to unblock**: Medium. Requires Discord outreach to Akash providers, a bounty/incentive offer, and 1-2 weeks for a motivated provider to enable TEE.

---

### Option B: AWS EC2 with Nitro Enclaves (Easiest, Available Now)

**Status**: Generally available today. No additional charge for Nitro Enclaves — you pay only for the EC2 instance.

**How it works**:
- Launch a Nitro-enabled EC2 instance (e.g., `m7a.large` — AMD EPYC, supports SEV-SNP via Nitro)
- Create a Nitro Enclave from the WAVS Docker image
- The enclave runs in an isolated VM with encrypted memory
- No operator (not even AWS root) can access enclave memory
- ACM (AWS Certificate Manager) provides free TLS certificates for the enclave

**Instance sizing**:
- `m7a.large` (2 vCPU, 8 GB) — sufficient, ~$120/month on-demand
- `m7a.medium` (1 vCPU, 4 GB) — may work, ~$60/month on-demand (if Nitro Enclaves supports it — needs verification)
- Spot pricing could reduce cost by 60-90% (~$12-50/month) but spot instances can be terminated

**Estimated cost**: $60-120/month on-demand, ~$12-50/month spot

**Pros**:
- Available right now, no waiting
- No additional cost for enclave functionality
- Well-documented, mature attestation SDK
- Free TLS certificates via ACM
- Can pay with credit card (fiat)

**Cons**:
- Most expensive option (vs Akash)
- Requires AWS account (KYC, credit card)
- Centralized — AWS can terminate the account
- Nitro Enclaves have some limitations (no interactive shells, file system isolation)
- Fiat payment doesn't align with DAO treasury model

**Effort**: Low. Can be running in 1 day.

---

### Option C: Azure Confidential VM (Available Now, Mid-Price)

**Status**: Generally available. DCasv5-series (AMD SEV-SNP) and DCesv6-series (Intel TDX).

**How it works**:
- Launch a DCasv5 VM (AMD EPYC 7763v with SEV-SNP)
- The entire VM memory is encrypted by the CPU's secure processor
- Keys are generated inside the AMD Secure Processor, never exposed
- Azure engineers cannot access VM memory
- OS disk can be pre-encrypted before provisioning

**Instance sizing**:
- `Standard_DC2as_v5` (2 vCPU, 8 GB) — smallest SEV-SNP CVM, ~$165/month
- `Standard_DC4as_v5` (4 vCPU, 16 GB) — ~$330/month (overkill)

**Estimated cost**: ~$165/month

**Pros**:
- Available right now
- Full VM encryption (not just enclave) — entire OS is protected
- SEV-SNP hardware attestation built in
- Well-documented
- We already have Azure experience from the March 2026 SGX test

**Cons**:
- More expensive than AWS
- Requires Azure account (KYC, credit card)
- Centralized
- Fiat payment
- No spot pricing for confidential VMs

**Effort**: Low. Can be running in 1 day.

---

### Option D: Google Cloud Confidential VM (Available Now, Mid-Price)

**Status**: Generally available. N2D series (AMD SEV-SNP) and C3 series (Intel TDX).

**Instance sizing**:
- `n2d-standard-2` + confidential (2 vCPU, 8 GB) — ~$125/month
- Spot pricing available — potentially ~$12-25/month

**Estimated cost**: ~$125/month on-demand, ~$12-25/month spot

**Pros**:
- Available right now
- Cheapest of the three major clouds for SEV-SNP
- Spot pricing available (biggest discount)
- Good documentation
- GCP Attestation Agent built in

**Cons**:
- Requires GCP account (KYC, credit card)
- Centralized
- Fiat payment
- Spot instances can be preempted

**Effort**: Low. Can be running in 1 day.

---

### Option E: Bare Metal Rental (Latitude.sh or Equinix Metal)

**Status**: Available. We already proved this works — the M2.1 cross-platform determinism test ran on Latitude.sh London (AMD EPYC 7443P).

**How it works**:
- Rent a bare metal server with AMD EPYC (SEV-SNP) or Intel Xeon (TDX)
- Full root access, enable SEV-SNP in BIOS
- Run the WAVS Docker container directly
- No hypervisor overhead, no cloud provider can see memory

**Instance sizing**:
- Latitude.sh `s3-large-x86` (AMD EPYC, 32 GB) — ~$0.43/hour (~$310/month if running 24/7)
- But we only need it running 24/7 for production. Can use smaller configs.
- Equinix Metal `c3.small.x86` — similar specs, ~$0.50/hour

**Estimated cost**: ~$200-310/month (bare metal is more expensive than cloud VMs but offers full hardware control)

**Pros**:
- We already proved it works (M2.1 determinism test)
- Full root access, no virtualization layer
- Can enable SEV-SNP directly in BIOS
- No cloud provider can access memory (bare metal)
- Can pay with crypto (Latitude.sh accepts BTC)

**Cons**:
- Most expensive option for 24/7 operation
- Requires manual setup (BIOS, OS, Docker)
- No auto-scaling or managed services
- Hardware failures require manual intervention
- Overkill resources for what we need

**Effort**: Medium. We know how to do it, but 24/7 bare metal is expensive for a lightweight signing service.

---

### Option F: Self-Hosted / Community-Donated Hardware

**Status**: Possible if a community member has compatible hardware.

**How it works**:
- A DAO member or community contributor donates server time on their TEE-capable hardware
- They run the WAVS Docker container with SEV-SNP/TDX enabled
- The DAO governs access via the sealed blob (the contributor never has the key)

**Requirements**:
- AMD EPYC (Milan or newer) or Intel Xeon Scalable (Sapphire Rapids or newer) with TDX
- Ubuntu 24.04+ (good SEV-SNP/TDX kernel support)
- Stable internet connection
- Docker

**Estimated cost**: $0 (donated)

**Pros**:
- Free
- Full hardware control
- No KYC or account requirements
- Community alignment

**Cons**:
- Depends on a single community member's reliability
- No SLA or uptime guarantee
- Hardware could fail or go offline
- Trust in the operator (though TEE protects the key regardless)
- Not scalable or professional

**Effort**: Low if a contributor steps up. High if we need to find one.

---

### Option G: Hybrid — Start on Cloud, Migrate to Akash

**Status**: Recommended path.

**Phase 1 (Week 1)**: Deploy on the cheapest available cloud TEE (GCP spot or AWS) to get production signing live immediately. The plaintext mnemonic is retired now.

**Phase 2 (Weeks 2-4)**: Outreach to Akash providers via Discord. Offer a 3-month deployment bounty. Once a provider enables TEE, migrate the WAVS container to Akash.

**Phase 3 (Ongoing)**: Run on Akash at $15-30/month. The cloud instance becomes cold standby.

---

## 3. Ranking by Criteria

| Criteria | Cheapest | Easiest | Most Practical | Most Decentralized |
|---|---|---|---|---|
| **1st** | Akash ($15-30/mo) | AWS Nitro (1 day) | Hybrid (G now → Akash) | Akash |
| **2nd** | GCP Spot ($12-25/mo) | GCP (1 day) | AWS Nitro (reliable) | Self-hosted |
| **3rd** | AWS ($60-120/mo) | Azure (1 day) | GCP (cheap + spot) | Bare metal (Latitude.sh) |
| **4th** | Azure ($165/mo) | Bare Metal (we know how) | Azure (familiar SGX) | AWS/GCP/Azure (all centralized) |
| **5th** | Bare Metal ($200-310/mo) | Self-hosted (if available) | Self-hosted (if reliable) | — |
| **6th** | Self-hosted ($0) | Akash (needs outreach) | — | — |

---

## 4. Recommended Path: The Hybrid Plan

### Phase 1: Immediate Deployment (Week 1)

**Target**: GCP `n2d-standard-2` with Confidential VM (AMD SEV-SNP) — **SPOT PRICING**

**Why GCP Spot**:
- Spot pricing: ~$15-25/month (91% off on-demand of ~$125/month)
- DAO treasury is ~$200 USD equivalent in JUNO — spot pricing gives 2-3 months of runway
- On-demand would burn 62% of treasury in one month — spot is the only sustainable option within budget
- The sealed signer is stateless — preemption causes a restart, not data loss. The sealed blob persists on disk.
- SEV-SNP is mature and well-documented

**Steps**:
1. Create a GCP project (or use an existing one)
2. Launch `n2d-standard-2` confidential VM with **spot pricing** in a region with SEV-SNP availability
3. Attach a persistent disk for the sealed blob (survives preemption)
4. Install Docker + docker-compose
5. Deploy the WAVS Docker stack (aggregator + sealed signer + invoke server + bridge)
6. Generate the sealed key inside the TEE (first run)
7. Verify attestation report
8. Point moultbook.js at the invoke server endpoint
9. Retire the plaintext mnemonic

**Cost**: ~$15-25/month from DAO treasury (~$50-75 for 2-3 months)

### Phase 2: Akash Provider Outreach (Weeks 2-4)

**Target**: Get at least one Akash provider to enable TEE

**Outreach plan**:
1. Post in Akash Discord (#providers channel) explaining the use case and offering a 3-month deployment commitment
2. DM Overclock (Akash core team) — they run AMD EPYC in Equinix and are the most likely to enable TEE quickly
3. Post on Akash forum with a "TEE Provider Bounty" — first provider to advertise `tee/type: cpu` gets a 3-month lease at above-market rate
4. Offer to help with Kata Containers setup (we have the docs from AEP-83)

**Bounty terms** (proposed):
- 3-month deployment commitment at $25/month (above typical Akash small instance pricing)
- Total commitment: $75 for 3 months
- Provider must: enable SEV-SNP or TDX, install Kata Containers, advertise `tee/type: cpu` attribute
- Paid in AKT
- Outreach is free: Jake's Juno AI agent posts the bounty across Discord, Akash forum, and social channels at zero cost to the DAO treasury

### Phase 3: Migration to Akash (Once TEE Provider Available)

**Steps**:
1. Write the Akash SDL with `params.tee: cpu`
2. Deploy the WAVS Docker stack on Akash
3. Transfer the sealed blob (the key is portable — same passphrase, same sealed blob works on any TEE)
4. Verify attestation via the Akash attestation sidecar API
5. Update moultbook.js endpoint to the Akash lease URL
6. Shut down the GCP instance (or keep as cold standby)

**Cost**: ~$15-30/month on Akash (vs ~$125/month on GCP)

---

## 5. DAO Proposal: A034 — Fund TEE Instance for Sealed Signer Production

### Proposal Summary

A033 authorized the sealed signer as DAO signing infrastructure. This proposal funds the compute infrastructure to run it — within the actual DAO treasury balance of ~$200 USD equivalent in JUNO.

**What A034 does**:
1. Authorizes DAO treasury spend of the full current balance (~$200 USD equivalent in JUNO) for TEE infrastructure.
2. Directs builders to deploy on GCP Confidential VM using **spot pricing** (~$15-25/month) — 2-3 months of runway within budget.
3. Authorizes a $75 AKT bounty for the first Akash provider to enable TEE (`tee/type: cpu` attribute).
4. Directs Jake's Juno AI agent to post the bounty across Discord, Akash forum, and social channels at zero cost.
5. Directs builders to deploy within 1 week of passage and migrate to Akash once a TEE provider is available.
6. Requires monthly Moultbook reports on TEE uptime, signing volume, and costs.

**What A034 does NOT do**:
- Does not change the sealed signer code (already built and tested)
- Does not change the agent-company contract (already extended)
- Does not mandate a specific cloud provider (builder discretion within budget)
- Does not authorize key rotation (future proposal if needed)
- Does not fund beyond 2-3 months (A035 will address continued funding)

**Budget breakdown**:
- GCP Confidential VM spot (2-3 months): ~$50-75
- Akash TEE provider bounty: $75
- Buffer (DNS, monitoring, migration): ~$50-75
- **Total: ~$200 USD equivalent in JUNO (full treasury)**

**Voting**:
- YES = fund TEE infrastructure and provider bounty
- NO = keep the sealed signer on testnet only / defer production deployment
- ABSTAIN = defer to builders

---

## 6. Akash Provider Bounty — Draft Discord Message

```
🚀 Akash TEE Provider Bounty — $75 AKT for 3 months of guaranteed revenue

We need a confidential compute (TEE) provider on Akash mainnet. The software stack is ready (AEP-83, Kata Containers, attestation sidecar — all merged). What's missing is a provider with the hardware attribute enabled.

What we need:
- AMD SEV-SNP or Intel TDX enabled in BIOS
- Kata Containers runtime installed and registered as Kubernetes RuntimeClass
- tee/type: cpu attribute advertised via provider inventory service

What you get:
- 3-month deployment commitment at $25/month (paid in AKT)
- Total: $75 AKT for ~2 hours of setup work
- Be the FIRST Akash provider to offer confidential compute

Our use case: a Cosmos DAO signing service (WAVS sealed signer) that needs TEE to protect a signing key. Lightweight workload — 1-2 vCPU, 512 MB RAM, minimal traffic.

DM me if interested. Full setup docs: https://akash.network/roadmap/aep-83/
```

---

## 7. Key Insight: The Sealed Blob Is Portable

A critical architectural point: **the sealed key blob is portable across TEE instances**. The sealed blob is encrypted with a passphrase (environment variable). As long as the same passphrase is used, the same sealed blob can be loaded on any TEE — GCP, AWS, Azure, Akash, or bare metal.

This means:
- We can generate the key on GCP, then migrate to Akash without rotating the key
- The DAO address stays the same across migrations
- No on-chain changes needed when switching infrastructure
- The passphrase is the only secret that must be protected during migration

This portability is what makes the hybrid plan viable. We don't need to commit to a single provider forever.

---

## 8. Security Considerations

- **Attestation verification**: Before trusting the TEE, the DAO should verify the attestation report. On GCP, this means checking the GCP Attestation Agent's report. On Akash, the attestation sidecar provides raw hardware-signed reports.
- **Passphrase custody**: The `WAVS_INVOKE_PASSPHRASE` environment variable must be set securely. On cloud, use the provider's secret manager. On Akash, use the SDL's env vars (encrypted in the deployment manifest).
- **Sealed blob backup**: The sealed blob should be backed up to a secure location (e.g., DAO multisig or encrypted storage). If the TEE instance is lost, the blob can be restored on a new instance with the same passphrase.
- **Network security**: The invoke server should only accept connections from known IPs (moultbook.js host) or require a bearer token (already implemented).
- **Monitoring**: The DAO should monitor the TEE instance for uptime, signing activity, and attestation freshness.

---

## 9. Timeline

| Week | Milestone |
|---|---|
| Week 0 | A034 proposed on-chain (~$200 JUNO, full treasury) |
| Week 1 (post-pass) | Deploy on GCP Confidential VM (spot), retire plaintext mnemonic. Jake's AI agent posts Akash bounty. |
| Week 2 | DM Overclock and AMD EPYC providers. Monitor bounty responses. |
| Week 3-4 | Provider enables TEE, or we continue on GCP spot |
| Week 5 (if Akash ready) | Migrate to Akash, verify attestation |
| Monthly | Moultbook reports on uptime, costs, signing volume |
| Month 3 | If still on GCP, propose A035 for continued funding or report Akash migration |

---

## 10. Decision Matrix

If the DAO wants **cheapest long-term**: Akash ($15-30/mo) — but needs provider outreach
If the DAO wants **fastest to production within budget**: GCP Spot ($15-25/mo) — available today, 2-3 months runway
If the DAO wants **most decentralized**: Akash or self-hosted — aligns with ethos
If the DAO wants **most reliable**: AWS Nitro ($120/mo) — mature, but exceeds monthly treasury
If the DAO wants **familiar**: Azure ($165/mo) — we've done SGX there before, but exceeds treasury

**Recommendation**: GCP Spot now + free Akash outreach via Jake's agent → migrate to Akash when provider enables TEE. Fits the $200 treasury perfectly.
