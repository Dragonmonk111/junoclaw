#!/usr/bin/env bash
set -euo pipefail
CONTAINER="junoclaw-bn254-devnet"
CHAIN_ID="junoclaw-bn254-1"
KEYRING="test"

ADMIN=$(docker exec "${CONTAINER}" junod keys show admin -a --keyring-backend "${KEYRING}")
echo "admin=${ADMIN}"

# Store
STORE_TX=$(docker exec "${CONTAINER}" junod tx wasm store /tmp/moultbook_v0.wasm \
  --from admin --chain-id "${CHAIN_ID}" --keyring-backend "${KEYRING}" \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.025ujuno \
  --broadcast-mode sync --yes --output json | jq -r '.txhash')
echo "store_tx=${STORE_TX}"

# Aggressive poll (max 15s)
CODE_ID=""
for i in $(seq 1 15); do
  sleep 1
  out=$(docker exec "${CONTAINER}" junod query tx "${STORE_TX}" --output json 2>/dev/null || true)
  if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
    CODE_ID=$(echo "${out}" | jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value' | head -n1)
    break
  fi
done
echo "code_id=${CODE_ID}"

if [ -z "${CODE_ID}" ]; then
  echo "error: store_code failed or not indexed" >&2
  exit 1
fi

# Instantiate
INIT=$(jq -nc \
  --arg admin "${ADMIN}" \
  --argjson maxsize 1048576 \
  --argjson maxrefs 8 \
  --argjson maxctlen 64 \
  --argjson maxgroup 50 \
  '{
     admin: $admin,
     whoami_contract: null,
     max_size_bytes: $maxsize,
     max_refs: $maxrefs,
     max_content_type_len: $maxctlen,
     max_group_size: $maxgroup
   }')

INIT_TX=$(docker exec "${CONTAINER}" junod tx wasm instantiate "${CODE_ID}" "${INIT}" \
  --from admin --chain-id "${CHAIN_ID}" --keyring-backend "${KEYRING}" \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.025ujuno \
  --label "moultbook-v0" --admin "${ADMIN}" \
  --broadcast-mode sync --yes --output json | jq -r '.txhash')
echo "init_tx=${INIT_TX}"

# Poll for instantiate
ADDR=""
for i in $(seq 1 15); do
  sleep 1
  out=$(docker exec "${CONTAINER}" junod query tx "${INIT_TX}" --output json 2>/dev/null || true)
  if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
    ADDR=$(echo "${out}" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' | head -n1)
    break
  fi
done
echo "addr=${ADDR}"
