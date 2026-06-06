#!/bin/bash
# Robust single-shot deploy that runs entirely INSIDE the container.
# Blocks on each tx's on-chain commit before sending the next, so the
# account sequence always increments and we never hit the seq-mismatch race.
set -uo pipefail

CHAIN_ID=junoclaw-bn254-1
KEYRING=test
GAS=auto
GAS_ADJ=1.5
GAS_PRICES=0.075ujuno

ADMIN=$(junod keys show admin -a --keyring-backend "$KEYRING")
INIT="{\"admin\":\"$ADMIN\"}"
echo "admin=$ADMIN"

# Broadcast a tx (sync) and echo its txhash.
bcast() {
  junod tx "$@" \
    --from admin --chain-id "$CHAIN_ID" --keyring-backend "$KEYRING" \
    --gas "$GAS" --gas-adjustment "$GAS_ADJ" --gas-prices "$GAS_PRICES" \
    --broadcast-mode sync --yes --output json | jq -r '.txhash'
}

# Block until a txhash is committed; print the full tx json. Fail on code!=0.
wait_tx() {
  local h="$1" out=""
  for i in $(seq 1 40); do
    sleep 1
    out=$(junod query tx "$h" --output json 2>/dev/null || true)
    if [ -n "$out" ] && [ "$(echo "$out" | jq -r '.code // empty')" != "" ]; then
      local code
      code=$(echo "$out" | jq -r '.code')
      if [ "$code" != "0" ]; then
        echo "tx $h failed with code $code: $(echo "$out" | jq -r '.raw_log')" >&2
        return 1
      fi
      echo "$out"
      return 0
    fi
  done
  echo "timed out waiting for tx $h" >&2
  return 1
}

echo "[1/4] store pure"
PURE_TX=$(bcast wasm store /tmp/zk_verifier_pure.wasm)
PURE_JSON=$(wait_tx "$PURE_TX") || exit 1
PURE_CODE=$(echo "$PURE_JSON" | jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value' | head -n1)
echo "  PURE_CODE=$PURE_CODE"

echo "[2/4] store precompile"
PREC_TX=$(bcast wasm store /tmp/zk_verifier_precompile.wasm)
PREC_JSON=$(wait_tx "$PREC_TX") || exit 1
PREC_CODE=$(echo "$PREC_JSON" | jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value' | head -n1)
echo "  PREC_CODE=$PREC_CODE"

[ -n "$PURE_CODE" ] && [ -n "$PREC_CODE" ] || { echo "missing code_id" >&2; exit 1; }

echo "[3/4] instantiate pure"
PURE_ITX=$(bcast wasm instantiate "$PURE_CODE" "$INIT" --label zk-verifier-pure --no-admin)
PURE_IJSON=$(wait_tx "$PURE_ITX") || exit 1
PURE_ADDR=$(echo "$PURE_IJSON" | jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value' | head -n1)
echo "  PURE_ADDR=$PURE_ADDR"

echo "[4/4] instantiate precompile"
PREC_ITX=$(bcast wasm instantiate "$PREC_CODE" "$INIT" --label zk-verifier-precompile --no-admin)
PREC_IJSON=$(wait_tx "$PREC_ITX") || exit 1
PREC_ADDR=$(echo "$PREC_IJSON" | jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value' | head -n1)
echo "  PREC_ADDR=$PREC_ADDR"

[ -n "$PURE_ADDR" ] && [ -n "$PREC_ADDR" ] || { echo "missing addr" >&2; exit 1; }

cat > /tmp/deploy.env <<EOF
CHAIN_ID=$CHAIN_ID
NODE=http://localhost:26657
PURE_CODE_ID=$PURE_CODE
PURE_ADDR=$PURE_ADDR
PRECOMPILE_CODE_ID=$PREC_CODE
PRECOMPILE_ADDR=$PREC_ADDR
ADMIN_ADDR=$ADMIN
EOF

echo "DEPLOY_OK"
cat /tmp/deploy.env
