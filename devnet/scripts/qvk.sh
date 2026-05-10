#!/bin/sh
PURE="juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8"
PRECOMPILE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"

echo "=== Pure contract VK status ==="
junod query wasm contract-state smart "$PURE" '{"vk_status":{}}' --output json 2>&1
echo ""
echo "=== Precompile contract VK status ==="
junod query wasm contract-state smart "$PRECOMPILE" '{"vk_status":{}}' --output json 2>&1
