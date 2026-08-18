#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"

echo "=== Storing precompile wasm ==="
RESULT=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id junoclaw-bn254-local --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode block -y 2>&1)
echo "${RESULT}" | python3 -c "
import sys, json, re
text = sys.stdin.read()
# Find the JSON line
for line in text.split('\n'):
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            print('txhash:', d.get('txhash','?'))
            print('gas_used:', d.get('gas_used','?'))
            print('gas_wanted:', d.get('gas_wanted','?'))
            print('code:', d.get('code','?'))
            print('raw_log:', d.get('raw_log','?')[:200])
            # Find code_id
            for event in d.get('events', []):
                if event.get('type') == 'store_code':
                    for attr in event.get('attributes', []):
                        if attr.get('key') == 'code_id':
                            print('code_id:', attr.get('value'))
        except:
            pass
"

echo ""
echo "=== Storing pure wasm ==="
RESULT2=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id junoclaw-bn254-local --gas 5000000 --gas-prices 0ujuno \
  --broadcast-mode block -y 2>&1)
echo "${RESULT2}" | python3 -c "
import sys, json
text = sys.stdin.read()
for line in text.split('\n'):
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            print('txhash:', d.get('txhash','?'))
            print('gas_used:', d.get('gas_used','?'))
            print('code:', d.get('code','?'))
            print('raw_log:', d.get('raw_log','?')[:200])
            for event in d.get('events', []):
                if event.get('type') == 'store_code':
                    for attr in event.get('attributes', []):
                        if attr.get('key') == 'code_id':
                            print('code_id:', attr.get('value'))
        except:
            pass
"

echo ""
echo "=== Querying stored codes ==="
"${JUNOD}" query wasm list-code 2>&1 | head -20
