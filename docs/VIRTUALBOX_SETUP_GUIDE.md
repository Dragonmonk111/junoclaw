# VirtualBox Setup Guide — Ubuntu 24.04 VM for JunoClaw Soak Test

This guide walks through setting up an Ubuntu 24.04 VM in VirtualBox on Windows,
installing the Rust toolchain (with NASM for commonware-p2p), building the
JunoClaw coordination stack, and running the 7-day soak test.

## Prerequisites

- **Host OS:** Windows 10/11 with VirtualBox 7.x installed
- **RAM:** 16 GB allocated to VM (host has 64 GB total)
- **Disk:** 40 GB dynamically allocated VDI on a drive with 80+ GB free
- **Network:** Bridged adapter or NAT with port forwarding

## Step 1: Create the VM

1. Open VirtualBox, click **New**
2. **Name:** `junoclaw-soak`
3. **Type:** Linux
4. **Version:** Ubuntu (24.04, 64-bit)
5. **Memory:** 16384 MB (16 GB)
6. **CPU:** 4 cores (Settings → System → Processor)
7. **Disk:** 40 GB dynamically allocated VDI
8. **Network:** Settings → Network → Attached to: **Bridged Adapter**
   - This gives the VM its own IP on your LAN, useful for SSH and P2P ports

## Step 2: Install Ubuntu 24.04 LTS

1. Download Ubuntu 24.04 LTS Server ISO: https://ubuntu.com/download/server
2. Mount the ISO: Settings → Storage → Optical Drive → Choose ISO
3. Start the VM, follow the installer:
   - **Minimal install** (no GUI — server is sufficient)
   - **Create user:** `juno` / password of your choice
   - **Install OpenSSH server:** YES
   - **Skip snaps:** Not needed
4. After install, reboot and SSH in from Windows:
   ```powershell
   ssh juno@<VM-IP>
   ```
   (Find VM IP with `ip addr` on the VM console, or check your router's DHCP table)

## Step 3: Install Build Dependencies

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential curl git nasm pkg-config libssl-dev \
    nodejs npm cmake clang llvm
```

**NASM is critical** — `commonware-p2p` depends on `aws-lc-sys` which requires
NASM to compile the TLS library on x86_64. Without NASM, `cargo build --features p2p`
will fail.

## Step 4: Install Rust Toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustc --version
```

## Step 5: Clone the JunoClaw Repository

```bash
cd ~
git clone <your-repo-url> junoclaw
cd junoclaw
```

Or if you have the repo on the Windows host, use a shared folder:

1. In VirtualBox: Settings → Shared Folders → Add
   - **Folder Path:** `C:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw`
   - **Mount Point:** `/mnt/junoclaw`
   - **Auto-mount:** Yes
   - **Make Permanent:** Yes

2. On the VM:
   ```bash
   sudo mount -t vboxsf junoclaw /mnt/junoclaw
   cp -r /mnt/junoclaw ~/junoclaw
   cd ~/junoclaw
   ```

## Step 6: Build the Coordination Stack

```bash
# Build with P2P feature enabled (requires NASM)
cargo build --release --features p2p -p junoclaw-coordination

# Build test-mesh binaries (consensus-test, gate-test)
cargo build --release -p junoclaw-test-mesh

# Verify binaries
./target/release/consensus-test
./target/release/gate-test
```

Expected output from `consensus-test`:
```
=== Phase 2: Consensus Integration Test ===
4 validators (3 honest, 1 byzantine), 300ms block time target
...
=== Phase 2 Consensus Test: PASS ===
```

## Step 7: Configure the Relayer

The soak-test script relays batches to uni-7 testnet every hour. You need:

1. **Wallet mnemonic** for a funded uni-7 account:
   ```bash
   export JUNO_MNEMONIC="word1 word2 ... word12"
   ```

2. **Deployed contract address** in `deploy/deployed-testnet.json`:
   ```json
   {
     "contract": "juno1...",
     "code_id": 123
   }
   ```

3. **Testnet RPC** (default: `https://juno.rpc.t.stavr.tech`):
   ```bash
   export RPC_URL="https://juno.rpc.t.stavr.tech"
   export CHAIN_ID="uni-7"
   ```

## Step 8: Run the Soak Test

```bash
# Make the script executable
chmod +x scripts/soak-test.sh

# Run for 7 days (default), with 5-minute cycles
./scripts/soak-test.sh

# Or customize:
SOAK_DAYS=7 SOAK_INTERVAL=300 ./scripts/soak-test.sh
```

### Running in Background with `tmux`

```bash
sudo apt install -y tmux
tmux new -s soak
./scripts/soak-test.sh
# Press Ctrl+B then D to detach
# Reattach: tmux attach -t soak
```

### Running as a systemd Service (Recommended)

```bash
sudo tee /etc/systemd/system/junoclaw-soak.service << 'EOF'
[Unit]
Description=JunoClaw 7-Day Soak Test
After=network.target

[Service]
Type=simple
User=juno
WorkingDirectory=/home/juno/junoclaw
Environment=JUNO_MNEMONIC=your_mnemonic_here
Environment=CHAIN_ID=uni-7
Environment=RPC_URL=https://juno.rpc.t.stavr.tech
ExecStart=/home/juno/junoclaw/scripts/soak-test.sh
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable junoclaw-soak
sudo systemctl start junoclaw-soak

# Check status
sudo systemctl status junoclaw-soak
journalctl -u junoclaw-soak -f
```

## Step 9: Monitor the Soak Test

### From the VM

```bash
# Watch live logs
tail -f ~/junoclaw/soak-logs/soak-main.log

# Check status
cat ~/junoclaw/soak-logs/soak-status.json

# Check per-cycle consensus results
ls ~/junoclaw/soak-logs/consensus-cycle-*.log | tail -5
```

### From Windows Host via SSH

```powershell
ssh juno@<VM-IP> "tail -f ~/junoclaw/soak-logs/soak-main.log"
```

### Health Checks

The soak-test script writes `soak-status.json` every cycle with:
- Current cycle number
- Elapsed/remaining seconds
- Last certificate size and throughput

A simple health check script:
```bash
#!/bin/bash
STATUS_FILE="$HOME/junoclaw/soak-logs/soak-status.json"
if [ ! -f "$STATUS_FILE" ]; then
    echo "CRITICAL: status file not found"
    exit 2
fi
REMAINING=$(grep -oP 'remaining_seconds": \K[0-9]+' "$STATUS_FILE")
if [ "$REMAINING" -lt 0 ]; then
    echo "OK: soak test complete"
    exit 0
fi
echo "OK: $REMAINING seconds remaining"
exit 0
```

## Step 10: After the Soak Test

1. **Generate the final report:**
   ```bash
   cat ~/junoclaw/soak-logs/SOAK_REPORT.md
   ```

2. **Collect logs for the DAO proposal:**
   ```bash
   cd ~/junoclaw
   tar czf soak-test-results.tar.gz soak-logs/
   ```

3. **Shut down the VM:**
   ```bash
   sudo shutdown -h now
   ```

## Troubleshooting

### NASM not found
```
error: failed to run custom build command for `aws-lc-sys`
```
**Fix:** `sudo apt install nasm`

### Out of disk space
```bash
df -h
# Clean cargo cache
cargo cache --autoclean
# Or remove target dirs
rm -rf target/*/debug
```

### P2P nodes can't connect
- Check firewall: `sudo ufw allow 4001:4004/tcp`
- Check if ports are listening: `ss -tlnp | grep 400`
- For bridged networking, ensure the VM IP is reachable

### Relayer fails with "insufficient funds"
- Check wallet balance: `junod query bank balances <address> --node $RPC_URL`
- Faucet: https://testnet.juno.bh.rocks (or appropriate uni-7 faucet)

### consensus-test crashes
- Check `soak-logs/consensus-cycle-N.log` for the specific error
- Run manually: `RUST_LOG=debug ./target/release/consensus-test`

## Resource Requirements Summary

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| RAM | 8 GB | 16 GB |
| Disk | 20 GB | 40 GB |
| CPU | 2 cores | 4 cores |
| Network | NAT | Bridged |
| NASM | Required | Required |
| Rust | 1.75+ | Latest stable |
| Node.js | 18+ | 20+ |
