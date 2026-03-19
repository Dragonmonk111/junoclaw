# Akash Deep Integration — Legal, Social, Dev Community & Console Guide

## JunoClaw x Akash Network

**Date**: March 17, 2026
**Status**: 63.77 AKT in wallet, SDL ready, WAVS component proven in TEE
**Key Contact**: Greg Osuri (@gregosuri) — Akash CEO, followed on X

---

## Part 1: Greg Osuri & Akash Leadership Context

### Who Is Greg Osuri

Greg Osuri is the CEO and co-founder of Overclock Labs, the company behind Akash Network. He's been building Akash since 2018 and is one of the most visible leaders in the decentralized compute space.

**Key facts for JunoClaw context:**
- He actively engages with builders on X — responds to projects deploying on Akash
- He's vocal about Akash being the "Airbnb for cloud compute" — our narrative fits perfectly
- He has publicly supported Cosmos ecosystem projects getting compute subsidies
- Overclock Labs runs the Akash Accelerate grants program
- He's interested in AI/agent use cases on Akash — JunoClaw is exactly this

**What to watch for on his X feed:**
- Akash GPU marketplace updates (relevant for future Akash LLM tier)
- New provider onboarding announcements (TEE-capable providers)
- Grants/credits programs for builders
- Partnerships with AI/agent projects
- Akash Console updates (our primary deployment interface)

### Staying Updated

1. **Follow @gregosuri on X** — already done
2. **Follow @akashnet_** — official Akash account
3. **Watch Akash GitHub releases** — https://github.com/akash-network/console
4. **Join Akash Discord #announcements** — https://discord.akash.network
5. **Subscribe to Akash blog** — https://akash.network/blog

---

## Part 2: Legal Considerations

### 2.1 Akash Network — Legal Structure

Akash Network is an **open-source, permissionless, decentralized cloud marketplace**. Key legal properties:

- **No KYC required** — anyone can deploy, anyone can provide compute
- **Payment in AKT only** — no fiat, no credit cards, pure crypto
- **No Terms of Service for deployers** — the network is permissionless
- **Provider responsibility** — each provider sets their own acceptable use policy
- **Open-source protocol** — Apache 2.0 licensed

### 2.2 JunoClaw's Legal Position on Akash

**What we're deploying**: A WAVS operator that watches Juno chain events and produces TEE-attested verification results. This is:

- **Not a financial service** — we're running verification compute, not custody or trading
- **Not processing personal data** — all data is from public blockchain state
- **Open-source** — full code at github.com/Dragonmonk111/junoclaw
- **Deterministic** — same input always produces same output (WASI component)

**Regulatory considerations:**

| Area | Risk | Mitigation |
|------|------|-----------|
| **Compute hosting** | Low — Akash is permissionless | No ToS violation — running open-source verification |
| **DEX operations** | Medium — Junoswap may be a "DeFi protocol" | JunoClaw verifies, it doesn't operate the DEX. The agent is a watcher, not a market maker. |
| **TEE attestation** | Low — hardware proofs are standard | Attestation is proof of computation, not a financial claim |
| **Token usage** | Low — AKT for compute, JUNO for gas | Standard Cosmos token usage, no novel tokenomics |
| **Cross-chain data** | Low — reading public blockchain state | All queried data is publicly available on-chain |
| **AI agent liability** | Emerging area — no clear precedent | Agent operates within DAO governance constraints, all actions are on-chain and auditable |

**Key principle**: JunoClaw's agent does not make autonomous financial decisions. It verifies data that the DAO governance process has already approved. The TEE attestation proves the computation was correct. This is fundamentally a **verification service**, not a **trading service**.

### 2.3 Jurisdiction Notes

- Akash is a Cosmos SDK chain — no single jurisdiction
- Overclock Labs is incorporated in the US (San Francisco)
- JunoClaw operates on Juno Network — also no single jurisdiction
- The WAVS operator on Akash is hosted by whoever accepts the deployment bid — could be any country
- **Best practice**: Keep all operations transparent, open-source, and auditable

### 2.4 IP & Licensing

| Component | License | Notes |
|-----------|---------|-------|
| JunoClaw contracts | Apache 2.0 | Open-source, permissive |
| WAVS (Layer.xyz) | Apache 2.0 | Open-source |
| Akash SDL | N/A | Configuration, not code |
| Junoswap v2 (Astroport fork) | Apache 2.0 | Astroport core is Apache 2.0 |
| Akash Console | Apache 2.0 | Open-source |

No licensing conflicts. All components are Apache 2.0 compatible.

---

## Part 3: Social & Dev Community Impact

### 3.1 Why This Matters to the Cosmos Ecosystem

JunoClaw on Akash is a **proof of composability** — Cosmos chains working together:

```
Juno (smart contracts) + Akash (compute) + WAVS (verification) = Verifiable AI Agent
```

This is the story Greg Osuri and the Akash team want to tell. Cosmos isn't just chains talking to each other via IBC — it's chains providing **specialized services** to each other:
- Juno provides the smart contract layer
- Akash provides the compute layer
- WAVS provides the verification layer
- The agent bridges all three

### 3.2 Community Touchpoints

| Community | How JunoClaw Contributes | Engagement Strategy |
|-----------|------------------------|-------------------|
| **Akash** | First WAVS operator on Akash. Proves AI agent workloads run on decentralized compute. | Deploy publicly, write about it, tag @akashnet_ and @gregosuri |
| **Juno** | Revives Junoswap, brings TEE verification to chain. Validator proposal thread active. | Validator proposal, community pool discussion, Jake Hartnell endorsement |
| **WAVS/Layer.xyz** | First production WAVS deployment outside Layer.xyz team. Proves the framework works. | Jake already engaged. Continue building, report bugs upstream. |
| **Cosmos Hub** | Demonstrates Cosmos composability narrative. | Cross-post to Cosmos forum, tag @cosmos on milestone tweets |
| **DeFi community** | TEE-attested price oracle is novel — no other Cosmos DEX has this. | Write explainer articles, present at Cosmos events |

### 3.3 Greg Osuri Engagement Strategy

**Do NOT cold-pitch Greg directly with an ask.** Instead:

1. **Deploy first, talk second** — Get WAVS running on Akash Console. Have the deployment live.
2. **Tweet the deployment** — Tag @gregosuri and @akashnet_ with the live deployment proof
3. **Show the narrative** — "First WAVS TEE operator on Akash — Cosmos-native compute for Cosmos-native AI"
4. **Then engage** — After the tweet gets traction, DM or reply with the grants question

**Draft tweet for after deployment:**

> JunoClaw's WAVS operator is now live on @akashnet_ — the first TEE-attested AI agent running on decentralized compute in Cosmos.
>
> Stack: Juno (contracts) + Akash (compute) + WAVS (verification)
>
> Cosmos composability isn't just IBC. It's chains providing specialized services to each other.
>
> Every swap on Junoswap v2 is verified by a WASI component running inside an SGX enclave on Akash. The attestation hash lands on Juno. Fully auditable. Fully decentralized.
>
> SDL, contracts, component — all open source:
> github.com/Dragonmonk111/junoclaw
>
> @gregosuri @Jake_Hartnell

### 3.4 Dev Community Building

**What other devs can learn from JunoClaw on Akash:**

1. **Multi-container SDL patterns** — WAVS operator + aggregator + IPFS
2. **CosmWasm + Akash integration** — how to wire a contract to off-chain compute
3. **TEE on Akash** — which providers support AMD SEV, how to verify
4. **WASI components on decentralized infra** — the new paradigm

**Content to create after deployment:**
- Medium article: "Running a TEE-Attested AI Agent on Akash"
- GitHub README update with Akash deployment instructions
- Akash Forum case study post
- Short video walkthrough of Console deployment

---

## Part 4: Step-by-Step Akash Console Integration

### Prerequisites

| Item | Status | Notes |
|------|--------|-------|
| AKT in wallet | 63.77 AKT | More than enough |
| Keplr wallet | Required | Connect to console.akash.network |
| SDL file | Ready | `wavs/akash.sdl.yml` with env vars |
| WAVS component | Built | 355KB WASI component, TEE-proven |

### Step 1 — Open Akash Console

1. Navigate to **https://console.akash.network**
2. Click **"Connect Wallet"** (top right)
3. Select **Keplr** — approve the connection
4. Verify your balance shows ~63.77 AKT

### Step 2 — Create New Deployment

1. Click **"Deploy"** button
2. Select **"Build your template"**
3. Click **"YAML"** tab (not the visual editor)
4. Paste the entire contents of `wavs/akash.sdl.yml`
5. Click **"Create Deployment"**

### Step 3 — Fund the Deployment Escrow

1. Akash requires a **5 AKT escrow deposit** to create the deployment
2. This deposit is refundable when you close the deployment
3. Actual compute costs are deducted from this deposit over time
4. Click **"Deposit"** and approve the Keplr transaction

### Step 4 — Select a Provider

After deposit, providers will bid on your deployment:

1. Wait ~30-60 seconds for bids to appear
2. You'll see providers with their prices and specs
3. **Prefer providers with:**
   - AMD EPYC hardware (for potential SEV support)
   - Low price per block
   - Good uptime reputation (shown in Console)
4. Click **"Accept Bid"** on your chosen provider
5. Approve the Keplr transaction

### Step 5 — Wait for Deployment

1. Console will show **"Deployment Active"** status
2. All 3 services should transition to **"Running"**:
   - `wavs-operator` — Running
   - `wavs-aggregator` — Running
   - `ipfs` — Running
3. This typically takes 1-3 minutes

### Step 6 — Get the Aggregator Endpoint

1. In Console, go to your deployment → **"Leases"** tab
2. Find the `wavs-aggregator` service
3. Copy the **public endpoint** — this is the globally-accessible URL
4. Format: `https://<random>.provider.akash.network:8080`

### Step 7 — Verify Services Are Running

1. In Console → **"Logs"** tab — check each service for errors
2. Test the aggregator endpoint:
   ```
   curl https://<aggregator-endpoint>/health
   ```
3. Check IPFS:
   - In Console → **"Shell"** tab → select `ipfs` service
   - Run: `ipfs id` — should show peer ID

### Step 8 — Update Bridge Configuration

Update `wavs/bridge/.env` with the Akash endpoint:

```
WAVS_AGGREGATOR_URL=https://<aggregator-endpoint>
```

### Step 9 — End-to-End Verification

1. Create a test proposal on agent-company contract
2. WAVS operator on Akash detects the event
3. Component runs, produces attestation
4. Bridge submits attestation to Juno uni-7
5. Query the attestation to verify it came from Akash

### Step 10 — Monitor & Maintain

- **Console dashboard** — check deployment health daily
- **AKT balance** — monitor escrow depletion rate
- **Logs** — check for errors or missed events
- **Top up escrow** — add more AKT before it runs out

---

## Part 5: Keeping JunoClaw Updated with Akash

### Automated Monitoring Checklist

| What to Monitor | How | Frequency |
|----------------|-----|-----------|
| Akash Console releases | GitHub: akash-network/console | Weekly |
| Greg Osuri tweets | @gregosuri on X | Daily |
| Akash blog | akash.network/blog | Weekly |
| AKT price (affects compute cost) | CoinGecko/Osmosis | Weekly |
| Provider TEE support | Akash Discord #providers | Monthly |
| WAVS image updates | ghcr.io/lay3rlabs/wavs | Per release |
| Akash SDL spec changes | docs.akash.network | Per release |
| Akash grants program | akash.network/community | Monthly |

### Key Akash Milestones to Watch

1. **GPU marketplace expansion** — When Akash adds more GPU providers, JunoClaw can run LLM inference on Akash (currently local/Ollama only)
2. **TEE provider certifications** — When Akash formally certifies TEE-capable providers, we can require TEE in our SDL
3. **Persistent storage** — Akash persistent storage improvements affect IPFS and component storage reliability
4. **Akash x WAVS** — If Layer.xyz (WAVS) and Akash formalize a partnership, JunoClaw is the reference implementation
5. **Akash Console v2** — Deployment management improvements affect our workflow

### Future Integration Opportunities

| Opportunity | Description | When |
|-------------|-------------|------|
| **Akash GPU for LLM** | Run Ollama/vLLM on Akash GPU instead of local | When GPU availability improves |
| **Multi-region operator** | Deploy WAVS operators in multiple Akash regions for redundancy | After mainnet launch |
| **Akash x Juno IBC** | Direct IBC channel for AKT payment from Juno DAO treasury | If Juno community supports |
| **Provider staking** | JunoClaw becomes an Akash provider-staker for guaranteed compute | Long-term |
| **Akash Accelerate grant** | Apply for grant to fund 12-month deployment | After live deployment proof |

---

## Part 6: Akash-Specific Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **Provider goes offline** | Operator stops, attestations delayed | Auto-redeploy on different provider via Console. Keep bridge as fallback on local. |
| **AKT price spike** | Compute becomes expensive | Pre-fund escrow during low-price periods. Apply for grants. |
| **AKT price crash** | Provider economics worsen, fewer providers | Low risk for us — we benefit from cheaper compute |
| **SDL spec breaking change** | Deployment fails on upgrade | Pin Akash provider version. Test before upgrading. |
| **TEE not available** | Software attestation only (still works) | Document clearly which tier of attestation is active |
| **WAVS image update** | Need to redeploy with new image | Monitor ghcr.io/lay3rlabs/wavs tags. Redeploy via Console. |
| **Network congestion** | Deployment creation slow/expensive | Deploy during off-peak. Keep deployment running continuously. |

---

## Appendix: Akash Console Keyboard Shortcuts & Tips

- **Deployment list**: Console home → "Deployments" sidebar
- **Quick redeploy**: Edit existing deployment SDL → "Update Deployment"
- **Shell access**: Deployment → "Shell" tab → select service → run commands
- **Log streaming**: Deployment → "Logs" tab → real-time container logs
- **Cost tracking**: Deployment → "Overview" → shows AKT spent/remaining
- **Multiple deployments**: You can run multiple deployments simultaneously
- **Escrow management**: Settings → "Deposits" → top up or close deployments
