#!/bin/bash
echo "=== wasmvm version ==="
junod version --long 2>&1 | grep -i wasm || junod version 2>&1

echo ""
echo "=== Check if bn254 capability is registered ==="
# Query code info which shows required capabilities
junod query wasm code-info 1 --node http://localhost:26657 --output json 2>/dev/null | jq '.'
echo "---"
junod query wasm code-info 2 --node http://localhost:26657 --output json 2>/dev/null | jq '.'

echo ""
echo "=== Try instantiate with explicit gas (no simulation) ==="
ADMIN=$(junod keys show validator -a --keyring-backend test)
# The key: use --gas flag (not auto) to skip simulation
junod tx wasm instantiate 2 '{"admin":"'"$ADMIN"'"}' \
  --from validator \
  --chain-id junoclaw-bn254-1 \
  --keyring-backend test \
  --node http://localhost:26657 \
  --gas 2000000 --fees 200000ujuno \
  --label "zkprecompile" \
  --no-admin \
  --broadcast-mode sync --yes --output json 2>&1
