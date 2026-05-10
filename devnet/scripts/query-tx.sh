#!/bin/bash
NODE="http://localhost:26657"
TXHASH="D19D56E2C63B113E0962354FCF5848E42D2163D42FAC749FCB3F5587827CFE9E"

echo "=== Query tx $TXHASH ==="
RESULT=$(junod query tx "$TXHASH" --node "$NODE" --output json 2>&1)
if echo "$RESULT" | jq -e '.code' >/dev/null 2>&1; then
  echo "$RESULT" | jq '{code, raw_log, gas_wanted, gas_used, height}'
  echo ""
  echo "Events:"
  echo "$RESULT" | jq '[.events[] | .type]'
  echo ""
  echo "Contract address (if any):"
  echo "$RESULT" | jq '[.events[]? | select(.type == "instantiate") | .attributes[]? | select(.key == "_contract_address") | .value]'
else
  echo "TX not found or error:"
  echo "$RESULT" | head -5
fi
