#!/usr/bin/env bash
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

# Check chain status
echo "=== Chain status ==="
curl -s http://localhost:26657/status | python3 -c "
import sys, json
d = json.load(sys.stdin)
si = d['result']['sync_info']
print('height:', si['latest_block_height'])
print('time:', si['latest_block_time'])
"

# Check if still producing blocks
sleep 5
echo "=== After 5s ==="
curl -s http://localhost:26657/status | python3 -c "
import sys, json
d = json.load(sys.stdin)
si = d['result']['sync_info']
print('height:', si['latest_block_height'])
print('time:', si['latest_block_time'])
"

# Try storing wasm
echo ""
echo "=== Storing precompile wasm ==="
JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet/zk_verifier_precompile.wasm"

timeout 60 "${JUNOD}" tx wasm store "${WASM}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id junoclaw-bn254-local --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode block -y 2>&1

echo ""
echo "=== Done ==="
