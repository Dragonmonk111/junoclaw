#!/bin/bash
# Run IMMEDIATELY after container restart, before any simulation.
# This script instantiates code 2 with explicit gas to avoid triggering
# the simulation panic that corrupts query state.
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"

echo "Waiting for node..."
for i in $(seq 1 30); do
  HEIGHT=$(curl -sf "$NODE/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])" 2>/dev/null || echo "0")
  if [ "$HEIGHT" -ge 1 ] 2>/dev/null; then
    echo "Node ready at block $HEIGHT"
    break
  fi
  sleep 1
done

ADMIN=$(junod keys show validator -a --keyring-backend test)
echo "validator=$ADMIN"

# Account state
junod query auth account "$ADMIN" --node "$NODE" 2>/dev/null | grep -E 'account_number|sequence'

echo ""
echo "=== Instantiate code 2 with EXPLICIT gas + fees (no simulation) ==="
RESULT=$(junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 2000000 --fees 200000ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1)
echo "$RESULT"

TXHASH=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.txhash // empty')
CODE=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.code // 0')
echo "txhash=$TXHASH code=$CODE"

if [ "$CODE" != "0" ]; then
  echo "FAILED: $(echo "$RESULT" | grep -o '{.*}' | jq -r '.raw_log')"
  echo ""
  echo "=== Try bank send to verify signing works ==="
  OTHER=$(junod keys show admin -a --keyring-backend test)
  junod tx bank send validator "$OTHER" 1ujuno \
    --chain-id "$CHAIN" \
    --keyring-backend test \
    --node "$NODE" \
    --gas 200000 --fees 20000ujuno \
    --broadcast-mode sync --yes --output json 2>&1
  exit 1
fi

echo "Waiting for inclusion..."
sleep 5
echo "=== Contracts for code 2 ==="
junod query wasm list-contract-by-code 2 --node "$NODE" --output json 2>/dev/null | jq '.contracts'
