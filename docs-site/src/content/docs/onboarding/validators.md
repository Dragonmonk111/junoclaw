---
title: Validators — how the G1 set works
description: How the JunoClaw G1 validator set is configured — equal weight, hybrid signing, the key ceremony and node requirements.
---

<p class="jc-kicker">The validator set · G2 public testnet</p>

**G1 is closed.** The first multi-operator ceremony ran a 48.2-hour soak across independent operators: 361,709 finalized blocks (heights 512,776 → 874,485), zero state divergence, and one documented 106-minute wedge — root-caused to two seats sharing a host's failure domain, published in full, and the fix is now an onboarding rule. **G2 opens the set to external operators and grows it toward n=5, then n=7 for mainnet.** This page is the reference for how the validator set works.

## What a validator does

| | |
|---|---|
| **Set** | Equal weight; external operators join in batched, devnet-rehearsed set-change windows |
| **Weight** | Equal. One BLS threshold share per validator, with no stake weighting |
| **Consensus** | Commonware Simplex BFT, hybrid BLS12-381 + MAYO2 certificates |
| **Staking / slashing** | None at the consensus layer. Safety comes from the BLS threshold, and economic stake lives in the truth-market contracts |
| **Rewards** | **None. This is an unpaid testnet.** Early operators are credited publicly and are first in line for later phases |
| **Set changes** | Static set. Changes are coordinated config + binary upgrades at an agreed height |

## What you need

### The hard rule: independent failure domains

G1's one wedge came from two seats sharing a host *and* an ISP — one outage removed two signers and dropped the set below quorum. It is the lesson G2 is built on:

- **Your seat must be its own failure domain** — dedicated host and network. No two seats may share a machine or an ISP.
- **Failover is active/passive only.** A backup that takes over when the primary dies is fine; two live signers for one seat is a double-sign. Never run your key twice.
- **Clock discipline is consensus-critical** — the wedge's tail was extended by ~83 s of drift. NTP-synced clock with **<2 s drift**, verified before each window.
- **A named ops contact** who answers a test-page during the testnet window.

### Hardware (G2 baseline)

| Resource | Minimum | Recommended |
|---|---|---|
| CPU | 2 cores | 4 cores |
| RAM | 4 GB | 8 GB |
| Disk | 20 GB SSD | 50 GB SSD |
| Network | 10 Mbps, stable | 100 Mbps, **stable public IP** |

The devnet runs all 4 hybrid validators on a single desktop machine, so these figures leave plenty of headroom. Pruning tiers keep disk use bounded: a validator keeps the last 540,000 heights of sidecar data and finality records (about 24 hours at devnet speed).

### Software and network

- Linux (Ubuntu 22.04+ recommended) with Docker, or Rust 1.97+ to build from source. Key generation and MAYO2 signing need a Unix host, so Windows cannot hold a hybrid seat
- **Inbound TCP `7001`** (P2P) reachable by the other validators
- **No static IP at home?** Peers dial each other at a fixed `IP:7001` (IP addresses, not DNS names), so the address must not change. A small cloud VM works, or a cheap VPS that forwards TCP `7001` to your home node over WireGuard. Cloudflare Tunnel is not a good fit for P2P
- `9090` (gRPC), which can stay private. Exposing it is optional
- **NTP clock sync.** Simplex timeouts depend on wall-clock time
- Monitoring/alerting you already trust. We'll ask you to alert on finalized-height stalls

### Operator commitments

- Be reachable on Telegram during the testnet window, especially for coordinated upgrades
- Keep your node up and report incidents. The testnet exists to find problems, so honest reports are the most valuable thing you can contribute
- Never share `keys.json`. It holds your secret BLS (and MAYO2) share and your Ed25519 P2P key

## How to apply

Post in the **[JunoClaw Telegram](https://t.me/junoclaw)** (or reply in the Juno validators channel) with:

1. **Operator name** and your Telegram handle
2. **Validator experience.** Chains you validate or have validated, e.g. Juno mainnet
3. **Hosting.** Provider/region, bare metal or VPS, and whether you have a static IP
4. **Hardware** you'll dedicate
5. **Confirmation it's an independent failure domain** (provider + network)
6. **Timezone** and availability for the set-change window

:::caution[Safety]
Admins never DM first. Nobody from JunoClaw will ever ask for your keys, mnemonic or funds. G2 has no token sale and nothing to pay for.
:::

## How the set was formed

1. **Selection.** Operators are picked for diversity across hosting providers and regions.
2. **Key ceremony.** Each operator generates their Ed25519 identity and MAYO2 key locally, and those never leave your machine. The coordinator then deals the BLS threshold shares from OS randomness and sends each operator theirs, encrypted, and deletes its copies. The coordinator briefly sees every BLS share, but a hybrid certificate also needs a quorum of MAYO2 signatures, so the coordinator alone cannot forge one. A dealer-free DKG is planned before mainnet. Accepted operators get the procedure in advance.
3. **Genesis.** You receive the genesis parameters, the threshold public key and the peer list (Ed25519 public keys + addresses), and you fill in your `node.toml`.
4. **Connectivity check.** Every operator confirms they can reach every peer on `7001` before genesis time.
5. **Launch.** Everyone starts their nodes, and we confirm the first certificate finalizes with every operator's share present.

Ready to see what the node config looks like? → [Run a node](/docs/reference/run-a-node/)
