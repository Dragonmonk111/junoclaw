#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

echo "=== Account info ==="
junod query auth account "$ADMIN" --node "$NODE" --output json 2>/dev/null | jq '.account.sequence // .sequence'

echo "=== Instantiate code 2 ==="
junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode block --yes --output json 2>&1 | tee /tmp/inst2_result.txt

echo ""
echo "=== Result ==="
cat /tmp/inst2_result.txt | grep -o '{.*}' | jq '{code, txhash, raw_log, events: [.events[]? | select(.type == "instantiate") | .attributes[]? | select(.key == "_contract_address")]}'
