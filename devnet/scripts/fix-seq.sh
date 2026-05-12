#!/bin/bash
set -e
NODE="http://localhost:26657"
CHAIN="junoclaw-bn254-1"
ADMIN="juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"

echo "=== Raw account query ==="
curl -s "$NODE/abci_query?path=\"/cosmos.auth.v1beta1.Query/Account\"&data=$(printf '%s' "0a2d$ADMIN" | xxd -p)" 2>/dev/null | head -5 || true

echo "=== junod query account (raw output) ==="
junod query auth account "$ADMIN" --node "$NODE" 2>&1 | head -20

echo ""
echo "=== Try with explicit sequence ==="
# Query what sequence the chain expects
ACCT_JSON=$(curl -s "http://localhost:1317/cosmos/auth/v1beta1/accounts/$ADMIN" 2>/dev/null || echo '{}')
echo "REST account response:"
echo "$ACCT_JSON" | jq '.' 2>/dev/null || echo "$ACCT_JSON"

SEQ=$(echo "$ACCT_JSON" | jq -r '.account.sequence // "0"' 2>/dev/null)
ACCT_NUM=$(echo "$ACCT_JSON" | jq -r '.account.account_number // "0"' 2>/dev/null)
echo "account_number=$ACCT_NUM sequence=$SEQ"

echo ""
echo "=== Instantiate with explicit seq ==="
junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id "$CHAIN" \
  --keyring-backend test \
  --node "$NODE" \
  --gas 800000 --gas-prices 0.1ujuno \
  --label "zkprecompile" \
  --no-admin \
  --account-number "$ACCT_NUM" --sequence "$SEQ" \
  --broadcast-mode sync --yes --output json 2>&1

echo ""
echo "=== Wait and check ==="
sleep 5
junod query wasm list-contract-by-code 2 --node "$NODE" --output json 2>/dev/null | jq '.contracts'
