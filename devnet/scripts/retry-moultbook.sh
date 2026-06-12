#!/usr/bin/env bash
set -euo pipefail
CONTAINER="junoclaw-bn254-devnet"
CHAIN_ID="junoclaw-bn254-1"
KEYRING="test"

ADMIN=$(docker exec "${CONTAINER}" junod keys show admin -a --keyring-backend "${KEYRING}")
echo "admin=${ADMIN}"

for attempt in 1 2 3; do
  echo "[retry-moultbook] Store attempt ${attempt}/3 …"
  STORE_JSON=$(docker exec "${CONTAINER}" junod tx wasm store /tmp/moultbook_v0.wasm \
    --from admin --chain-id "${CHAIN_ID}" --keyring-backend "${KEYRING}" \
    --gas auto --gas-adjustment 1.3 --gas-prices 0.025ujuno \
    --broadcast-mode sync --yes --output json)
  STORE_TX=$(echo "${STORE_JSON}" | jq -r '.txhash // empty')
  if [ -z "${STORE_TX}" ]; then
    echo "  no txhash, retrying …"
    sleep 2
    continue
  fi
  echo "  txhash=${STORE_TX}"

  # Quick poll
  for i in $(seq 1 8); do
    sleep 1
    out=$(docker exec "${CONTAINER}" junod query tx "${STORE_TX}" --output json 2>/dev/null || true)
    if [ -n "${out}" ]; then
      CODE=$(echo "${out}" | jq -r '.code // empty')
      if [ -n "${CODE}" ]; then
        echo "  indexed code=${CODE}"
        if [ "${CODE}" = "0" ]; then
          CODE_ID=$(echo "${out}" | jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value' | head -n1)
          echo "  code_id=${CODE_ID}"
          if [ -n "${CODE_ID}" ]; then
            echo "SUCCESS_STORE=${CODE_ID}"
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
            INST_JSON=$(docker exec "${CONTAINER}" junod tx wasm instantiate "${CODE_ID}" "${INIT}" \
              --from admin --chain-id "${CHAIN_ID}" --keyring-backend "${KEYRING}" \
              --gas auto --gas-adjustment 1.3 --gas-prices 0.025ujuno \
              --label "moultbook-v0" --admin "${ADMIN}" \
              --broadcast-mode sync --yes --output json)
            INST_TX=$(echo "${INST_JSON}" | jq -r '.txhash // empty')
            echo "  inst_tx=${INST_TX}"
            for j in $(seq 1 8); do
              sleep 1
              inst_out=$(docker exec "${CONTAINER}" junod query tx "${INST_TX}" --output json 2>/dev/null || true)
              if [ -n "${inst_out}" ]; then
                INST_CODE=$(echo "${inst_out}" | jq -r '.code // empty')
                if [ -n "${INST_CODE}" ] && [ "${INST_CODE}" = "0" ]; then
                  ADDR=$(echo "${inst_out}" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' | head -n1)
                  echo "SUCCESS_ADDR=${ADDR}"
                  exit 0
                fi
              fi
            done
            echo "  instantiate not indexed, retrying whole flow …"
            break
          fi
        else
          echo "  tx failed with code=${CODE}, retrying …"
          break
        fi
      fi
    fi
  done
  echo "  attempt ${attempt} failed, retrying …"
  sleep 2
done

echo "error: all 3 attempts failed" >&2
exit 1
