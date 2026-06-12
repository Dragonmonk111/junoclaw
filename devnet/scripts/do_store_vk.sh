#!/bin/sh
MSG="$(cat /tmp/store_vk_msg.json)"
junod tx wasm execute juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p "$MSG" \
  --from admin --chain-id junoclaw-bn254-1 --keyring-backend test \
  --gas auto --gas-adjustment 1.5 --gas-prices 0.075ujuno \
  --broadcast-mode sync --yes --output json --node http://localhost:26657
