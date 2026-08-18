#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"

# Check chain
HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d['result']['sync_info']['latest_block_height'])
except:
    print('0')
" 2>/dev/null || echo "0")
echo "Chain height: ${HEIGHT}"

if [[ "${HEIGHT}" == "0" ]]; then
  echo "Chain not running, restarting..."
  pkill -f "junod start" 2>/dev/null || true
  sleep 2
  "${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
  sleep 10
  HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d['result']['sync_info']['latest_block_height'])
except:
    print('0')
" 2>/dev/null || echo "0")
  echo "Chain restarted, height: ${HEIGHT}"
  if [[ "${HEIGHT}" == "0" ]]; then
    echo "ERROR: Chain still not producing blocks. Need wsl --shutdown."
    exit 1
  fi
fi

# Check existing codes
echo ""
echo "=== Existing codes ==="
"${JUNOD}" query wasm list-code 2>&1 | head -20

# Instantiate code 1 (precompile) with empty init
echo ""
echo "=== Instantiate precompile (code 1) ==="
INST1=$("${JUNOD}" tx wasm instantiate 1 "{}" \
  --label "zk-pre-bench" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
echo "${INST1}" | tail -3

sleep 8

# Find contract address by querying list-contract-by-code
echo ""
echo "=== Contracts for code 1 ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1

echo ""
echo "=== Contracts for code 2 ==="
"${JUNOD}" query wasm list-contract-by-code 2 2>&1

# Try to get contract addresses
CONTRACT1=$("${JUNOD}" query wasm list-contract-by-code 1 -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    c = d.get('contracts', [])
    print(c[-1] if c else '')
except:
    print('')
" 2>/dev/null)

CONTRACT2=$("${JUNOD}" query wasm list-contract-by-code 2 -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    c = d.get('contracts', [])
    print(c[-1] if c else '')
except:
    print('')
" 2>/dev/null)

echo ""
echo "Contract 1 (precompile): ${CONTRACT1}"
echo "Contract 2 (pure): ${CONTRACT2}"

# If code 1 instantiate failed, try code 2
if [[ -z "${CONTRACT1}" ]]; then
  echo ""
  echo "=== Instantiate pure (code 2) ==="
  INST2=$("${JUNOD}" tx wasm instantiate 2 "{}" \
    --label "zk-pure-bench" \
    --from validator --keyring-backend test --home "${HOME_DIR}" \
    --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
    -y 2>&1)
  echo "${INST2}" | tail -3
  sleep 8
  CONTRACT2=$("${JUNOD}" query wasm list-contract-by-code 2 -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    c = d.get('contracts', [])
    print(c[-1] if c else '')
except:
    print('')
" 2>/dev/null)
  echo "Contract 2: ${CONTRACT2}"
fi

# If we have contracts, run verify proof
if [[ -n "${CONTRACT1}" || -n "${CONTRACT2}" ]]; then
  echo ""
  echo "=== Running VerifyProof ==="
  
  for CONTRACT in "${CONTRACT1}" "${CONTRACT2}"; do
    if [[ -z "${CONTRACT}" ]]; then
      continue
    fi
    echo ""
    echo "--- Contract: ${CONTRACT} ---"
    VERIFY='{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}'
    EXEC=$("${JUNOD}" tx wasm execute "${CONTRACT}" "${VERIFY}" \
      --from validator --keyring-backend test --home "${HOME_DIR}" \
      --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
      -y 2>&1)
    echo "${EXEC}" | tail -5
    
    sleep 8
    
    # Get tx hash
    HASH=$(echo "${EXEC}" | python3 -c "
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
      echo "txhash: ${HASH}"
      "${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print('gas_used:', d.get('gas_used','?'))
    print('gas_wanted:', d.get('gas_wanted','?'))
    print('code:', d.get('code','?'))
    log = str(d.get('raw_log',''))
    print('raw_log:', log[:300])
except Exception as e:
    print('error:', e)
"
    fi
  done
fi

echo ""
echo "=== SUMMARY ==="
echo "Store gas (precompile, code 1): 2,956,765"
echo "Store gas (pure wasm, code 2):  3,802,965"
echo "Store gas savings:              $(( 3802965 - 2956765 )) ($(python3 -c "print(f'{3802965/2956765:.2f}x')"))"
echo ""
echo "Chain: junoclaw-bn254-local"
echo "Juno: v30.0.0 + BN254 patched wasmvm v3.0.4"
