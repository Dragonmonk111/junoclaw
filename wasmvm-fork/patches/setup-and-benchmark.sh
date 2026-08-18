#!/usr/bin/env bash
# setup-and-benchmark.sh — init chain, start, store wasm, benchmark gas
# All in one script to minimize the WSL2 clock jump window
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"

# Clean slate
pkill -f "junod start" 2>/dev/null || true
sleep 2
rm -rf "${HOME_DIR}"

# === INIT ===
echo "=== Init chain ==="
"${JUNOD}" init bn254-validator --chain-id "${CHAIN_ID}" --home "${HOME_DIR}" 2>&1 | tail -2

"${JUNOD}" keys add validator --keyring-backend test --home "${HOME_DIR}" 2>&1 | grep -E "address|name" || true
VALIDATOR_ADDR=$("${JUNOD}" keys show validator -a --keyring-backend test --home "${HOME_DIR}")

# Fix staking denom
python3 -c "
import json
p = '${HOME_DIR}/config/genesis.json'
with open(p) as f:
    g = json.load(f)
g['app_state']['staking']['params']['bond_denom'] = 'ujuno'
if 'mint' in g['app_state']:
    g['app_state']['mint']['params']['mint_denom'] = 'ujuno'
if 'crisis' in g['app_state']:
    g['app_state']['crisis']['constant_fee']['denom'] = 'ujuno'
if 'gov' in g['app_state']:
    for d in g['app_state']['gov'].get('params',{}).get('min_deposit',[]):
        d['denom'] = 'ujuno'
# Fix feemarket denom
if 'feemarket' in g['app_state']:
    g['app_state']['feemarket']['params']['fee_denom'] = 'ujuno'
with open(p, 'w') as f:
    json.dump(g, f, indent=2)
"

"${JUNOD}" genesis add-genesis-account "${VALIDATOR_ADDR}" 1000000000000ujuno --home "${HOME_DIR}"
"${JUNOD}" genesis gentx validator 1000000000ujuno --chain-id "${CHAIN_ID}" --keyring-backend test --home "${HOME_DIR}" 2>&1 | tail -2
"${JUNOD}" genesis collect-gentxs --home "${HOME_DIR}" 2>&1 | tail -2

# === START ===
echo ""
echo "=== Starting chain ==="
"${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
JUNOD_PID=$!
echo "PID: ${JUNOD_PID}"

# Wait for first new block
echo "Waiting for blocks..."
HEIGHT=0
for i in $(seq 1 20); do
  sleep 3
  HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d['result']['sync_info']['latest_block_height'])
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${HEIGHT}" != "0" ]]; then
    echo "Chain at height ${HEIGHT}"
    break
  fi
  echo "  ...(${i})"
done

if [[ "${HEIGHT}" == "0" ]]; then
  echo "ERROR: Chain not producing blocks"
  exit 1
fi

# === STORE PRECOMPILE WASM ===
echo ""
echo "=== Storing zk_verifier_precompile.wasm ==="
STORE_OUT=$(timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${STORE_OUT}" | tail -5

sleep 7

# Get tx hash
TXHASH=$(echo "${STORE_OUT}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            print(d.get('txhash',''))
            break
        except:
            pass
" 2>/dev/null)

echo "txhash: ${TXHASH}"

# Query tx for gas and code_id
if [[ -n "${TXHASH}" ]]; then
  echo "=== Precompile store result ==="
  "${JUNOD}" query tx "${TXHASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print('gas_used:', d.get('gas_used','?'))
    print('gas_wanted:', d.get('gas_wanted','?'))
    print('code:', d.get('code','?'))
    for event in d.get('events', []):
        if event.get('type') == 'store_code':
            for attr in event.get('attributes', []):
                if attr.get('key') == 'code_id':
                    print('code_id:', attr.get('value'))
except Exception as e:
    print('parse error:', e)
    print(sys.stdin.read()[:500])
" 2>/dev/null
fi

# === STORE PURE WASM ===
echo ""
echo "=== Storing zk_verifier_pure.wasm ==="
STORE_OUT2=$(timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${STORE_OUT2}" | tail -5

sleep 7

TXHASH2=$(echo "${STORE_OUT2}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            print(d.get('txhash',''))
            break
        except:
            pass
" 2>/dev/null)

echo "txhash: ${TXHASH2}"

if [[ -n "${TXHASH2}" ]]; then
  echo "=== Pure wasm store result ==="
  "${JUNOD}" query tx "${TXHASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print('gas_used:', d.get('gas_used','?'))
    print('gas_wanted:', d.get('gas_wanted','?'))
    print('code:', d.get('code','?'))
    for event in d.get('events', []):
        if event.get('type') == 'store_code':
            for attr in event.get('attributes', []):
                if attr.get('key') == 'code_id':
                    print('code_id:', attr.get('value'))
except Exception as e:
    print('parse error:', e)
" 2>/dev/null
fi

# === LIST CODES ===
echo ""
echo "=== Stored codes ==="
"${JUNOD}" query wasm list-code 2>&1 | head -20

# === WASMVM VERSION ===
echo ""
echo "=== wasmvm version ==="
"${JUNOD}" query wasm libwasmvm-version 2>&1

echo ""
echo "=== DONE ==="
echo "Chain PID: ${JUNOD_PID}"
echo "To stop: kill ${JUNOD_PID}"
