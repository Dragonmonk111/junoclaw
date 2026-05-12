#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

# Get account number and sequence from auth module
ACCT_NUM=$(junod query auth account "$ADMIN" --node "$NODE" --output json 2>/dev/null | jq -r '.account.account_number // .account_number // "0"')
SEQ=$(junod query auth account "$ADMIN" --node "$NODE" --output json 2>/dev/null | jq -r '.account.sequence // .sequence // "0"')
echo "account_number=$ACCT_NUM sequence=$SEQ"

echo "=== Instantiate code 2 with correct seq ==="
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
  echo "FAILED: $(echo "$RESULT" | grep -o '{.*}' | jq -r '.raw_log')"
  exit 1
fi

echo "Waiting for inclusion..."
sleep 5

echo "=== Contracts for code 2 ==="
junod query wasm list-contract-by-code 2 --node "$NODE" --output json 2>/dev/null | jq '.contracts'
