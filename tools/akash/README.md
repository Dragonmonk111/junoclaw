# JunoClaw Autonomous Akash Deployer

Budget-capped autonomous signing for J-Lens GPU deployments on Akash (A039/A040).

## Overview

This tool lets the JunoClaw agent sign Akash deployment transactions autonomously,
without human approval per-transaction, while bounding risk through:

- **Spend cap**: Pre-fund a dedicated wallet with exact session cost. The script
  rejects any bid that would exceed the cap.
- **Auto-close timer**: Deployment is automatically closed after N minutes,
  stopping billing regardless of agent state.
- **Moultbook audit trail**: Every signed transaction (create, lease, close) is
  posted to the Moultbook contract on Juno mainnet as an immutable audit entry.
- **Encrypted keyring**: Wallet mnemonic is stored in the encrypted WalletStore
  (DPAPI/Keychain), decrypted only for the duration of keyring setup, then scrubbed.
- **Temp keyring**: A temporary filesystem keyring is created and destroyed each
  session — no persistent key material on disk.

## Architecture

```
  ┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
  │  WalletStore     │     │  auto-deploy.mjs │     │  Akash Chain     │
  │  (encrypted)     │────▶│                  │────▶│  (akash-2)       │
  │  akash-jlens     │     │  - decrypt       │     │                  │
  └──────────────────┘     │  - temp keyring  │     │  MsgCreateDeploy │
                           │  - create deploy │     │  MsgCreateLease  │
  ┌──────────────────┐     │  - select bid    │     │  MsgCloseDeploy  │
  │  Moultbook       │◀────│  - create lease  │     └──────────────────┘
  │  (juno-1)        │     │  - auto-close    │
  │  audit entries   │     │  - audit post    │
  └──────────────────┘     └──────────────────┘
```

## Prerequisites

1. **Akash CLI** installed in WSL (Ubuntu-24.04) — the script auto-detects Windows
   and invokes `akash` via `wsl.exe`. On Linux, `akash` must be on PATH.
   - Install: `curl -sSfL https://raw.githubusercontent.com/akash-network/node/main/install.sh | sh -s -- -b /usr/local/bin`
   - Requires GLIBC 2.38+ (Ubuntu 24.04, not 22.04)
2. **MCP WalletStore** built (`cd mcp && npm run build`)
3. **Dedicated Akash wallet** generated via the script (see Funding Flow below):
   ```bash
   node tools/akash/generate-akash-wallet.mjs --id akash-jlens --yes
   ```
4. **ACT tokens** minted from AKT via BME (post-v2.0 upgrade, deposits use `uact` not `uakt`):
   ```bash
   akash tx bme mint-act 23000000uakt --from mykey --node $AKASH_NODE --chain-id akashnet-2
   ```
5. **Juno wallet** for Moultbook audit posts (existing builder wallet works):
   ```bash
   # Already registered as "builder" in WalletStore
   ```

## Funding Flow (JUNO → AKT → Akash wallet)

### Step 1: Generate a dedicated Akash wallet

```bash
# Dry-run first (no key material generated)
node tools/akash/generate-akash-wallet.mjs --id akash-jlens

# Generate for real — mnemonic is created inside WalletStore.generateAndAdd()
# and never leaves that function. Not printed, not logged, not returned.
node tools/akash/generate-akash-wallet.mjs --id akash-jlens --yes

# The output will show the wallet address. Send AKT to that address.
# The mnemonic exists only in the encrypted WalletStore file, decryptable
# only via this machine's DPAPI/Keychain.
```

### Step 2: Fund the wallet with AKT, then mint ACT

**Post-v2.0 Akash uses a dual-token model:**
- **AKT (uakt)**: staking token, used for gas fees
- **ACT (uact)**: compute token (USD-pegged), used for deployment deposits and lease payments

**Route A: Direct AKT purchase + IBC transfer**

1. Buy AKT on an exchange (e.g. Kraken, KuCoin) or Osmosis
2. Withdraw AKT to your `akash1...` address on akashnet-2 chain
3. Mint ACT: `akash tx bme mint-act 23000000uakt --from mykey --node ... --chain-id akashnet-2`
   (23 AKT at ~$0.44/AKT mints ~10 ACT, the minimum mint amount)

**Route B: JUNO → Osmosis swap → IBC to Akash**

1. Send JUNO from your Juno wallet to Osmosis (IBC channel-0 → channel-42)
2. Swap JUNO for AKT on Osmosis DEX
3. Withdraw AKT from Osmosis to your `akash1...` address (IBC channel-15 → channel-9)
4. Mint ACT as above

**Route C: Use existing Akash wallet**

The existing WAVS deployment wallet (`akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta`)
has ~63.77 AKT. You can import its mnemonic into WalletStore under a new ID.

### Step 3: Verify funding

```bash
# Check both uakt (gas) and uact (compute) balances
akash query bank balances akash1... --node https://akash-rpc.polkachu.com:443
```

## Cost Estimation

| Session type | GPU | Duration | Est. cost |
|---|---|---|---|
| Phase 1: Mixtral 8x7B validation | 1x A100/L40S | 2 hours | ~3-6 ACT (~$3-6) |
| Phase 3: Kimi K3 one-shot | 4x H100 | 4-6 hours | ~50-100 ACT (~$50-100) |

**Always pre-fund the exact session cost + 20% buffer in ACT.** Keep at least
0.5 AKT for gas fees. The spend cap in the script prevents overspend even if
bids come in higher than expected.

## Usage

### Basic deployment

```bash
node tools/akash/auto-deploy.mjs \
  --sdl tools/akash/sdl-mixtral-8x7b.yml \
  --wallet-id akash-jlens \
  --max-spend-uakt 6000000 \
  --timeout-minutes 120 \
  --moultbook-addr juno1r59ulw66alrv7s65egfk03zqs28yz04ajnl95r877e85mx8h7qnq8ze2w5 \
  --juno-wallet-id builder
```

### Dry run (no broadcast)

```bash
node tools/akash/auto-deploy.mjs \
  --sdl deploy.yml \
  --wallet-id akash-jlens \
  --max-spend-uakt 5000000 \
  --timeout-minutes 60 \
  --dry-run
```

### Without Moultbook audit (testing only)

```bash
node tools/akash/auto-deploy.mjs \
  --sdl deploy.yml \
  --wallet-id akash-jlens \
  --max-spend-uakt 5000000 \
  --timeout-minutes 60
  # No --moultbook-addr or --juno-wallet-id = audit posts skipped
```

## What the script does

1. **Decrypts** the Akash wallet mnemonic from encrypted WalletStore
2. **Creates** a temporary filesystem keyring (scrubbed on exit)
3. **Creates** a client certificate (generate + publish to chain)
4. **Broadcasts** `MsgCreateDeployment` with the SDL file and uact deposit
5. **Waits** 60 seconds for bids to arrive
6. **Selects** the cheapest bid within the spend cap (rejects if none qualify)
7. **Broadcasts** `MsgCreateLease` with the selected provider
8. **Posts** audit entries to Moultbook (deployment_created, lease_created, deployment_active)
9. **Sets** an auto-close timer for the specified timeout
10. **Heartbeats** every 60 seconds with deployment status
11. **On exit** (Ctrl+C, SIGTERM, or timeout): broadcasts `MsgCloseDeployment` and posts final audit

## Security model

| Threat | Mitigation |
|---|---|
| Key theft from disk | Encrypted WalletStore (DPAPI/Keychain), temp keyring scrubbed |
| Runaway billing | Auto-close timer + spend cap on bid selection |
| Unauthorized signing | WalletStore `signingPaused` kill-switch respected |
| No audit trail | Every tx posted to Moultbook (immutable, on-chain) |
| Process crash | SIGTERM/SIGINT handlers close deployment before exit |
| Key in memory | Mnemonic scrubbed after keyring setup, buffers zeroed |

## Files

- `generate-akash-wallet.mjs` — Generate a new dedicated Akash wallet in WalletStore (highest-safety path: mnemonic never leaves the WalletStore module)
- `auto-deploy.mjs` — Main autonomous deployment script
- `audit-post.mjs` — Moultbook audit posting helper
- `sdl-mixtral-8x7b.yml` — SDL template for Phase 1 validation (single L40S, vLLM)
- `sdl-kimi-k3.yml` — SDL template for Phase 3 Kimi K3 (4x H100, llama.cpp — placeholder pending provider availability)

## ICA future path

This tool uses a builder/agent wallet for signing. A future proposal could set up
an Interchain Account (ICA) so the Juno Agents DAO contract directly controls an
Akash-chain account, signing deployments via governance proposals — no human-operated
wallet needed at all. See memory tag `ica` for details. Out of scope for A039/A040.
