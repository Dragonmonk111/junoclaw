#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"
export TASK_X_REMOTE_TASKFILES=1

cd /home/azureuser/wavs-foundry-template

# Ensure Anvil is running
if ! curl -s http://127.0.0.1:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
  echo "Restarting Anvil..."
  pkill anvil 2>/dev/null || true
  nohup anvil --host 0.0.0.0 > /tmp/anvil.log 2>&1 &
  sleep 3
fi

# Ensure Docker services running
docker compose up -d 2>&1

# Clean stale state
rm -rf .docker/deployment_summary.json .docker/service-cid .nodes 2>/dev/null || true
rm -rf infra/wavs-1 infra/aggregator-1 2>/dev/null || true

echo "Running pnpm deploy:full..."
pnpm run deploy:full 2>&1

echo ""
echo "=== RESULT ==="
cat .docker/deployment_summary.json 2>/dev/null || echo "No summary"
ls -la /dev/sgx_enclave /dev/sgx_provision 2>/dev/null
