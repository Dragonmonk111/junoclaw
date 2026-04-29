#!/bin/bash
set -e

cd /tmp

# Extract from image
id=$(docker create junoclaw/junod-bn254:devnet 2>&1 | head -1)
docker cp "$id:/usr/local/bin/junod" ./junod
docker cp "$id:/lib/libwasmvm.x86_64.so" ./libwasmvm.x86_64.so 2>/dev/null || \
  docker cp "$id:/usr/lib/libwasmvm.x86_64.so" ./libwasmvm.x86_64.so 2>/dev/null || \
  docker cp "$id:/usr/local/lib/libwasmvm.x86_64.so" ./libwasmvm.x86_64.so 2>/dev/null
docker rm "$id"

# Make executable
chmod +x junod

# Test
echo "Testing junod..."
LD_LIBRARY_PATH=/tmp ./junod version 2>&1 | head -1

# Create data dir
mkdir -p /tmp/juno-home

# Init
echo "Initializing chain..."
LD_LIBRARY_PATH=/tmp ./junod init test --chain-id junoclaw-bn254-1 --home /tmp/juno-home 2>&1 | head -2

# Add key
echo "Adding validator key..."
LD_LIBRARY_PATH=/tmp ./junod keys add validator --keyring-backend test --home /tmp/juno-home 2>&1 | head -1

ADDR=$(LD_LIBRARY_PATH=/tmp ./junod keys show validator -a --keyring-backend test --home /tmp/juno-home 2>&1)
echo "Validator: $ADDR"

# Fund
echo "Funding genesis..."
LD_LIBRARY_PATH=/tmp ./junod genesis add-genesis-account "$ADDR" 1000000000ujuno --home /tmp/juno-home 2>&1 | head -1

# Gentx
LD_LIBRARY_PATH=/tmp ./junod genesis gentx validator 500000000ujuno --chain-id junoclaw-bn254-1 --keyring-backend test --home /tmp/juno-home 2>&1 | head -1

# Collect
LD_LIBRARY_PATH=/tmp ./junod genesis collect-gentxs --home /tmp/juno-home 2>&1 | head -2

# Modify genesis for block gas
jq '.consensus_params.block.max_gas = "80000000"' /tmp/juno-home/config/genesis.json > /tmp/genesis.tmp && mv /tmp/genesis.tmp /tmp/juno-home/config/genesis.json

# Start junod in background
echo "Starting junod..."
LD_LIBRARY_PATH=/tmp ./junod start --home /tmp/juno-home --rpc.laddr tcp://0.0.0.0:26657 --wasm.skip_wasmvm_version_check 2>&1 &
JUNOD_PID=$!
sleep 8

# Check if running
if curl -s http://localhost:26657/status >/dev/null 2>&1; then
  echo "JUNOD RUNNING!"
  curl -s http://localhost:26657/status 2>&1 | grep -o '"latest_block_height":"[0-9]*"' | head -1
else
  echo "JUNOD NOT RESPONDING"
  kill $JUNOD_PID 2>/dev/null || true
fi
