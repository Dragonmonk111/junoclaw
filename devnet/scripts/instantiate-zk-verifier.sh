#!/bin/sh
# Instantiate zk-verifier from a given code_id
# Usage: sh instantiate-zk-verifier.sh <code_id> <label>

CODE_ID="${1:-4}"
LABEL="${2:-zk-verifier-precompile-v2}"
ADMIN=$(junod keys show admin -a --keyring-backend test)
INIT="{\"admin\":\"$ADMIN\"}"

junod tx wasm instantiate "$CODE_ID" "$INIT" \
  --from admin --label "$LABEL" --no-admin \
  --chain-id junoclaw-bn254-1 --keyring-backend test \
  --gas auto --gas-adjustment 1.5 --gas-prices 0.075ujuno \
  --broadcast-mode sync --yes --output json --node http://localhost:26657
