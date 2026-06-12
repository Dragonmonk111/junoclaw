#!/usr/bin/env bash
set -euo pipefail

INIT='{"admin":"juno1z72k4yhdgfxr6msnhkcjxltuwy0hvcz4e5lqrm","genesis":"juno1z72k4yhdgfxr6msnhkcjxltuwy0hvcz4e5lqrm"}'

docker exec junoclaw-bn254-devnet junod tx wasm instantiate 1 "$INIT" \
  --from admin \
  --label "jclaw-mayo" \
  --no-admin \
  --chain-id junoclaw-bn254-1 \
  --keyring-backend test \
  --gas auto \
  --gas-adjustment 1.3 \
  --gas-prices 0.075ujuno \
  --broadcast-mode sync \
  --yes \
  --output json
