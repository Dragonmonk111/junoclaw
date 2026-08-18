#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"

# Kill stalled junod
echo "=== Killing stalled junod ==="
pkill -f "junod start" || true
sleep 3

# Restart
echo "=== Restarting junod ==="
"${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
JUNOD_PID=$!
echo "junod PID: ${JUNOD_PID}"

# Wait for blocks
echo "Waiting for blocks..."
for i in $(seq 1 30); do
  sleep 3
  HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    h = d['result']['sync_info']['latest_block_height']
    print(h)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${HEIGHT}" != "0" && "${HEIGHT}" != "28" ]]; then
    echo "Chain resumed at height ${HEIGHT}"
    break
  fi
  echo "  ...waiting (${i})"
done

if [[ "${HEIGHT}" == "0" || "${HEIGHT}" == "28" ]]; then
  echo "ERROR: Chain still stalled. Need wsl --shutdown."
  exit 1
fi

# Store precompile wasm with sync broadcast
echo ""
echo "=== Storing precompile wasm ==="
timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id junoclaw-bn254-local --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode sync -y 2>&1

echo ""
sleep 6

# Check tx result
echo "=== Querying latest txs ==="
"${JUNOD}" query tx --type=hash $("${JUNOD}" q tx-search --events "message.action=store_code" --limit 1 -o json 2>/dev/null | python3 -c "
import sys, json
d = json.load(sys.stdin)
txs = d.get('txs', [])
if txs:
    print(txs[0]['txhash'])
" 2>/dev/null) 2>&1 | grep -E "gas|code_id|height" | head -10

echo ""
echo "=== List codes ==="
"${JUNOD}" query wasm list-code 2>&1 | head -20

echo ""
echo "=== Storing pure wasm ==="
timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id junoclaw-bn254-local --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode sync -y 2>&1

sleep 6
echo ""
echo "=== List codes after both stores ==="
"${JUNOD}" query wasm list-code 2>&1 | head -30

echo ""
echo "=== Done ==="
