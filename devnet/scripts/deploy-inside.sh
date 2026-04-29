#!/bin/bash
H="http://localhost:26657"
C="junoclaw-bn254-1"

echo "=== STORE PURE ==="
R1=$(junod tx wasm store /tmp/zk_verifier_pure.wasm --from validator --gas auto --gas-adjustment 1.3 --chain-id $C --keyring-backend test --node $H --yes 2>&1)
echo "$R1" | grep -E "txhash|gas"

echo "=== STORE PRECOMPILE ==="
R2=$(junod tx wasm store /tmp/zk_verifier_precompile.wasm --from validator --gas auto --gas-adjustment 1.3 --chain-id $C --keyring-backend test --node $H --yes 2>&1)
echo "$R2" | grep -E "txhash|gas"

echo "=== LIST CODES ==="
junod query wasm list-code --node $H 2>&1 | grep -E "code_id|creator" | tail -6
