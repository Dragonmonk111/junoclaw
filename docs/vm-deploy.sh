#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

# Trust remote taskfiles (required for WAVS foundry template)
export TASK_X_REMOTE_TASKFILES=1

cd /home/azureuser/wavs-foundry-template

echo "=== Step 1: Verify all services are running ==="

# Check Anvil
ANVIL_CHECK=$(curl -s -X POST http://127.0.0.1:8545 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>&1 || echo "FAIL")
echo "Anvil: $ANVIL_CHECK"

# Check Docker services
docker compose ps 2>&1

# Check IPFS
IPFS_CHECK=$(curl -s http://127.0.0.1:5001/api/v0/id 2>&1 | head -1 || echo "FAIL")
echo "IPFS: $IPFS_CHECK"

echo ""
echo "=== Step 2: List available tasks ==="
task --list 2>&1 | head -40

echo ""
echo "=== Step 3: Copy our WASI component ==="
mkdir -p compiled
cp /home/azureuser/junoclaw/wavs/target/wasm32-wasip1/release/junoclaw_wavs_component.wasm compiled/
ls -lh compiled/

echo ""
echo "=== Step 4: Run full deployment pipeline ==="
echo "Running: pnpm run deploy:full"
pnpm run deploy:full 2>&1

echo ""
echo "=== Deployment Complete ==="
echo "Check .docker/deployment_summary.json for details"
cat .docker/deployment_summary.json 2>/dev/null || echo "No deployment summary yet"
