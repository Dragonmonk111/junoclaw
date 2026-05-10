#!/bin/bash
# Quick devnet status check
set -e

PURE="juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8"
PRECOMPILE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"
NODE="junoclaw-bn254-devnet"

echo "=== Chain Status ==="
docker exec $NODE junod status 2>/dev/null | python3 -c '
import sys, json
d = json.load(sys.stdin)
si = d.get("sync_info", {})
vi = d.get("validator_info", {})
h = si.get("latest_block_height")
t = si.get("latest_block_time")
c = si.get("catching_up")
v = vi.get("voting_power")
print(f"  Height:       {h}")
print(f"  Block time:   {t}")
print(f"  Catching up:  {c}")
print(f"  Voting power: {v}")
'

echo ""
echo "=== Pure Contract (code 1): $PURE ==="
docker exec $NODE junod query wasm contract-state smart "$PURE" '{"vk_status":{}}' --output json 2>/dev/null | python3 -c '
import sys, json
d = json.load(sys.stdin)
data = d.get("data", {})
hv = data.get("has_vk")
vs = data.get("vk_size_bytes", "?")
print(f"  has_vk: {hv}  vk_size: {vs}")
'

echo ""
echo "=== Precompile Contract (code 2): $PRECOMPILE ==="
docker exec $NODE junod query wasm contract-state smart "$PRECOMPILE" '{"vk_status":{}}' --output json 2>/dev/null | python3 -c '
import sys, json
d = json.load(sys.stdin)
data = d.get("data", {})
hv = data.get("has_vk")
vs = data.get("vk_size_bytes", "?")
print(f"  has_vk: {hv}  vk_size: {vs}")
'

echo ""
echo "=== Current block ==="
sleep 2
docker exec $NODE junod status 2>/dev/null | python3 -c '
import sys, json
d = json.load(sys.stdin)
si = d.get("sync_info", {})
h = si.get("latest_block_height")
print(f"  Height now: {h}  (chain advancing)")
'
