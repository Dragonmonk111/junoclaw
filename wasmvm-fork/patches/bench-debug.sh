#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"

echo "=== Query instantiate tx 1 (precompile) ==="
"${JUNOD}" query tx A84013E5B60EA58129454182E8C97F6FFB8CAA60B418516F1B58ABABC2A7FE69 --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('code:', d.get('code','?'))
print('gas_used:', d.get('gas_used','?'))
print('raw_log:', str(d.get('raw_log','?'))[:500])
for event in d.get('events', []):
    if event.get('type') == 'instantiate_contract':
        for attr in event.get('attributes', []):
            print(f'  {attr.get(\"key\")}: {attr.get(\"value\")}')
"

echo ""
echo "=== Query instantiate tx 2 (pure) ==="
"${JUNOD}" query tx 4D69E69F85E2B486E8E3FA739321E19FBCE1B6FDA1A56415DC8EC8AD253E6202 --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('code:', d.get('code','?'))
print('gas_used:', d.get('gas_used','?'))
print('raw_log:', str(d.get('raw_log','?'))[:500])
for event in d.get('events', []):
    if event.get('type') == 'instantiate_contract':
        for attr in event.get('attributes', []):
            print(f'  {attr.get(\"key\")}: {attr.get(\"value\")}')
"

echo ""
echo "=== List contracts ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1
echo "---"
"${JUNOD}" query wasm list-contract-by-code 2 2>&1

echo ""
echo "=== Try simpler init msg ==="
# Try with empty JSON object
echo "Instantiating code 1 with {} ..."
INST=$("${JUNOD}" tx wasm instantiate 1 "{}" \
  --label "zk-pre" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync --no-admin -y 2>&1)
echo "${INST}" | tail -5

sleep 7

# Get hash
HASH=$(echo "${INST}" | python3 -c "
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

if [[ -n "${HASH}" ]]; then
  "${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('code:', d.get('code','?'))
print('gas_used:', d.get('gas_used','?'))
print('raw_log:', str(d.get('raw_log','?'))[:500])
for event in d.get('events', []):
    if event.get('type') == 'instantiate_contract':
        for attr in event.get('attributes', []):
            print(f'  {attr.get(\"key\")}: {attr.get(\"value\")}')
"
fi

echo ""
echo "=== List contracts after retry ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1
