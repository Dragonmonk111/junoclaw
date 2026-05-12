#!/bin/bash
echo "starting inst2"
junod tx wasm instantiate 2 '{"admin":"juno1axckrxrjpckw00800s9v6yga6mr0dpge45ejsz"}' --from validator --chain-id junoclaw-bn254-1 --keyring-backend test --node http://localhost:26657 --gas 800000 --gas-prices 0.1ujuno --label zkprecompile --no-admin --broadcast-mode sync --yes --output json > /tmp/inst2_out.txt 2>&1
echo "exit code: $?"
cat /tmp/inst2_out.txt
