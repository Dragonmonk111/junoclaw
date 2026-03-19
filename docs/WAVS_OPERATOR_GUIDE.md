# WAVS Operator Setup Guide
## JunoClaw — Closing the TEE Gap

**Goal**: Run a real WAVS operator node on Azure DCsv3 (Intel SGX) so the WASI component
executes inside a TEE enclave and hardware-signs the attestation, replacing the local
software operator.

---

## How WAVS Actually Runs (Architecture Reality Check)

WAVS is **not a single binary**. It is a Docker Compose stack of services:

```
┌─────────────────────────────────────────────────────┐
│  WAVS Docker Compose Stack (on your Linux server)   │
│                                                     │
│  wavs-operator   ← watches Juno for trigger events  │
│                    runs WASI component in TEE        │
│                    signs result with operator key    │
│                                                     │
│  wavs-aggregator ← collects signed operator results │
│                    exposes HTTP API on :8080         │
│                                                     │
│  (supporting: prometheus, grafana, etc.)            │
└─────────────────────────────────────────────────────┘
         │
         │  HTTP poll (bridge daemon, runs anywhere)
         ▼
┌─────────────────────────────────────────────────────┐
│  bridge.ts (our TypeScript daemon)                  │
│  polls aggregator → submits to agent-company on Juno│
└─────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────┐
│  agent-company contract on Juno uni-7               │
│  stores attestation immutably on-chain              │
└─────────────────────────────────────────────────────┘
```

The bridge daemon can run anywhere (your Windows machine, a Raspberry Pi, a VPS).
The WAVS operator stack is what needs the TEE hardware.

---

## Prerequisites (on the Azure VM)

These must all be installed on the Linux server:

| Tool | Purpose |
|------|---------|
| Rust + rustup | Build WASI component |
| cargo-component | Compile to wasm32-wasip2 |
| warg-cli + wkg | Publish component to wa.dev |
| Docker + Docker Compose | Run WAVS operator stack |
| Task (Taskfile) | Orchestration commands |
| pnpm + Node v21+ | JS dependencies |
| jq | JSON parsing in scripts |
| git | Clone repos |

---

## Full Step-by-Step

### Step 1 — Provision Azure VM (~10 min)

1. Go to [portal.azure.com](https://portal.azure.com) → Create Resource → Virtual Machine
2. Settings:
   - **Region**: East US 2
   - **Image**: Ubuntu 22.04 LTS
   - **Size**: `Standard_DC2s_v3` (2 vCPU / 8GB / Intel SGX) — ~$0.23/hr
   - **Auth**: SSH public key
   - **Inbound ports**: 22 (SSH)
3. Download the SSH key, then connect:

```bash
chmod 400 junoclaw-tee-key.pem
ssh -i junoclaw-tee-key.pem azureuser@<VM_PUBLIC_IP>
```

---

### Step 2 — Install All Dependencies (~15 min)

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Docker
sudo apt install -y docker.io docker-compose-v2 git curl jq
sudo usermod -aG docker $USER && newgrp docker

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
rustup toolchain install stable
rustup target add wasm32-wasip2

# cargo-component + warg/wkg tools (for wa.dev registry)
cargo install cargo-binstall
cargo binstall cargo-component wasm-tools warg-cli wkg --locked --no-confirm --force

# Configure wa.dev as default registry
wkg config --default-registry wa.dev

# Register a new key for publishing (one-time)
# On Linux (NOT WSL), standard keychain works:
warg key new

# Task (Taskfile runner)
npm install -g @go-task/cli

# Node v21+
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
source ~/.bashrc
nvm install --lts

# pnpm
npm install -g pnpm

# jq already installed above
```

---

### Step 3 — Clone the WAVS Foundry Template + Your Repo (~5 min)

```bash
# WAVS operator stack lives in this template
git clone https://github.com/Lay3rLabs/wavs-foundry-template.git
cd wavs-foundry-template

# Also clone JunoClaw to get the WASI component + service.json
git clone https://github.com/Dragonmonk111/junoclaw.git
```

---

### Step 4 — Build the JunoClaw WASI Component (~10 min)

```bash
cd ~/junoclaw/wavs

# Build for wasm32-wasip2 (WAVS target)
cargo component build --release

# Output:
# target/wasm32-wasip2/release/junoclaw_wavs_component.wasm  (~352KB)
```

---

### Step 5 — Publish Component to wa.dev (~5 min)

```bash
# Publish to the public registry
# Format: wkg publish <path> --name <namespace>:<name> --version <semver>
wkg publish \
  target/wasm32-wasip2/release/junoclaw_wavs_component.wasm \
  --name junoclaw:verifier \
  --version 0.1.0

# This gives you a content digest — update service.json if needed
# But our service.json already uses Registry source, which resolves by name
```

---

### Step 6 — Configure .env for WAVS Operator (~5 min)

In `~/wavs-foundry-template/.env` (copy from `.env.example`):

```env
# Your Neo wallet mnemonic (the operator wallet)
WAVS_CLI_COSMOS_MNEMONIC="word1 word2 word3 ... word24"

# Juno testnet config — WAVS picks this up for Cosmos chain monitoring
# (These map to what's in wavs.toml)
```

In `~/junoclaw/wavs/.env` (copy from `.env.example`):

```env
MNEMONIC="word1 word2 word3 ... word24"
RPC_ENDPOINT=https://juno-testnet-rpc.polkachu.com
CHAIN_ID=uni-7
AGENT_COMPANY_CONTRACT=juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6
TASK_LEDGER_CONTRACT=juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46
GAS_PRICE=0.075ujunox
WAVS_AGGREGATOR_URL=http://localhost:8080
```

---

### Step 7 — Start the WAVS Operator Stack (~5 min)

```bash
cd ~/wavs-foundry-template

# This starts the full Docker Compose stack:
# - WAVS operator node (SGX auto-detected on DCsv3)
# - WAVS aggregator (listens on :8080)
# - Supporting monitoring services
task start

# Or with Docker Compose directly:
docker compose up -d

# Watch the logs:
docker compose logs -f wavs-operator
```

When you see `SGX enclave initialized` in the logs — the TEE is active.

---

### Step 8 — Deploy the JunoClaw Service to WAVS (~5 min)

```bash
cd ~/junoclaw/wavs

# Register the service with the WAVS operator
# This tells WAVS to start watching for wasm-outcome_create events
# from the agent-company contract on Juno uni-7
wavs-cli service deploy \
  --manifest service.json \
  --config wavs.toml

# The operator will now:
# 1. Watch Juno for wasm-outcome_create events
# 2. Run the WASI component inside the SGX enclave
# 3. Hardware-sign the attestation hash
# 4. Expose the signed result at aggregator :8080
```

---

### Step 9 — Start Bridge Daemon (from Windows or anywhere) (~1 min)

Back on your Windows machine:

```bash
cd junoclaw/wavs/bridge
npx tsx src/bridge.ts
```

The bridge daemon polls `http://<VM_IP>:8080` (update `WAVS_AGGREGATOR_URL` in your local `.env`
to point to the VM's public IP), picks up signed results, and submits them to the
agent-company contract on Juno.

---

### Step 10 — Test It End-to-End (~5 min)

```bash
# On Windows — create a new proposal
npx tsx src/test-proposal.ts

# Watch the WAVS logs on the VM:
docker compose logs -f wavs-operator
# You'll see: "Processing wasm-outcome_create event for proposal 4"
# And: "SGX signed result: 0x..."

# Query the attestation (back on Windows):
npx tsx src/query-attestation.ts --list
# Proposal 4 will appear with a HARDWARE-SIGNED attestation hash
```

---

## What Changes in the Attestation Hash

| Stage | Hash source |
|-------|------------|
| Proposal 2 (manual) | `wavs_tee_attestation_hash_7065a358` — test string |
| Proposal 3 (local operator) | Real SHA-256, but software-signed |
| Proposal 4 (TEE operator) | Real SHA-256, **hardware-signed by SGX enclave** |

The data_hash will be identical to proposal 3 (same inputs, same SHA-256).
The attestation_hash will be different — it's signed by the enclave's hardware key,
which cannot be forged even by the server operator.

---

## Cost Summary

| Item | Cost |
|------|------|
| Azure DCsv3 (2 vCPU, SGX) | ~$0.23/hr |
| 4-hour proof session | ~$1.00 |
| Azure free credit available | $200 |
| **Net cost** | **$0 (within free credit)** |

Delete the VM after the milestone. Total time from zero to TEE attestation on-chain: ~1 hour.

---

## What to do After the Proof

1. **Screenshot/record** the `SGX enclave initialized` log + the attestation TX
2. **Query on-chain** — show the 3 attestations side by side (manual / software / hardware)
3. **Write the article**: "The loop is fully closed."
4. **Migrate to Hetzner AX52** (~€59/mo) if you want a persistent 24/7 operator
5. **Akash migration** = Akash milestone done simultaneously

---

## Known Unknowns (Verify Before Running)

- `wavs-cli service deploy` — exact command name; check `wavs-cli --help` after install
- Docker Compose file location in wavs-foundry-template — may be `.docker/compose.yml`
- The `task start` command — check `Taskfile.yml` in the template repo for exact targets
- Port 8080 may need to be opened in Azure VM's Network Security Group for remote bridge

When in doubt: `wavs-cli --help` and `task --list` are your friends.
