#!/bin/bash
set -x
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"

# Simple bank send to test signing
ADMIN=$(junod keys show validator -a --keyring-backend test)
OTHER=$(junod keys show admin -a --keyring-backend test)
echo "validator=$ADMIN admin=$OTHER"

# Check sequence
junod query auth account "$ADMIN" --node "$NODE" 2>&1 | grep -E 'account_number|sequence'

# Simple send tx
echo "=== Bank send ==="
junod tx bank send validator "$OTHER" 1ujuno \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 200000 --gas-prices 0.1ujuno \
  --broadcast-mode sync --yes --output json 2>&1

echo ""
echo "=== Try instantiate with --gas auto (needs gRPC for simulation) ==="
junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.5 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1
