#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

cd /home/azureuser/wavs-foundry-template

echo "=== Trusting remote taskfiles ==="
# Auto-trust the remote taskfile includes
yes | task --list 2>&1 || true
echo ""

echo "=== Available tasks ==="
task --list 2>&1 || echo "task --list still failing"

echo "=== Deploying WAVS core contracts on local Anvil ==="
# Check if anvil is running
curl -s -X POST http://127.0.0.1:8545 -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>&1 || echo "Anvil not reachable"

echo "=== Starting WAVS operator via Docker ==="
# Start the WAVS operator Docker container
# It needs:
#   --network host (to access Anvil, IPFS, etc.)
#   --device /dev/sgx_enclave (for Intel SGX)
#   --device /dev/sgx_provision (for SGX provisioning)
#   -v for mounting config and data

docker run -d --name wavs-operator \
  --network host \
  --device /dev/sgx_enclave:/dev/sgx_enclave \
  --device /dev/sgx_provision:/dev/sgx_provision \
  --env-file .env \
  -v $(pwd):/data \
  ghcr.io/lay3rlabs/wavs:1.5.1 \
  wavs-cli service manager start \
  --home /data \
  --data /data/.docker 2>&1 || echo "wavs-operator start failed, checking help..."

# If that command is wrong, try just running the image
docker logs wavs-operator 2>&1 || true

echo "=== Checking what wavs-cli service manager can do ==="
docker run --rm ghcr.io/lay3rlabs/wavs:1.5.1 wavs-cli service manager --help 2>&1

echo "=== Phase 3 info gathered ==="
