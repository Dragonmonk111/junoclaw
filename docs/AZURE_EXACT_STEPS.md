# Azure DCsv3 — Exact Steps
## From zero to SSH'd in, with a TEE-capable Linux VM

---

## Part 1 — Create Azure Account (if new)

1. Go to: https://azure.microsoft.com/en-us/free
2. Click **"Start free"**
3. Sign in with **GitHub** (click "Sign-in options" → "Sign in with GitHub")
4. Enter phone number for identity verification (SMS code)
5. Enter card details — **$0 will be charged**, you get $200 credit
6. Skip the "survey" page, click **"Go to Azure portal"**

---

## Part 2 — Create the VM (Azure Portal UI)

### 2a — Navigate to VM creation

In the Azure portal search bar at top, type:
```
Virtual machines
```
Click **"Virtual machines"** → click **"+ Create"** → **"Azure virtual machine"**

---

### 2b — Basics tab

| Field | Exact value |
|-------|------------|
| Subscription | Azure subscription 1 (the free one) |
| Resource group | Click "Create new" → name it `junoclaw-tee` |
| Virtual machine name | `junoclaw-operator` |
| Region | **(US) East US 2** ← important, best DCsv3 availability |
| Availability options | No infrastructure redundancy required |
| Security type | **Standard** ← SGX comes from the DC2s_v3 hardware itself, not this setting |
| Image | Click "See all images" → search `Ubuntu 22.04` → pick **Ubuntu Server 22.04 LTS - x64 Gen2** |
| Size | Click "See all sizes" → search `DC2s_v3` → pick **Standard_DC2s_v3** (2 vcpus, 8 GiB) |
| Authentication type | SSH public key |
| Username | `azureuser` |
| SSH public key source | **Generate new key pair** |
| Key pair name | `junoclaw-operator-key` |

Under **Inbound port rules**:
- Public inbound ports: **Allow selected ports**
- Select inbound ports: **SSH (22)**

Click **"Next: Disks"** — leave defaults → **"Next: Networking"**

---

### 2c — Networking tab

Leave all defaults. Make note that a public IP will be created automatically.

Click **"Review + create"**

---

### 2d — Review + Create

Check that Size shows `Standard_DC2s_v3`. Click **"Create"**.

A popup appears: **"Generate new key pair"** → click **"Download private key and create resource"**

This downloads `junoclaw-operator-key.pem` to your Downloads folder. **Save this file — you cannot get it again.**

Deployment takes ~2–3 minutes. When done, click **"Go to resource"**.

---

### 2e — Get the Public IP

On the VM overview page, find **"Public IP address"** on the right side. It looks like:
```
40.121.xxx.xxx
```
Copy it.

---

### 2f — Open Port 8080 (for bridge daemon)

On the VM page, left sidebar → **"Networking"** → **"Add inbound port rule"**:

| Field | Value |
|-------|-------|
| Destination port ranges | 8080 |
| Protocol | TCP |
| Action | Allow |
| Name | allow-wavs-aggregator |

Click **"Add"**.

---

## Part 3 — SSH Into the VM (from Windows)

Open **PowerShell** (Windows key → type `powershell`):

```powershell
# Move the key file (adjust path if saved elsewhere)
Move-Item "$env:USERPROFILE\Downloads\junoclaw-operator-key.pem" "C:\Users\Taj\junoclaw-operator-key.pem"

# Set correct permissions (Windows equivalent of chmod 400)
icacls "C:\Users\Taj\junoclaw-operator-key.pem" /inheritance:r /grant:r "$env:USERNAME:R"

# SSH in (replace with your actual IP)
ssh -i "C:\Users\Taj\junoclaw-operator-key.pem" azureuser@40.121.xxx.xxx
```

You should see:
```
Welcome to Ubuntu 22.04 LTS
azureuser@junoclaw-operator:~$
```

**You are now on a TEE-capable Linux server. SGX hardware is present.**

---

## Part 4 — Install Dependencies (paste as one block)

```bash
# System update
sudo apt update && sudo apt upgrade -y

# Core tools
sudo apt install -y docker.io docker-compose-v2 git curl jq build-essential pkg-config libssl-dev

# Add user to docker group
sudo usermod -aG docker $USER && newgrp docker

# Node.js via nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
source ~/.bashrc
nvm install --lts

# pnpm + task runner
npm install -g pnpm @go-task/cli

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
rustup toolchain install stable
rustup target add wasm32-wasip2

# cargo-component + WASM tools + warg/wkg for wa.dev
cargo install cargo-binstall
cargo binstall cargo-component wasm-tools warg-cli wkg --locked --no-confirm --force

# Configure wa.dev as default registry
wkg config --default-registry wa.dev

# Generate a new publishing key for wa.dev (one-time)
warg key new
```

This takes ~10 minutes. Get a coffee.

---

## Part 5 — Clone Repos

```bash
# WAVS operator template (gives us the Docker Compose stack)
git clone https://github.com/Lay3rLabs/wavs-foundry-template.git

# JunoClaw (WASI component + service.json + bridge)
git clone https://github.com/Dragonmonk111/junoclaw.git
```

---

## Part 6 — Build the WASI Component

```bash
cd ~/junoclaw/wavs
cargo component build --release

# Expected output:
# Compiling junoclaw-wavs-component v0.1.0
# Finished release profile
# Component: target/wasm32-wasip2/release/junoclaw_wavs_component.wasm

ls -lh target/wasm32-wasip2/release/junoclaw_wavs_component.wasm
# Should show ~300-400KB
```

---

## Part 7 — Configure .env

```bash
cd ~/junoclaw/wavs
cp .env.example .env
nano .env
```

Fill in:
```env
MNEMONIC=word1 word2 word3 ... word24
RPC_ENDPOINT=https://juno-testnet-rpc.polkachu.com
CHAIN_ID=uni-7
AGENT_COMPANY_CONTRACT=juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6
TASK_LEDGER_CONTRACT=juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46
GAS_PRICE=0.075ujunox
WAVS_AGGREGATOR_URL=http://localhost:8080
```

Save: `Ctrl+X` → `Y` → Enter

---

## Part 8 — Start WAVS Operator Stack

```bash
cd ~/wavs-foundry-template

# Check available task commands
task --list

# Check Docker Compose file location
ls .docker/

# Start the full WAVS stack
# (exact command to confirm from task --list output)
task start
# OR:
docker compose -f .docker/compose.yml up -d

# Watch logs — look for "SGX enclave initialized"
docker compose -f .docker/compose.yml logs -f
```

---

## Part 9 — Verify SGX is Active

```bash
# Check SGX device is present
ls /dev/sgx*
# Should show: /dev/sgx_enclave  /dev/sgx_provision

# Or check WAVS logs for TEE confirmation
docker compose logs wavs-operator | grep -i "sgx\|tee\|enclave"
```

---

## Part 10 — Update Bridge Daemon on Windows

In `junoclaw/wavs/bridge/.env` on your Windows machine, update:
```env
WAVS_AGGREGATOR_URL=http://40.121.xxx.xxx:8080
```

Then run the bridge:
```bash
npx tsx src/bridge.ts
```

---

## Part 11 — Test End-to-End

```bash
# Windows: create proposal 4
npx tsx src/test-proposal.ts

# VM: watch WAVS process it
docker compose logs -f wavs-operator

# Windows: query result
npx tsx src/query-attestation.ts --list
# Proposal 4 = hardware-signed attestation hash
```

---

## ⚠️ Pay-as-you-go Billing Note

- Charges begin the **moment the VM is running** (~$0.23/hr)
- A 4-hour session = **~$0.92** total
- **Set a budget alert**: Azure portal → search "Cost Management" → Budgets → Create → set $10 limit with email alert
- **Delete the VM when done** — stopping it still charges for the disk (~$0.02/hr)

---

## Cleanup (After Milestone)

**To stop billing immediately:**

1. Azure portal → Virtual machines → `junoclaw-operator`
2. Click **"Delete"**
3. Check "Delete OS disk" and "Delete public IP"
4. Confirm deletion

Billing stops within minutes. Total cost for a 4-hour session: ~$1.

---

## Troubleshooting

**`Standard_DC2s_v3` not available in East US 2?**
Try: East US, West US 2, or North Europe — DCsv3 is available in all major regions.

**SSH permission denied?**
```powershell
icacls "C:\Users\Taj\junoclaw-operator-key.pem" /inheritance:r
icacls "C:\Users\Taj\junoclaw-operator-key.pem" /grant:r "$env:USERNAME:(R)"
```

**Docker permission denied?**
```bash
newgrp docker
# or log out and back in
```

**`wasm32-wasip2` target missing?**
```bash
rustup target add wasm32-wasip2
```
