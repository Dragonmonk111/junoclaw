# Akash Deployment — LIVE ✅

> **DEPLOYED March 17, 2026** — WAVS operator stack running on Akash.

| Field | Value |
|-------|-------|
| **Status** | ✅ LIVE — all 3 containers healthy |
| **Provider** | `provider.akash-palmito.org` |
| **Aggregator URL** | `http://provider.akash-palmito.org:31812` |
| **Cost** | US$7.85/month |
| **Resources** | 3.5 CPU, 5.91 GB RAM, 18.25 GB storage |
| **Chain health** | `Cosmos chain [cosmos:juno_testnet] is healthy` |

---

## What You're Deploying

Three Docker containers on a rented Akash server:

| Container | Image | Purpose |
|-----------|-------|---------|
| `wavs-operator` | ghcr.io/lay3rlabs/wavs:1.5.1 | Watches Juno chain, runs WASI verification, submits attestations |
| `wavs-aggregator` | ghcr.io/lay3rlabs/wavs:1.5.1 | Collects results, exposes health endpoint |
| `ipfs` | ipfs/kubo:v0.37.0 | Stores the 355KB WASI component binary |

**Cost**: ~5 AKT upfront deposit (refundable) + ~12-18 AKT/month running cost
**Budget**: 63.77 AKT — covers 3-5 months

---

## Your Wallet Details

| Field | Value |
|-------|-------|
| **Akash address** | `akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta` |
| **AKT balance** | ~63.77 AKT |
| **Mnemonic source** | Same as Neo wallet (same seed, different chain prefix) |
| **How to add to Keplr** | Import via mnemonic in Keplr → Akash chain |

---

## Step A1 — Open Akash Console & Connect Keplr

1. Open a browser and go to: **https://console.akash.network**
2. Click **"Connect Wallet"** (top-right corner)
3. Select **"Keplr"**
4. Keplr will ask to add the Akash chain if not already added — click **"Approve"**
5. Keplr will ask for connection permission — click **"Approve"**
6. ✅ Verify: Your AKT balance shows ~63.77 AKT in the top-right

> **Troubleshoot**: If Akash chain isn't in Keplr, go to https://chains.keplr.app, search "Akash", click "Add to Keplr"

---

## Step A2 — Start a New Deployment

1. Click the big **"Deploy"** button (center of the page or left sidebar)
2. You'll see a deployment template selector:
   - "Hello World"
   - "Stable Diffusion"
   - "Build your template" ← **CLICK THIS ONE**
3. The editor opens with example YAML

---

## Step A3 — Paste the SDL File

1. In the editor, click the **"YAML"** tab at the top (not "Builder" or "3D" view)
2. **Select ALL the example text** in the editor (Ctrl+A)
3. **Delete it**
4. Open the file: `wavs/akash.sdl.yml` in your IDE (it's at `c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\wavs\akash.sdl.yml`)
5. Copy the entire SDL content (lines 26-141 — starting from `version: "2.0"` to the end)
   **Paste it** into the Akash Console YAML editor

> **Important**: Paste from line 26 onwards (the `version: "2.0"` line). Skip the comment lines at the top (lines 1-25 starting with `#`) — they're fine to include but not required.

6. Click **"Create Deployment"**

---

## Step A4 — Fund the Deployment Escrow

A dialog appears asking for a deposit:

1. The amount shown will be **5 AKT** (this is refundable when you close the deployment)
2. Click **"Deposit"**
3. Keplr opens a transaction confirmation — review it:
   - Amount: 5 AKT
   - To: Akash escrow contract
   - Fee: ~0.025 AKT gas
4. Click **"Approve"** in Keplr
5. ✅ Wait for TX confirmation (~6-10 seconds)
6. Console moves to the "Select Provider" screen

---

## Step A5 — Select a Provider

After the deposit confirms, providers automatically bid on your deployment. Wait 30-60 seconds.

You'll see a table like:

```
Provider          | Price/block | CPU  | Memory | Region  | Uptime
─────────────────────────────────────────────────────────────────
provider-xyz.com  | 0.82 uAKT  | AMD  | 16GB   | US-East | 99.9%
provider-abc.net  | 1.20 uAKT  | Intel| 8GB    | EU-West | 99.2%
provider-def.io   | 0.65 uAKT  | AMD  | 32GB   | Asia    | 99.7%
```

**How to pick:**
- ✅ **AMD hardware** preferred (future SEV/TEE upgrade path)
- ✅ **>99% uptime** required (operator must be reliable)
- ✅ **Lowest price** wins if both above conditions met
- ❌ Avoid providers with no uptime rating shown

1. Click on your chosen provider row to select it
2. Click **"Accept Bid"**
3. Keplr opens another TX — click **"Approve"**
4. ✅ Wait for confirmation (~6-10 seconds)

---

## Step A6 — Watch Deployment Spin Up

Console now shows your deployment in "Deploying" state. Three services will appear:

```
wavs-operator    [Deploying...]
wavs-aggregator  [Deploying...]
ipfs             [Deploying...]
```

- Each service pulls its Docker image and starts
- This typically takes **1-3 minutes**
- If any service stays "Deploying" for >5 minutes, check the Logs tab for errors

✅ When all three show **"Running"** or green, the deployment is live.

---

## Step A7 — Verify the Operator Logs

1. In Console, find the **"Logs"** tab (usually top or left sidebar of the deployment view)
2. Select **"wavs-operator"** from the dropdown
3. You should see output like:
   ```
   [INFO] wavs operator starting...
   [INFO] chain_id: uni-7
   [INFO] contract: juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6
   [INFO] connected to RPC: https://rpc.uni.junonetwork.io:443
   [INFO] polling for events...
   ```

> **If you see errors about the service ID or registry**, that's expected — the WAVS service registry is for more advanced setups. The operator still polls for events via the contract address.

4. Check **"ipfs"** logs — should see:
   ```
   Initializing daemon...
   Daemon is ready
   ```

5. Check **"wavs-aggregator"** logs — should see it start and connect to operator

---

## Step A8 — Get the Aggregator Public URL

This is the external URL that the bridge tools use to connect to the Akash deployment.

1. In Console, find the **"Leases"** tab or **"Services"** section
2. Look for **"wavs-aggregator"** service
3. Find the **"Public endpoints"** or **"Exposed ports"** section
4. Copy the URL — it looks like:
   ```
   https://provider-xyz.akash.network:31234
   ```
   or
   ```
   http://3.89.145.22:31234
   ```
5. **Write this URL down** — paste it back to me so I can update the bridge `.env`

---

## Step A9 — Quick End-to-End Test

To confirm the operator is actually watching the chain:

1. In the Akash Console Logs tab → **wavs-operator**
2. In a separate window, check the most recent Juno block on: https://www.mintscan.io/juno-testnet
3. Within 5-10 seconds of a new block, you should see the operator log "polling..." or "no new events"

✅ If you see regular log output, the operator is live.

---

## Step A10 — Record Deployment Details

Once everything is running, note down:

```
Deployment ID:       _________________________________
Provider:            _________________________________
Aggregator URL:      _________________________________
Deployment date:     March 17, 2026
Monthly cost:        _____________ AKT/month
Lease ID:            _________________________________
```

**Send me the Aggregator URL** and I will:
1. Update `wavs/bridge/.env` with `WAVS_AGGREGATOR_URL=<your-url>`
2. Update all docs with deployment confirmed status
3. Move to Task B: Governance proposal submission

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Keplr doesn't show AKT balance | Add Akash chain via https://chains.keplr.app |
| No bids after 2 minutes | Your SDL resource requirements may be too high — let me know |
| Service stuck "Deploying" | Check logs for pull errors — image may be rate-limited |
| wavs-operator crashes on start | Check env vars in the SDL — all addresses look correct |
| IPFS won't start | Storage allocation issue — provider may not have enough disk |
| Can't find aggregator URL | Look under "Leases" → lease row → "Services" → "wavs-aggregator" → "Ports" |

---

## After Akash — Next Is Task B

Once you have the aggregator URL:
1. **Paste it back to me** (I'll update the config)
2. Then we move to **Task B: Juno governance proposal**
   - Open: https://ping.pub/juno/gov
   - File ready: `docs/GOV_PROP_COPYPASTE.md` (I'll create this now)
   - Sign with Keplr (VairagyaNodes wallet)
