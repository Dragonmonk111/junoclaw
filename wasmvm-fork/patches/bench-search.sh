#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"

echo "=== Search for all wasm txs ==="
"${JUNOD}" query tx-search --events "message.module=wasm" --limit 10 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
for tx in d.get('txs', []):
    print(f\"height={tx.get('height','?')} txhash={tx.get('txhash','?')[:16]}... code={tx.get('code',0)} gas_used={tx.get('gas_used','?')}\")
    for event in tx.get('events', []):
        etype = event.get('type','')
        if etype in ('store_code','instantiate_contract','execute_contract'):
            attrs = {a['key']: a['value'] for a in event.get('attributes',[])}
            print(f'  {etype}: {attrs}')
"

echo ""
echo "=== Try instantiate code 1 with {} ==="
INST=$("${JUNOD}" tx wasm instantiate 1 "{}" \
  --label "zk-pre-bench" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${INST}" | tail -5

sleep 10

echo ""
echo "=== Search for instantiate txs ==="
"${JUNOD}" query tx-search --events "message.action=instantiate_contract" --limit 5 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
for tx in d.get('txs', []):
    print(f\"height={tx.get('height','?')} txhash={tx.get('txhash','?')} code={tx.get('code',0)} gas_used={tx.get('gas_used','?')}\")
    print(f'  raw_log: {str(tx.get(\"raw_log\",\"\"))[:300]}')
    for event in tx.get('events', []):
        if event.get('type') == 'instantiate_contract':
            for attr in event.get('attributes', []):
                print(f'  {attr[\"key\"]}: {attr[\"value\"]}')
"

echo ""
echo "=== List contracts ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1
echo "---"
"${JUNOD}" query wasm list-contract-by-code 2 2>&1
