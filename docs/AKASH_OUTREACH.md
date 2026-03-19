# Akash Outreach — All Links + Exact Messages
## JunoClaw WAVS Operator Deployment

**Updated**: March 17, 2026 — TEE milestone PROVEN on Azure (proposal 4, TX `6EA1AE79...D26B22`)
**Scope expanded**: Jake Hartnell directed Junoswap revival + Neutron DeFi fork integration

---

## All Akash Community Links

| Platform | Link | Best for |
|----------|------|---------|
| **Discord** | https://discord.akash.network | Fastest responses — providers, devs |
| **Forum** | https://forum.akash.network | Formal proposals, credit requests |
| **Twitter/X** | https://x.com/akashnet_ | Public visibility, team DMs |
| **GitHub** | https://github.com/akash-network | Technical issues |
| **Console** | https://console.akash.network | Deploy directly (no CLI needed) |
| **Website** | https://akash.network | Docs, pricing info |
| **Docs** | https://docs.akash.network | SDL reference, deployment guide |

---

## Step 1 — Get AKT First (You Need This to Deploy)

Akash deployments are paid in AKT. You need some before you can do anything.

**Cheapest/fastest: Osmosis DEX** (you're already in Cosmos)
1. Go to https://app.osmosis.zone
2. Connect Keplr
3. Swap JUNOX or ATOM → AKT
4. You need **~10 AKT minimum** (~$10) for a test deployment
5. **50 AKT** (~$50) covers ~30 days persistent operator

**Alternatively**: Buy AKT on any CEX (Kraken, Coinbase, Kucoin) and send to your Keplr Akash address.

---

## Step 2 — Discord: What to Post

### Channel: `#providers`

Post this exactly:

---
> **[JunoClaw] TEE-proven WAVS operator looking for permanent Akash home (AMD EPYC/SEV)**
>
> Hey Akash community 👋
>
> I'm building JunoClaw — an agentic DAO on Juno that uses WAVS (WebAssembly Verifiable Services by Jake Hartnell / Layer.xyz) for hardware-attested off-chain compute.
>
> **Already proven**: We just ran our WASI component inside an Intel SGX enclave on Azure DCsv3 and submitted the first hardware-attested WAVS result in Cosmos. TX: `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` on Juno testnet.
>
> Now we want to move to **permanent decentralised hosting on Akash** — Cosmos-native compute for a Cosmos-native DAO.
>
> Looking for:
> - A provider with **AMD EPYC hardware and SEV enabled** (need `/dev/sev` accessible in container)
> - 3 containers: WAVS operator (2 vCPU/4GB) + aggregator (1 vCPU/1GB) + IPFS (0.5 vCPU/512MB)
> - Port **8080 exposed globally**
> - Persistent 24/7 — this is production, not a demo
>
> Scope is expanding: Jake Hartnell directed us to point the agent at reviving Junoswap and forking dead Neutron DeFi projects. The operator needs to be always-on.
>
> Questions:
> 1. Which providers have confirmed `/dev/sev` accessible inside containers?
> 2. What's the AKT cost for this spec for 30 days?
> 3. Does Akash have a **community grant or credit program** for open-source Cosmos projects?
>
> Repo: https://github.com/Dragonmonk111/junoclaw
> Fully open source. TEE proof already on-chain.

---

### Channel: `#deployments`

Post this for technical help when you're ready to deploy:

---
> Anyone have a working SDL for a multi-container Docker Compose setup on Akash?
> I need to run WAVS (wavs-operator + wavs-aggregator containers) with port 8080 globally exposed.
> The containers are from ghcr.io/lay3rlabs/ — happy to share the SDL draft for review.

---

## Step 3 — Twitter DM to @akashnet_

Send this as a DM:

---
> Hey Akash team — JunoClaw just produced the first hardware-attested WAVS result in Cosmos (Intel SGX on Azure, TX on Juno testnet). Jake Hartnell is directing us to expand scope: revive Junoswap + fork dead Neutron DeFi protocols, all verified by TEE compute.
>
> We want to move the operator permanently to Akash — Cosmos-native compute for a Cosmos-native DAO. Multi-container: WAVS operator + aggregator + IPFS.
>
> Two asks:
> 1. Does Akash have a community grant/credit program for open-source Cosmos projects?
> 2. Can you point us to an AMD EPYC provider with SEV enabled?
>
> Repo: https://github.com/Dragonmonk111/junoclaw
> TEE proof already on-chain. Will write about the Akash migration publicly.

---

## Step 4 — Forum Post (forum.akash.network)

Category: **General / Projects**

**Title**: JunoClaw — Running a WAVS TEE Operator on Akash (Cosmos-native compute for Cosmos-native DAO)

---
> ## What We're Building
>
> JunoClaw is an agentic DAO on Juno that uses WAVS (WebAssembly Verifiable Services) for hardware-attested off-chain compute. When a proposal passes on-chain, a WASI component runs inside a TEE enclave, computes a verification hash, and the signed result is submitted back on-chain.
>
> **Already proven**: Proposal 4 on Juno testnet is the first hardware-attested WAVS result in Cosmos — executed inside Intel SGX on Azure DCsv3.
> TX: `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22`
>
> ## Why Akash
>
> The Cosmos ecosystem should run on Cosmos infrastructure. Jake Hartnell (Juno co-founder, WAVS architect at Layer.xyz) is directing us to expand scope: revive Junoswap, fork dead Neutron DeFi protocols — all verified by the same TEE compute layer. The operator needs to be always-on. Akash is the right home.
>
> ## What We Need
>
> - 3 containers: WAVS operator (2 vCPU/4GB) + aggregator (1 vCPU/1GB) + IPFS (0.5 vCPU/512MB)
> - Port 8080 globally exposed
> - Ideally: AMD EPYC provider with `/dev/sev` accessible (for AMD SEV hardware attestation)
> - Persistent 24/7 — this is production infrastructure, not a demo
>
> ## Ask
>
> Does Akash have a community grant or credit program for open-source Cosmos projects?
> We're a zero-revenue open-source project building core Juno infrastructure. Any credits toward the first 30-day deployment would be meaningful and we'll document and publish the integration publicly.
>
> Repo: https://github.com/Dragonmonk111/junoclaw
> TEE proof on-chain. SDL ready. Scope expanding.

---

## Step 5 — Twitter Public Thread (Optional, High Visibility)

Post this publicly tagging both Akash and WAVS:

> JunoClaw just produced the first hardware-attested WAVS result in Cosmos — Intel SGX enclave on Azure, attestation hash on Juno testnet.
>
> TX: 6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22
>
> Now moving the operator to @akashnet_ for permanent decentralised hosting.
>
> @Jake_Hartnell is directing us to expand: revive Junoswap + fork dead Neutron DeFi protocols. All verified by the same TEE compute layer.
>
> Cosmos-native compute on Cosmos-native infrastructure. Anyone in the Akash community run containers with /dev/sev accessible? 👇
>
> github.com/Dragonmonk111/junoclaw

---

## Akash Credit Programs (Check These)

1. **Akash Accelerate** — https://akash.network/community/grants
   - Community grants for builders in the Cosmos ecosystem
   - Apply with GitHub repo + description of use case

2. **Overclock Labs (Akash core team)** — @gregosuri on Twitter
   - Founders are responsive to builders in Cosmos
   - DM directly with the project description

3. **Juno Community Pool** — ask the Juno community to fund the AKT
   - A signal proposal: "Juno community pool funds AKT for JunoClaw operator on Akash"
   - Small ask (~50 AKT) but strong narrative

---

## What You Can Deploy Without Card / Fiat

Everything on Akash is paid in AKT. No credit card needed at all. Pure crypto.

If you have any ATOM, OSMO, or JUNOX you can swap to AKT on Osmosis and deploy immediately.

The `console.akash.network` UI accepts Keplr wallet — no account, no signup, no card.
