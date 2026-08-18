#!/usr/bin/env bash
# all-in-one-bench.sh — init, start, store, instantiate, verify, report
# Runs everything in one shot to beat WSL2 clock jumps
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"

pkill -f "junod start" 2>/dev/null || true
sleep 2
rm -rf "${HOME_DIR}"

echo "=== 1. INIT ==="
"${JUNOD}" init bn254-validator --chain-id "${CHAIN_ID}" --home "${HOME_DIR}" 2>&1 | tail -1
"${JUNOD}" keys add validator --keyring-backend test --home "${HOME_DIR}" 2>&1 | grep address || true
VALIDATOR_ADDR=$("${JUNOD}" keys show validator -a --keyring-backend test --home "${HOME_DIR}")

# Fix all denoms
python3 << 'PYEOF'
import json
p = "/root/.junoclaw-bn254-local/config/genesis.json"
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
if 'feemarket' in g['app_state']:
    g['app_state']['feemarket']['params']['fee_denom'] = 'ujuno'
with open(p, 'w') as f:
    json.dump(g, f, indent=2)
print("denoms fixed")
PYEOF

"${JUNOD}" genesis add-genesis-account "${VALIDATOR_ADDR}" 1000000000000ujuno --home "${HOME_DIR}"
"${JUNOD}" genesis gentx validator 1000000000ujuno --chain-id "${CHAIN_ID}" --keyring-backend test --home "${HOME_DIR}" 2>&1 | tail -1
"${JUNOD}" genesis collect-gentxs --home "${HOME_DIR}" 2>&1 | tail -1

echo ""
echo "=== 2. START ==="
"${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
JUNOD_PID=$!

# Wait for blocks
for i in $(seq 1 20); do
  sleep 3
  HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try:
    print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${HEIGHT}" != "0" ]]; then
    echo "Chain at height ${HEIGHT}"
    break
  fi
done

if [[ "${HEIGHT}" == "0" ]]; then
  echo "FATAL: No blocks"
  exit 1
fi

echo ""
echo "=== 3. STORE PRECOMPILE WASM ==="
STORE1=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
sleep 8

# Get store1 hash and query
HASH1=$(echo "${STORE1}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)

if [[ -n "${HASH1}" ]]; then
  "${JUNOD}" query tx "${HASH1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
gas = d.get('gas_used','?')
print(f'STORE_PRECOMPILE_GAS={gas}')
print(f'STORE_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
for e in d.get('events',[]):
    if e.get('type') == 'store_code':
        for a in e.get('attributes',[]):
            if a.get('key') == 'code_id':
                print(f'PRECOMPILE_CODE_ID={a.get(\"value\")}')
"
fi

echo ""
echo "=== 4. STORE PURE WASM ==="
STORE2=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
sleep 8

HASH2=$(echo "${STORE2}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)

if [[ -n "${HASH2}" ]]; then
  "${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
gas = d.get('gas_used','?')
print(f'STORE_PURE_GAS={gas}')
print(f'STORE_PURE_CODE={d.get(\"code\",\"?\")}')
for e in d.get('events',[]):
    if e.get('type') == 'store_code':
        for a in e.get('attributes',[]):
            if a.get('key') == 'code_id':
                print(f'PURE_CODE_ID={a.get(\"value\")}')
"
fi

echo ""
echo "=== 5. INSTANTIATE PRECOMPILE (code 1) ==="
INST1=$("${JUNOD}" tx wasm instantiate 1 "{}" \
  --label "zk-pre" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --no-admin -y 2>&1)
sleep 8

HASHI1=$(echo "${INST1}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)

if [[ -n "${HASHI1}" ]]; then
  "${JUNOD}" query tx "${HASHI1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'INST_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'INST_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'INST_PRECOMPILE_LOG={log[:200]}')
for e in d.get('events',[]):
    if e.get('type') == 'instantiate_contract':
        for a in e.get('attributes',[]):
            if a.get('key') == '_contract_address':
                print(f'PRECOMPILE_CONTRACT={a.get(\"value\")}')
"
fi

echo ""
echo "=== 6. INSTANTIATE PURE (code 2) ==="
INST2=$("${JUNOD}" tx wasm instantiate 2 "{}" \
  --label "zk-pure" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --no-admin -y 2>&1)
sleep 8

HASHI2=$(echo "${INST2}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)

if [[ -n "${HASHI2}" ]]; then
  "${JUNOD}" query tx "${HASHI2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'INST_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'INST_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'INST_PURE_LOG={log[:200]}')
for e in d.get('events',[]):
    if e.get('type') == 'instantiate_contract':
        for a in e.get('attributes',[]):
            if a.get('key') == '_contract_address':
                print(f'PURE_CONTRACT={a.get(\"value\")}')
"
fi

echo ""
echo "=== 7. VERIFY PROOF ON PRECOMPILE ==="
# Get contract address
CONTRACT_PRE=$("${JUNOD}" query wasm list-contract-by-code 1 -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    c = d.get('contracts', [])
    print(c[-1] if c else '')
except:
    print('')
" 2>/dev/null)

if [[ -n "${CONTRACT_PRE}" ]]; then
  echo "Contract: ${CONTRACT_PRE}"
  EXEC1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
    '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
    --from validator --keyring-backend test --home "${HOME_DIR}" \
    --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
    -y 2>&1)
  sleep 8
  
  HASHE1=$(echo "${EXEC1}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)
  
  if [[ -n "${HASHE1}" ]]; then
    "${JUNOD}" query tx "${HASHE1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PRECOMPILE_LOG={log[:300]}')
"
  fi
else
  echo "No precompile contract found"
fi

echo ""
echo "=== 8. VERIFY PROOF ON PURE ==="
CONTRACT_PURE=$("${JUNOD}" query wasm list-contract-by-code 2 -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    c = d.get('contracts', [])
    print(c[-1] if c else '')
except:
    print('')
" 2>/dev/null)

if [[ -n "${CONTRACT_PURE}" ]]; then
  echo "Contract: ${CONTRACT_PURE}"
  EXEC2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
    '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
    --from validator --keyring-backend test --home "${HOME_DIR}" \
    --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
    -y 2>&1)
  sleep 8
  
  HASHE2=$(echo "${EXEC2}" | python3 -c "
import sys, json
for l in sys.stdin:
    l = l.strip()
    if l.startswith('{'):
        try:
            print(json.loads(l).get('txhash',''))
            break
        except: pass
" 2>/dev/null)
  
  if [[ -n "${HASHE2}" ]]; then
    "${JUNOD}" query tx "${HASHE2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PURE_LOG={log[:300]}')
"
  fi
else
  echo "No pure contract found"
fi

echo ""
echo "=== 9. WASMVM VERSION ==="
"${JUNOD}" query wasm libwasmvm-version 2>&1

echo ""
echo "========================================"
echo "=== BENCHMARK COMPLETE ==="
echo "========================================"
echo "Chain: ${CHAIN_ID} (Juno v30.0.0 + BN254 wasmvm v3.0.4)"
echo "PID: ${JUNOD_PID}"
