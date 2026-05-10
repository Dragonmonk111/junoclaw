#!/bin/bash
set -e
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"
CHAIN_ID="junoclaw-bn254-1"
NODE="http://localhost:26657"

echo "=== Instantiating code_id 2 (precompile) ==="
RESULT=$(junod tx wasm instantiate 2 \
  "{\"admin\":\"${ADMIN}\"}" \
  --from validator \
  --chain-id "$CHAIN_ID" \
  --keyring-backend test \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.5 --gas-prices 0.1ujuno \
  --label "zk-verifier-precompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1)
echo "$RESULT"

TXHASH=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.txhash // empty')
echo "txhash: $TXHASH"

if [ -n "$TXHASH" ]; then
  echo "Waiting for inclusion..."
  sleep 4
  junod query tx "$TXHASH" --node "$NODE" --output json 2>/dev/null | jq '{code, events: [.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address")]}'
fi
