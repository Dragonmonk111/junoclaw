# Akash Integration Plan
## JunoClaw WAVS Operator — Persistent Decentralised Deployment

**Status**: ✅ READY TO DEPLOY — Azure TEE milestone complete (proposal 4, TX `6EA1AE79...D26B22`)

**Proven tooling from Azure session** (March 17, 2026):
- WAVS image: `ghcr.io/lay3rlabs/wavs:1.5.1`
- Component: `junoclaw_wavs_component.wasm` (355KB, built + tested in SGX enclave)
- WAVS CLI: `wavs-cli exec` confirmed working with `--device /dev/sgx_enclave`
- Attestation hash produced + submitted to Juno uni-7 — verifiable on-chain
- Full WAVS operator stack deployed: operator, aggregator, IPFS, warg registry, Anvil

**What this achieves**:
- Permanent 24/7 WAVS operator — no Azure VM to manage or pay for hourly
- Decentralised infrastructure — paid in AKT, hosted on community hardware
- Cosmos-native compute for a Cosmos-native DAO
- Platform for expanded scope: Junoswap revival + Neutron DeFi forks (per Jake Hartnell)

---

## Expanded Scope (per Jake Hartnell, March 17)

Jake Hartnell (Juno co-founder, WAVS architect) directed:

> "We should point the agent at projects like reviving Junoswap, etc."
> "Neutron is dead now so the agent could probably fork some of those projects as well."

The Akash operator is no longer just an attestation endpoint — it becomes the **persistent compute layer** for:
1. **JunoClaw DAO verification** — current (outcome markets, data feeds, randomness)
2. **Junoswap revival** — agent monitors + verifies DEX state, pool health, price feeds
3. **Neutron DeFi forks** — agent executes verification for forked lending/DEX protocols

This means the operator needs to be always-on, not just a demo. Akash is the right home.

---

## Architecture on Akash

```
Akash Provider (AMD EPYC, ideally SEV-capable)
┌──────────────────────────────────────────────────────┐
│  Container: wavs-operator                            │
│    - watches Juno uni-7 for trigger events           │
│    - runs junoclaw:verifier WASI component           │
│    - signs results (hardware if SEV active)          │
│    - future: Junoswap pool verification              │
│    - future: Neutron fork protocol verification      │
│                                              │
│  Container: wavs-aggregator                          │
│    - HTTP :8080 exposed globally                     │
│    - bridge daemon polls this endpoint               │
│                                                      │
│  Container: ipfs-node                                │
│    - local IPFS for service manifest + component     │
│    - :5001 API, :8081 gateway (internal only)        │
└──────────────────────────────────────────────────────┘
         │
         │ HTTP (Akash random hostname)
         ▼
bridge.ts (runs on any always-on machine)
         │
         ▼
agent-company contract on Juno uni-7
         │
         ├── outcome markets
         ├── Junoswap pool state
         └── forked Neutron protocol state
```

---

## What We Need Before Starting

1. **AKT tokens** — check your Keplr wallet
   - Minimum to deploy: ~5–10 AKT (~$5–10)
   - Recommended for 30-day persistent operator: ~50 AKT (~$50)
   - Check balance: open Keplr → Akash

2. **Keplr wallet connected to Akash** — you likely already have this

3. **The Akash Console** — [console.akash.network](https://console.akash.network) (no CLI needed)

---

## The SDL File (Production-Ready)

Updated with exact WAVS image tag from Azure deployment.

Save as: `junoclaw/wavs/akash.sdl.yml`

```yaml
---
version: "2.0"

services:
  wavs-operator:
    image: ghcr.io/lay3rlabs/wavs:1.5.1
    command: ["wavs-cli", "exec", "--component", "/data/junoclaw_wavs_component.wasm", "--home", "/data"]
    env:
      - WAVS_CLI_COSMOS_MNEMONIC=REPLACE_WITH_MNEMONIC
      - RUST_LOG=info
    expose:
      - port: 8041
        as: 8041
        to:
          - service: wavs-aggregator
        proto: tcp

  wavs-aggregator:
    image: ghcr.io/lay3rlabs/wavs:1.5.1
    env:
      - AGGREGATOR_MODE=true
    expose:
      - port: 8040
        as: 8080
        to:
          - global: true
        proto: tcp

  ipfs:
    image: ipfs/kubo:v0.37.0
    expose:
      - port: 5001
        as: 5001
        to:
          - service: wavs-operator
          - service: wavs-aggregator
        proto: tcp

profiles:
  compute:
    wavs-operator:
      resources:
        cpu:
          units: 2
        memory:
          size: 4Gi
        storage:
          - size: 10Gi
    wavs-aggregator:
      resources:
        cpu:
          units: 1
        memory:
          size: 1Gi
        storage:
          - size: 2Gi
    ipfs:
      resources:
        cpu:
          units: 0.5
        memory:
          size: 512Mi
        storage:
          - size: 5Gi
  placement:
    akash:
      attributes:
        host: amd
      signedBy:
        anyOf:
          - akash1365ez...  # Known TEE-capable providers
      pricing:
        wavs-operator:
          denom: uakt
          amount: 100
        wavs-aggregator:
          denom: uakt
          amount: 50
        ipfs:
          denom: uakt
          amount: 25

deployment:
  wavs-operator:
    akash:
      profile: wavs-operator
      count: 1
  wavs-aggregator:
    akash:
      profile: wavs-aggregator
      count: 1
  ipfs:
    akash:
      profile: ipfs
      count: 1
```

**Notes**:
- Image tag `1.5.1` confirmed working from Azure deployment
- Multi-container: operator + aggregator + IPFS (matches Azure stack)
- Aggregator exposed globally on :8080 — bridge daemon connects here
- IPFS internal only — stores service manifest + component
- `host: amd` attribute prefers AMD EPYC providers for SEV capability

---

## Step-by-Step Akash Deployment

### Step 1 — Prepare Component + Service Manifest

Already done. From Azure session we have:
- `junoclaw_wavs_component.wasm` — 355KB, builds with `cargo component build --release`
- Component digest: `59fcdc322ac1a3c3f73a28752b716ff2dd74a7c4024321321e5956ccfdb189d3`
- Service manifest: `service.json` in `junoclaw/wavs/`

### Step 2 — Upload Component to IPFS (persistent)

```bash
# Upload to public IPFS pinning service (web3.storage or Pinata)
curl -X POST https://api.web3.storage/upload \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/wasm" \
  --data-binary @compiled/junoclaw_wavs_component.wasm
# Returns CID — use this in service.json
```

### Step 3 — Deploy via Akash Console

1. Go to [console.akash.network](https://console.akash.network)
2. Connect Keplr wallet
3. Click **"Deploy"** → **"Upload SDL"** → paste SDL content
4. Click **"Create Deployment"**
5. Select provider with **AMD EPYC** in specs (prefer Frankfurt/EU for Hetzner hardware)
6. Accept bid → deployment starts (~2 min)

### Step 4 — Verify TEE on Provider

```bash
# In Akash Console → your deployment → Shell tab
ls /dev/sev*     # AMD SEV present = hardware TEE
cat /proc/cpuinfo | grep "model name"  # Should show AMD EPYC
```

### Step 5 — Get Akash Hostname + Update Bridge

```bash
# From Akash Console → Leases tab → copy hostname
# e.g. abc123.provider.akash.network:8080

# Update bridge daemon config:
echo "WAVS_AGGREGATOR_URL=http://abc123.provider.akash.network:8080" >> junoclaw/wavs/bridge/.env
```

### Step 6 — Verify End-to-End

```bash
npx tsx src/test-proposal.ts       # Create proposal 5
npx tsx src/query-attestation.ts 5 # Verify attestation from Akash operator
```

---

## TEE Tiers on Akash

| Outcome | What it means | Valid for JunoClaw? |
|---------|--------------|---------------------|
| `/dev/sev` present | AMD SEV active — hardware TEE | **Yes — full hardware attestation** |
| AMD EPYC, no `/dev/sev` | SEV capable but not enabled by provider | Software attestation (still works) |
| Intel Xeon | No TEE | Software attestation (still works) |

Even without hardware TEE, the Akash deployment proves:
- Decentralised hosting ✅
- Persistent 24/7 operation ✅
- Cosmos-native infrastructure ✅

---

## Cost Estimate

| Duration | AKT needed | USD estimate |
|----------|-----------|--------------|
| 1 week test | ~3 AKT | ~$3 |
| 1 month persistent | ~12 AKT | ~$12 |
| 1 year | ~144 AKT | ~$144 |

Multi-container (operator + aggregator + IPFS) costs ~1.5x single container.

---

## The Milestone Narrative

Once WAVS operator is running on Akash, three stories converge:

1. **Azure** → Proved hardware TEE works (proposal 4, Intel SGX)
2. **Akash** → Decentralised hosting, always-on (proposal 5+, AMD SEV target)
3. **Validators** → Distributed operator set (proposal by Jake/community)

> "JunoClaw's verification layer runs on decentralised infrastructure — no servers, no cloud bills, paid in AKT. The operator lives on Akash. The proofs land on Juno. The bridge connects them. Every layer is open, every layer is replaceable. And the scope is expanding: Junoswap revival, Neutron DeFi forks — all verified by the same TEE compute layer."

---

## Checklist

- [x] WAVS image tag confirmed: `ghcr.io/lay3rlabs/wavs:1.5.1`
- [x] Component built + tested in SGX enclave
- [x] Attestation proven on-chain (proposal 4)
- [x] SDL updated with multi-container architecture + env vars + clear docs
- [x] Acquire AKT tokens — **63.77 AKT** in `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`
- [x] Deep integration doc: legal, social, dev community, Greg Osuri strategy (AKASH_DEEP_INTEGRATION.md)
- [x] Akash Console deployment guide (AKASH_DEPLOY_GUIDE.md)
- [x] Outreach templates for Discord, Forum, Twitter (AKASH_OUTREACH.md)
- [x] Crystal-clear walkthrough: AKASH_WALKTHROUGH.md (what we hire, run, connect)
- [x] agent-company v3 migrated (code_id=63, CodeUpgrade proposals, supermajority 67%)
- [x] Junoswap factory wired into agent-company via CodeUpgrade proposal 5
- [x] Jake Hartnell endorsement: "good chance for Juno to rise again"
- [x] Deploy via Akash Console — LIVE at `http://provider.akash-palmito.org:31812` (March 17, 2026)
- [x] Verify all 3 services running (operator + aggregator + IPFS) — all healthy, chain connected
- [x] Bridge daemon connected to Akash aggregator endpoint — config.ts updated
- [ ] End-to-end test (create proposal → Akash operator detects → submits attestation)
- [ ] Tweet deployment milestone — tag @gregosuri @akashnet_ @Jake_Hartnell
- [ ] Apply for Akash Accelerate grant (after live proof)

**NOTE**: Akash is NOT SGX/TEE. It's regular decentralized compute.
TEE was proven on Azure (one-time). Akash = permanent hosting.
See AKASH_WALKTHROUGH.md for the full explanation.
