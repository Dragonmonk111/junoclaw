#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

echo "=== Account sequence ==="
SEQ=$(junod query account "$ADMIN" --node "$NODE" --output json 2>/dev/null | jq -r '.account.sequence // .sequence // "unknown"')
echo "sequence: $SEQ"

echo "=== Instantiate code 2 ==="
RESULT=$(junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1)
echo "$RESULT"

TXHASH=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.txhash // empty')
CODE=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.code // 0')
echo "txhash=$TXHASH code=$CODE"

if [ "$CODE" != "0" ] && [ -n "$CODE" ]; then
  echo "TX rejected: $(echo "$RESULT" | grep -o '{.*}' | jq -r '.raw_log')"
  exit 1
fi

if [ -n "$TXHASH" ]; then
  echo "Waiting 6s for inclusion..."
  sleep 6
  echo "=== Query tx ==="
  junod query tx "$TXHASH" --node "$NODE" --output json 2>/dev/null | jq '{code, raw_log, contracts: [.events[]? | select(.type == "instantiate") | .attributes[]? | select(.key == "_contract_address") | .value]}'
fi
