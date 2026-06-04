#!/bin/bash
set -e
CHAIN_ID=junoclaw-bn254-1
KEYRING=test
GAS=auto
GAS_ADJ=1.5
GAS_PRICES=0.075ujuno

junod tx wasm store /tmp/zk_verifier_pure.wasm --from admin --chain-id $CHAIN_ID --keyring-backend $KEYRING --gas $GAS --gas-adjustment $GAS_ADJ --gas-prices $GAS_PRICES --broadcast-mode sync --yes --output json > /tmp/pure_tx.json 2>&1 || true
sleep 4
junod tx wasm store /tmp/zk_verifier_precompile.wasm --from admin --chain-id $CHAIN_ID --keyring-backend $KEYRING --gas $GAS --gas-adjustment $GAS_ADJ --gas-prices $GAS_PRICES --broadcast-mode sync --yes --output json > /tmp/prec_tx.json 2>&1 || true
sleep 4

PURE_TX=$(jq -r '.txhash // empty' /tmp/pure_tx.json)
PREC_TX=$(jq -r '.txhash // empty' /tmp/prec_tx.json)

get_code_id() {
  local tx_hash=$1
  local out=''
  for i in 1 2 3 4 5 6 7 8; do
    sleep 1
    out=$(junod query tx "$tx_hash" --output json 2>/dev/null)
    if [ -n "$out" ] && [ "$(echo "$out" | jq -r '.code // empty')" != "" ]; then
      break
    fi
  done
  echo "$out" | jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value' | head -n1
}

PURE_CODE=$(get_code_id "$PURE_TX")
PREC_CODE=$(get_code_id "$PREC_TX")

echo "PURE_CODE=$PURE_CODE PREC_CODE=$PREC_CODE"

if [ -z "$PURE_CODE" ] || [ -z "$PREC_CODE" ]; then
  echo 'Missing code_id' >&2
  exit 1
fi

ADMIN=$(junod keys show admin -a --keyring-backend $KEYRING)
INIT="{\"admin\":\"$ADMIN\"}"

junod tx wasm instantiate "$PURE_CODE" "$INIT" --from admin --label zk-verifier-pure --no-admin --chain-id $CHAIN_ID --keyring-backend $KEYRING --gas $GAS --gas-adjustment $GAS_ADJ --gas-prices $GAS_PRICES --broadcast-mode sync --yes --output json > /tmp/pure_inst.json 2>&1 || true
sleep 3
junod tx wasm instantiate "$PREC_CODE" "$INIT" --from admin --label zk-verifier-precompile --no-admin --chain-id $CHAIN_ID --keyring-backend $KEYRING --gas $GAS --gas-adjustment $GAS_ADJ --gas-prices $GAS_PRICES --broadcast-mode sync --yes --output json > /tmp/prec_inst.json 2>&1 || true
sleep 3

get_addr() {
  local tx_hash=$1
  local out=''
  for i in 1 2 3 4 5 6 7 8; do
    sleep 1
    out=$(junod query tx "$tx_hash" --output json 2>/dev/null)
    if [ -n "$out" ] && [ "$(echo "$out" | jq -r '.code // empty')" != "" ]; then
      break
    fi
  done
  echo "$out" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' | head -n1
}

PURE_INST_TX=$(jq -r '.txhash // empty' /tmp/pure_inst.json)
PREC_INST_TX=$(jq -r '.txhash // empty' /tmp/prec_inst.json)

PURE_ADDR=$(get_addr "$PURE_INST_TX")
PREC_ADDR=$(get_addr "$PREC_INST_TX")

echo "PURE_ADDR=$PURE_ADDR PREC_ADDR=$PREC_ADDR"

if [ -z "$PURE_ADDR" ] || [ -z "$PREC_ADDR" ]; then
  echo 'Missing addr' >&2
  exit 1
fi

cat > /tmp/deploy.env << EOF
CHAIN_ID=$CHAIN_ID
NODE=http://localhost:26657
PURE_CODE_ID=$PURE_CODE
PURE_ADDR=$PURE_ADDR
PRECOMPILE_CODE_ID=$PREC_CODE
PRECOMPILE_ADDR=$PREC_ADDR
ADMIN_ADDR=$ADMIN
EOF

echo 'DEPLOY_OK'
