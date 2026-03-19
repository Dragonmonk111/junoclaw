#!/bin/bash
set -e

# === PATH ===
export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"
export TASK_X_REMOTE_TASKFILES=1

echo "========================================="
echo "  WAVS Full Deployment — Final Attempt"
echo "========================================="

# === FIX 1: warg keyring backend ===
echo "[1/5] Fixing warg keyring for headless server..."
warg config --keyring-backend flat-file --overwrite
warg key new
echo "  -> warg key created"

# === FIX 2: Restart Anvil if dead ===
echo "[2/5] Checking Anvil..."
if ! curl -s http://127.0.0.1:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
  echo "  -> Restarting Anvil..."
  pkill anvil 2>/dev/null || true
  nohup anvil --host 0.0.0.0 > /tmp/anvil.log 2>&1 &
  sleep 3
fi
echo "  -> Anvil OK"

# === FIX 3: Restart Docker Compose if needed ===
echo "[3/5] Checking Docker services..."
cd /home/azureuser/wavs-foundry-template
docker compose up -d 2>&1
sleep 2
echo "  -> Docker services OK"

# === CLEAN: Remove stale deployment artifacts ===
echo "[4/5] Cleaning stale deployment state..."
rm -rf .docker/deployment_summary.json .docker/service-cid .nodes 2>/dev/null || true
rm -rf infra/wavs-1 infra/aggregator-1 2>/dev/null || true

# === DEPLOY ===
echo "[5/5] Running pnpm deploy:full..."
echo ""
pnpm run deploy:full 2>&1

echo ""
echo "========================================="
echo "  DEPLOYMENT COMPLETE"
echo "========================================="
cat .docker/deployment_summary.json 2>/dev/null || echo "(no summary file)"

echo ""
echo "SGX status:"
ls -la /dev/sgx_enclave /dev/sgx_provision 2>/dev/null
