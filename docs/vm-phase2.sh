#!/bin/bash
set -e

# Fix PATH for all tools
export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

echo "=== Verifying tools ==="
echo "forge: $(forge --version 2>&1 | head -1)"
echo "cargo-component: $(cargo-component --version)"
echo "node: $(node --version)"
echo "docker: $(docker --version)"

echo "=== Setting up foundry template ==="
cd /home/azureuser/wavs-foundry-template

# Initialize git submodules for forge libraries
git submodule update --init --recursive 2>/dev/null || true
forge install 2>/dev/null || echo "forge install skipped (may need manual setup)"

# Copy our WASI component into compiled/
mkdir -p compiled
cp /home/azureuser/junoclaw/wavs/target/wasm32-wasip1/release/junoclaw_wavs_component.wasm compiled/
echo "Component copied: $(ls -lh compiled/junoclaw_wavs_component.wasm)"

# Get component SHA256 digest
DIGEST=$(sha256sum compiled/junoclaw_wavs_component.wasm | cut -d' ' -f1)
echo "Component digest: $DIGEST"

echo "=== Starting Docker Compose services (IPFS + warg) ==="
cd /home/azureuser/wavs-foundry-template
docker compose up -d 2>&1
sleep 5

echo "=== Checking services ==="
docker compose ps 2>&1

echo "=== Starting Anvil (local EVM chain for WAVS management) ==="
# Start anvil in background
nohup anvil --host 0.0.0.0 > /tmp/anvil.log 2>&1 &
sleep 3
echo "Anvil PID: $(pgrep anvil || echo 'not running')"

echo "=== Checking WAVS endpoint ==="
# The WAVS operator needs to be started via Docker
# Check what task commands are available
echo "Available task commands:"
task --list 2>&1 | head -30 || echo "task --list failed"

echo "=== SGX Status ==="
ls -la /dev/sgx* 2>/dev/null || echo "No SGX devices"

echo "=== Phase 2 Complete ==="
echo "Component digest: $DIGEST"
echo "Next: deploy WAVS operator + upload component + deploy service"
