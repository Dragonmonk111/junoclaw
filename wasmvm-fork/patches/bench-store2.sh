#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"

# Chain should already be running from setup-and-benchmark.sh
HEIGHT=$(curl -s http://localhost:26657/status | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d['result']['sync_info']['latest_block_height'])
" 2>/dev/null || echo "0")
echo "Chain at height ${HEIGHT}"

if [[ "${HEIGHT}" == "0" ]]; then
  echo "ERROR: Chain not running"
  exit 1
fi

# Store precompile wasm with actual fees
echo ""
echo "=== Storing zk_verifier_precompile.wasm ==="
STORE_OUT=$(timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${STORE_OUT}" | tail -5

sleep 7

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

if [[ -n "${TXHASH}" ]]; then
  echo "=== Precompile store result ==="
  "${JUNOD}" query tx "${TXHASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print('gas_used:', d.get('gas_used','?'))
    print('gas_wanted:', d.get('gas_wanted','?'))
    print('code:', d.get('code','?'))
    print('raw_log:', str(d.get('raw_log','?'))[:200])
    for event in d.get('events', []):
        if event.get('type') == 'store_code':
            for attr in event.get('attributes', []):
                if attr.get('key') == 'code_id':
                    print('code_id:', attr.get('value'))
except Exception as e:
    print('parse error:', e)
" 2>/dev/null
fi

# Store pure wasm
echo ""
echo "=== Storing zk_verifier_pure.wasm ==="
STORE_OUT2=$(timeout 30 "${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
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
    print('raw_log:', str(d.get('raw_log','?'))[:200])
    for event in d.get('events', []):
        if event.get('type') == 'store_code':
            for attr in event.get('attributes', []):
                if attr.get('key') == 'code_id':
                    print('code_id:', attr.get('value'))
except Exception as e:
    print('parse error:', e)
" 2>/dev/null
fi

# List codes
echo ""
echo "=== Stored codes ==="
"${JUNOD}" query wasm list-code 2>&1

echo ""
echo "=== wasmvm version ==="
"${JUNOD}" query wasm libwasmvm-version 2>&1

echo ""
echo "=== DONE ==="
