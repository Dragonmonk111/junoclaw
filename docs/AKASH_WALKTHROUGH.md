# Akash Walkthrough — What We Hire, What We Run, What Connects to What

> This is the single source of truth for how Akash fits into JunoClaw.

---

## The One-Sentence Answer

**We rent cheap cloud servers from strangers on the Akash marketplace to run a program that watches Juno and submits verification results back to our contract.**

That's it. Everything below is details.

---

## The Two Things People Confuse

### Thing 1: Azure (the lab — done, one-time)

On March 17, we rented a special Microsoft server (Azure DCsv3) with **Intel SGX hardware** — a chip that creates a sealed room inside the CPU. We ran our WASI component inside that sealed room and proved it works. That proof is Proposal 4 on uni-7.

**We are done with Azure.** It was a one-time experiment. Cost $0.19/hr. We proved TEE works. Delete the VM.

### Thing 2: Akash (the home — permanent)

Akash is where the WAVS operator **lives permanently**. It's a decentralized marketplace where:

- People with spare servers list them ("providers")
- People who need servers rent them ("deployers" — that's us)
- Payment is in AKT tokens
- No KYC, no credit card, no single company can shut us down

**Akash does NOT necessarily have SGX/TEE hardware.** Most Akash providers are regular servers. That's fine — the operator still works correctly, it just can't produce *hardware-signed* attestations. Think of it as: Azure proved the math is right. Akash runs the math 24/7.

When Akash providers start offering AMD SEV or Intel TDX hardware (happening slowly), we can upgrade to TEE-on-Akash. For now, Akash = reliable decentralized hosting.

---

## What We're Renting

Three containers, running 24/7, costing ~$15-25/month total:

```
┌─────────────────────────────────────────────────────────────┐
│  YOUR AKASH DEPLOYMENT (some provider's server)             │
│                                                             │
│  ┌─────────────────────┐     ┌───────────────────────┐     │
│  │  1. WAVS OPERATOR   │────▶│  2. WAVS AGGREGATOR   │     │
│  │  2 CPU, 4GB RAM     │     │  1 CPU, 1GB RAM       │     │
│  │                     │     │                       │     │
│  │  THE WORKER         │     │  THE COLLECTOR        │     │
│  │  - Watches Juno RPC │     │  - Gathers results    │     │
│  │  - Detects events   │     │  - Future: multi-     │     │
│  │  - Runs WASI verify │     │    operator consensus │     │
│  │  - Signs results    │     │  - Exposes health API │     │
│  └──────────┬──────────┘     └───────────────────────┘     │
│             │                                               │
│  ┌──────────┴──────────┐                                   │
│  │  3. IPFS NODE       │                                    │
│  │  0.5 CPU, 512MB RAM │                                    │
│  │                     │                                    │
│  │  THE STORAGE        │                                    │
│  │  - Holds the 355KB  │                                    │
│  │    WASI component   │                                    │
│  │  - Operator pulls   │                                    │
│  │    component from   │                                    │
│  │    here to execute  │                                    │
│  └─────────────────────┘                                    │
└─────────────────────────────────────────────────────────────┘
```

**Total resources**: 3.5 CPU cores, 5.5 GB RAM, 17 GB storage.
**Total cost**: ~$15-25/month in AKT (we have 63.77 AKT, enough for 2-4 months).

---

## What Connects to What

```
                    AKASH PROVIDER
                    ┌─────────────────┐
                    │  wavs-operator   │
                    │                  │
    READS ◀─────── │  Polls Juno RPC  │ ───────▶ WRITES
    (events)       │  every ~5 sec    │          (attestation TXs)
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
    ┌──────────────┐ ┌────────────┐ ┌──────────────┐
    │  Juno RPC    │ │  IPFS      │ │  Juno RPC    │
    │  (READ)      │ │  (local)   │ │  (WRITE)     │
    │              │ │            │ │              │
    │  Polls for:  │ │  Stores:   │ │  Submits:    │
    │  - wasm-wavs │ │  - WASI    │ │  - SubmitAt- │
    │    _push     │ │    component│ │    testation │
    │  - wasm-out- │ │    binary  │ │    TX to     │
    │    come_cre- │ │  (355KB)   │ │    agent-    │
    │    ate       │ │            │ │    company   │
    │  - wasm-sor- │ │            │ │              │
    │    tition_   │ │            │ │  Signs with: │
    │    request   │ │            │ │  Neo wallet  │
    └──────────────┘ └────────────┘ └──────────────┘

    Juno RPC endpoint:                Agent-company contract:
    rpc.uni.junonetwork.io:443        juno1k8dxll425...stj85k6
```

### The connections in plain English:

1. **Operator → Juno RPC** (outbound HTTP, read-only): "Hey Juno, any new proposals executed since I last checked?"
2. **Operator → IPFS** (internal): "Give me the WASI component binary so I can run verification"
3. **Operator → Juno RPC** (outbound HTTP, write): "Here's the attestation hash for proposal X, signed by the Neo wallet"
4. **Operator → Aggregator** (internal): "Here's my result" (for future multi-operator setups)

**Nothing connects inbound to Akash.** The operator only makes outbound connections. The aggregator exposes a health endpoint for monitoring, but nothing outside needs to call in.

---

## The Exact Steps (Your Action)

### Before You Start

You need:
- **Keplr wallet** with the Akash address: `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`
- **63.77 AKT** in that wallet (confirmed)
- **The SDL file**: `wavs/akash.sdl.yml` (in the repo)

### Step 1: Open Console

Go to **https://console.akash.network** → Click **Connect Wallet** → Select **Keplr** → Approve

You'll see your AKT balance in the top right.

### Step 2: Create Deployment

1. Click the **"Deploy"** button (big blue button)
2. Choose **"Build your template"**
3. Click the **"YAML"** tab at the top
4. Open `wavs/akash.sdl.yml` from the repo in a text editor
5. Copy the entire file and paste it into the YAML editor
6. Click **"Create Deployment"**

### Step 3: Fund Escrow

Akash takes a **5 AKT deposit** (refundable when you close the deployment). This is like a security deposit on an apartment.

1. Click **"Deposit"**
2. Approve the Keplr transaction (5 AKT)
3. Wait for confirmation (~6 seconds)

### Step 4: Pick a Provider

After the deposit, providers bid on your deployment. This takes 30-60 seconds.

You'll see a list like:

```
Provider A:  0.8 AKT/block  |  US-East  |  AMD EPYC  |  99.8% uptime
Provider B:  1.2 AKT/block  |  EU-West  |  Intel Xeon |  99.5% uptime
Provider C:  0.6 AKT/block  |  Asia     |  AMD EPYC  |  98.9% uptime
```

**Pick the cheapest AMD EPYC provider with >99% uptime.** AMD EPYC = future SEV upgrade path.

Click **"Accept Bid"** → Approve Keplr TX.

### Step 5: Wait for Spin-Up (1-3 minutes)

Console shows deployment status. All 3 services should go green:

- ✅ `wavs-operator` — Running
- ✅ `wavs-aggregator` — Running
- ✅ `ipfs` — Running

### Step 6: Get the Aggregator URL

Go to **Leases** tab → find `wavs-aggregator` → copy the **public endpoint URL**.

It looks like: `https://abc123.provider.akash.network:8080`

### Step 7: Verify It Works

In Console → **Logs** tab → select `wavs-operator`:

You should see:
```
[INFO] Connected to Juno RPC: rpc.uni.junonetwork.io:443
[INFO] Watching contract: juno1k8dxll425...stj85k6
[INFO] Chain ID: uni-7
[INFO] Operator ready, polling for events...
```

### Step 8: Test End-to-End

Create a new proposal on agent-company, vote it through, execute it. Watch the Akash operator logs — it should detect the event, run the WASI component, and submit an attestation TX back to Juno.

### Step 9: Update the Bridge Config

After Akash is running, update `wavs/bridge/.env`:

```
WAVS_AGGREGATOR_URL=https://abc123.provider.akash.network:8080
```

This tells the local bridge tools where the Akash operator is.

---

## What Happens After Deployment

| Event | What Happens | Your Action |
|-------|-------------|-------------|
| **New proposal executed on Juno** | Operator auto-detects, verifies, submits attestation | Nothing — it's automatic |
| **AKT running low** | Console shows warning | Top up escrow in Console |
| **Provider goes down** | Deployment stops | Console → close → redeploy with new provider |
| **WAVS releases new image** | Old operator still works | Update SDL image tag → "Update Deployment" |
| **You want to stop** | Close deployment | Console → "Close Deployment" → get AKT deposit back |

---

## Cost Breakdown

| Item | Cost | Notes |
|------|------|-------|
| Escrow deposit | 5 AKT (refundable) | One-time, returned on close |
| Compute (~3.5 CPU) | ~10-15 AKT/month | Varies by provider |
| Storage (17 GB) | ~2-3 AKT/month | Mostly IPFS |
| **Total monthly** | **~12-18 AKT/month** | At current AKT price |
| **Budget** | 63.77 AKT | **Covers 3-5 months** |

---

## The Big Picture

```
YOU (VairagyaNodes)
  │
  ├── Juno validator (unbonded, waiting 6)
  │     └── Validates Juno blocks (when bonded)
  │
  ├── JunoClaw contracts (on Juno chain)
  │     ├── agent-company v3 (DAO governance, proposals, attestations)
  │     ├── junoswap-factory (DEX pair management)
  │     ├── junoswap-pair x2 (JUNOX/USDC, JUNOX/STAKE)
  │     ├── escrow (payment management)
  │     ├── task-ledger (task tracking)
  │     └── agent-registry (agent identity)
  │
  ├── WAVS operator (on Akash) ← THIS IS WHAT WE'RE DEPLOYING
  │     ├── Watches Juno for events
  │     ├── Runs WASI verification
  │     └── Submits attestations back to Juno
  │
  ├── TEE proof (on Azure, one-time, done)
  │     └── Proposal 4: proved SGX attestation works
  │
  └── Governance proposal (on Juno mainnet, pending)
        └── Signaling prop: recognize JunoClaw as Juno infra
```

**Akash's role**: Keep the WAVS operator running 24/7 on decentralized compute so that no single server, company, or jurisdiction can shut down JunoClaw's verification layer.
