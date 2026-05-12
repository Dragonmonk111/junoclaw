#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

# Get sequence from text output (more reliable than JSON parsing)
ACCT_INFO=$(junod query auth account "$ADMIN" --node "$NODE" 2>/dev/null)
echo "Raw account info:"
echo "$ACCT_INFO"
echo ""

ACCT_NUM=$(echo "$ACCT_INFO" | grep 'account_number' | grep -o '[0-9]*' | head -1)
SEQ=$(echo "$ACCT_INFO" | grep 'sequence' | grep -o '[0-9]*' | head -1)
echo "Parsed: account_number=$ACCT_NUM sequence=$SEQ"

if [ -z "$ACCT_NUM" ] || [ -z "$SEQ" ]; then
  echo "ERROR: could not parse account info"
  exit 1
fi

echo ""
echo "=== Instantiate code 2 ==="
RESULT=$(junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --account-number "$ACCT_NUM" --sequence "$SEQ" \
  --broadcast-mode sync --yes --output json 2>&1)
echo "$RESULT"

TXHASH=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.txhash // empty')
CODE=$(echo "$RESULT" | grep -o '{.*}' | jq -r '.code // 0')
echo "txhash=$TXHASH code=$CODE"

if [ "$CODE" != "0" ]; then
  echo "raw_log: $(echo "$RESULT" | grep -o '{.*}' | jq -r '.raw_log')"
  exit 1
fi

echo "Waiting 5s..."
sleep 5
echo "=== Contracts for code 2 ==="
junod query wasm list-contract-by-code 2 --node "$NODE" --output json 2>/dev/null | jq '.contracts'
