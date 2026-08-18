#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"

# Kill any previous instance
if [[ -f "${HOME_DIR}/pid" ]]; then
  kill "$(cat ${HOME_DIR}/pid)" 2>/dev/null || true
  sleep 2
fi
rm -rf "${HOME_DIR}"

echo "=== Step 1: Init chain ==="
${JUNOD} init bn254-validator --chain-id ${CHAIN_ID} --home ${HOME_DIR}

echo "=== Step 2: Create validator key ==="
${JUNOD} keys add validator --keyring-backend test --home ${HOME_DIR}

VALIDATOR_ADDR=$(${JUNOD} keys show validator -a --keyring-backend test --home ${HOME_DIR})
echo "Validator address: ${VALIDATOR_ADDR}"

echo "=== Step 2.5: Fix staking denom to ujuno ==="
python3 -c "
import json
genesis_path = '${HOME_DIR}/config/genesis.json'
with open(genesis_path) as f:
    g = json.load(f)
# Fix staking params
g['app_state']['staking']['params']['bond_denom'] = 'ujuno'
# Fix mint denom
if 'mint' in g['app_state']:
    g['app_state']['mint']['params']['mint_denom'] = 'ujuno'
# Fix crisis denom
if 'crisis' in g['app_state']:
    g['app_state']['crisis']['constant_fee']['denom'] = 'ujuno'
# Fix gov params
if 'gov' in g['app_state']:
    for p in g['app_state']['gov'].get('params', {}).get('min_deposit', []):
        p['denom'] = 'ujuno'
with open(genesis_path, 'w') as f:
    json.dump(g, f, indent=2)
print('Fixed staking denom to ujuno')
"

echo "=== Step 3: Add genesis account ==="
${JUNOD} genesis add-genesis-account ${VALIDATOR_ADDR} 1000000000000ujuno --home ${HOME_DIR}

echo "=== Step 4: Create gentx ==="
${JUNOD} genesis gentx validator 1000000000ujuno \
  --chain-id ${CHAIN_ID} \
  --keyring-backend test \
  --home ${HOME_DIR}

echo "=== Step 5: Collect gentxs ==="
${JUNOD} genesis collect-gentxs --home ${HOME_DIR}

echo "=== Step 6: Enable wasm in genesis ==="
# Juno v30 should have wasm enabled by default in genesis
# Check if we need to set the wasm params
python3 -c "
import json
with open('${HOME_DIR}/config/genesis.json') as f:
    g = json.load(f)
wasm = g.get('app_state', {}).get('wasm', {})
print('wasm in genesis:', bool(wasm))
if wasm:
    print('wasm params:', json.dumps(wasm.get('params', {}), indent=2))
"

echo "=== Step 7: Start chain ==="
${JUNOD} start --home ${HOME_DIR} --log_level info --minimum-gas-prices 0ujuno &
JUNOD_PID=$!
echo ${JUNOD_PID} > ${HOME_DIR}/pid
echo "junod started, PID: ${JUNOD_PID}"

# Wait for chain
echo "Waiting for blocks..."
for i in $(seq 1 60); do
  sleep 2
  HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys,json
try:
    d=json.load(sys.stdin)
    h=d['result']['sync_info']['latest_block_height']
    print(h if h else '0')
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${HEIGHT}" != "0" ]]; then
    echo "Chain at height ${HEIGHT}"
    break
  fi
  echo "  ...waiting (${i})"
done

if [[ "${HEIGHT}" == "0" ]]; then
  echo "ERROR: Chain did not produce blocks"
  echo "=== Last 20 lines of junod output ==="
  exit 1
fi

echo ""
echo "=== Wasm params ==="
${JUNOD} query wasm params --node http://localhost:26657 2>&1

echo ""
echo "=== wasmvm version ==="
${JUNOD} query wasm libwasmvm-version --node http://localhost:26657 2>&1

echo ""
echo "=== Chain is running ==="
echo "PID: ${JUNOD_PID}"
echo "RPC: http://localhost:26657"
echo "Home: ${HOME_DIR}"
