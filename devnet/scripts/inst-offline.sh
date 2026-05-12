#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

# Get fresh sequence from chain state
ACCT_INFO=$(junod query auth account "$ADMIN" --node "$NODE" 2>/dev/null)
ACCT_NUM=$(echo "$ACCT_INFO" | grep 'account_number' | grep -o '[0-9]*' | head -1)
SEQ=$(echo "$ACCT_INFO" | grep 'sequence' | grep -o '[0-9]*' | head -1)
echo "account_number=$ACCT_NUM sequence=$SEQ"

# Use --offline to force the CLI to use our values instead of querying
echo "=== Instantiate code 2 (offline signing) ==="
RESULT=$(junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --fees 80000ujuno \
  --label "zkprecompile" \
  --no-admin \
  --offline \
  --account-number "$ACCT_NUM" --sequence "$SEQ" \
  --generate-only 2>&1)
echo "Generated tx (first 200 chars): ${RESULT:0:200}"

# Sign offline
echo "$RESULT" > /tmp/unsigned_tx.json
SIGNED=$(junod tx sign /tmp/unsigned_tx.json \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --account-number "$ACCT_NUM" --sequence "$SEQ" \
  --offline 2>&1)
echo "$SIGNED" > /tmp/signed_tx.json
echo "Signed tx (first 200 chars): ${SIGNED:0:200}"

# Broadcast
echo "=== Broadcasting ==="
BCAST=$(junod tx broadcast /tmp/signed_tx.json --node "$NODE" --output json 2>&1)
echo "$BCAST"

TXHASH=$(echo "$BCAST" | grep -o '{.*}' | jq -r '.txhash // empty')
CODE=$(echo "$BCAST" | grep -o '{.*}' | jq -r '.code // 0')
echo "txhash=$TXHASH code=$CODE"

if [ "$CODE" != "0" ]; then
  echo "raw_log: $(echo "$BCAST" | grep -o '{.*}' | jq -r '.raw_log')"
fi

echo "Waiting 5s..."
sleep 5
echo "=== Contracts for code 2 ==="
junod query wasm list-contract-by-code 2 --node "$NODE" --output json 2>/dev/null | jq '.contracts'
