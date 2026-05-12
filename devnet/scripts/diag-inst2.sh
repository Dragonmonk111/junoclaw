#!/bin/bash
set -x
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN=$(junod keys show validator -a --keyring-backend test)
echo "ADMIN=$ADMIN"

# Check account state
junod query auth account "$ADMIN" --node "$NODE" 2>&1 | grep -E 'account_number|sequence'

# Try instantiate
junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1
echo "EXIT=$?"
