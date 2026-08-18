#!/usr/bin/env bash
# bench-verify-correct.sh — Store VK + VerifyProof with correct message schema
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"
PROOF_JSON="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/tmpdir/groth16_proof.json"

# Check chain
HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try: print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])
except: print('0')
" 2>/dev/null || echo "0")
echo "Chain height: ${HEIGHT}"

if [[ "${HEIGHT}" == "0" ]]; then
  echo "Chain dead. Restarting..."
  pkill -f "junod start" 2>/dev/null || true
  sleep 2
  "${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
  for i in $(seq 1 15); do
    sleep 3
    HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try: print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])
except: print('0')
" 2>/dev/null || echo "0")
    if [[ "${HEIGHT}" != "0" ]]; then
      echo "Chain at height ${HEIGHT}"
      break
    fi
  done
  if [[ "${HEIGHT}" == "0" ]]; then
    echo "FATAL: Chain not producing blocks. Need wsl --shutdown."
    exit 1
  fi
fi

# Extract proof data from JSON
VK_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['vk_base64'])")
PROOF_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['proof_base64'])")
INPUTS_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['public_inputs_base64'])")

echo "VK length: ${#VK_B64} chars"
echo "Proof length: ${#PROOF_B64} chars"
echo "Inputs length: ${#INPUTS_B64} chars"

# Get contract addresses
CONTRACT_PRE="juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8"
CONTRACT_PURE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"

# Verify contracts still exist
echo ""
echo "=== Contract check ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1 | head -5
echo ""
"${JUNOD}" query wasm list-contract-by-code 2 2>&1 | head -5

# ── Step 1: Store VK on precompile contract ──
echo ""
echo "=== Store VK on PRECOMPILE contract ==="
STORE_VK1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
  "{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 2000000 --gas-prices 1ujuno \
  -y 2>&1)
HASH_VK1=$(echo "${STORE_VK1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "txhash: ${HASH_VK1}"

echo "Waiting for inclusion..."
for i in $(seq 1 15); do
  sleep 3
  GAS=$("${JUNOD}" query tx "${HASH_VK1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS}" != "0" ]]; then
    echo "Tx included! gas_used=${GAS}"
    "${JUNOD}" query tx "${HASH_VK1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_VK_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_VK_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'STORE_VK_PRECOMPILE_LOG={log[:300]}')
"
    break
  fi
  echo "  ...(${i})"
done

# ── Step 2: Store VK on pure contract ──
echo ""
echo "=== Store VK on PURE contract ==="
STORE_VK2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  "{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 2000000 --gas-prices 1ujuno \
  -y 2>&1)
HASH_VK2=$(echo "${STORE_VK2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "txhash: ${HASH_VK2}"

for i in $(seq 1 15); do
  sleep 3
  GAS=$("${JUNOD}" query tx "${HASH_VK2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS}" != "0" ]]; then
    echo "Tx included! gas_used=${GAS}"
    "${JUNOD}" query tx "${HASH_VK2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_VK_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_VK_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'STORE_VK_PURE_LOG={log[:300]}')
"
    break
  fi
  echo "  ...(${i})"
done

# ── Step 3: VerifyProof on precompile contract ──
echo ""
echo "=== VerifyProof on PRECOMPILE contract ==="
EXEC1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
  "{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
HASH1=$(echo "${EXEC1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "txhash: ${HASH1}"

for i in $(seq 1 15); do
  sleep 3
  GAS=$("${JUNOD}" query tx "${HASH1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS}" != "0" ]]; then
    echo "Tx included! gas_used=${GAS}"
    "${JUNOD}" query tx "${HASH1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PRECOMPILE_LOG={log[:400]}')
"
    break
  fi
  echo "  ...(${i})"
done

# ── Step 4: VerifyProof on pure contract ──
echo ""
echo "=== VerifyProof on PURE contract ==="
EXEC2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  "{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
HASH2=$(echo "${EXEC2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "txhash: ${HASH2}"

for i in $(seq 1 15); do
  sleep 3
  GAS=$("${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS}" != "0" ]]; then
    echo "Tx included! gas_used=${GAS}"
    "${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PURE_LOG={log[:400]}')
"
    break
  fi
  echo "  ...(${i})"
done

echo ""
echo "========================================"
echo "=== BENCHMARK COMPLETE ==="
echo "========================================"
echo "Chain: ${CHAIN_ID} (Juno v30.0.0 + BN254 wasmvm v3.0.4)"
