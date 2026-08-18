#!/usr/bin/env bash
# run-bn254-localnet.sh
#
# Initializes and runs a local Juno testnet with the patched BN254 junod.
# Then stores the ZK verifier wasm and runs a VerifyProof gas benchmark.
set -euo pipefail

export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
MONIKER="bn254-validator"

echo "=== Setting up local Juno testnet with BN254 patched junod ==="

# Clean any previous state
rm -rf "${HOME_DIR}"
mkdir -p "${HOME_DIR}"

# Initialize the chain
echo "[1/6] Initializing chain..."
${JUNOD} init ${MONIKER} --chain-id ${CHAIN_ID} --home ${HOME_DIR} 2>&1 | tail -3

# Create a validator key
echo "[2/6] Creating validator key..."
${JUNOD} keys add validator --keyring-backend test --home ${HOME_DIR} 2>&1 | tail -5

# Add genesis account
VALIDATOR_ADDR=$(${JUNOD} keys show validator -a --keyring-backend test --home ${HOME_DIR})
echo "[3/6] Adding genesis account: ${VALIDATOR_ADDR}"
${JUNOD} genesis add-genesis-account ${VALIDATOR_ADDR} 1000000000000ujuno --home ${HOME_DIR}

# Create genesis transaction
echo "[4/6] Creating genesis tx..."
${JUNOD} genesis gentx validator 1000000000ujuno --chain-id ${CHAIN_ID} --keyring-backend test --home ${HOME_DIR} 2>&1 | tail -3

# Collect genesis txs
echo "[5/6] Collecting genesis txs..."
${JUNOD} genesis collect-gentxs --home ${HOME_DIR} 2>&1 | tail -3

# Start the chain in background
echo "[6/6] Starting chain..."
${JUNOD} start --home ${HOME_DIR} --log_level error &
JUNOD_PID=$!
echo "  junod PID: ${JUNOD_PID}"

# Wait for chain to produce first block
echo "  Waiting for first block..."
for i in $(seq 1 30); do
  if curl -s http://localhost:26657/status 2>/dev/null | grep -q "latest_block_height"; then
    HEIGHT=$(curl -s http://localhost:26657/status | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])" 2>/dev/null || echo "0")
    if [[ "${HEIGHT}" != "0" ]]; then
      echo "  Chain at height ${HEIGHT}"
      break
    fi
  fi
  sleep 2
done

# Check wasm params
echo ""
echo "=== Wasm status ==="
${JUNOD} query wasm params --node http://localhost:26657 2>&1

echo ""
echo "=== wasmvm version ==="
${JUNOD} query wasm libwasmvm-version --node http://localhost:26657 2>&1

echo ""
echo "Localnet is running. PID: ${JUNOD_PID}"
echo "To stop: kill ${JUNOD_PID}"
echo ""
echo "Next: store ZK verifier wasm and run VerifyProof benchmark"
