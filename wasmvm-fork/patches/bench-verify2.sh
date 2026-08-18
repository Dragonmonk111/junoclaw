#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"

# Check chain alive
HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try: print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])
except: print('0')
" 2>/dev/null || echo "0")
echo "Chain height: ${HEIGHT}"

if [[ "${HEIGHT}" == "0" ]]; then
  echo "Chain dead. Need wsl --shutdown + rerun all-in-one."
  exit 1
fi

# List codes
echo ""
echo "=== Codes ==="
"${JUNOD}" query wasm list-code 2>&1

# List contracts
echo ""
echo "=== Contracts code 1 ==="
"${JUNOD}" query wasm list-contract-by-code 1 2>&1
echo ""
echo "=== Contracts code 2 ==="
"${JUNOD}" query wasm list-contract-by-code 2 2>&1

# Get contract addresses
CONTRACT_PRE="juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8"
CONTRACT_PURE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"

# Run VerifyProof on precompile
echo ""
echo "=== VerifyProof on PRECOMPILE contract ==="
echo "Contract: ${CONTRACT_PRE}"
EXEC1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
  '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
echo "Raw output:"
echo "${EXEC1}"

sleep 8

# Extract hash from raw output
HASH1=$(echo "${EXEC1}" | grep -oP '"txhash"\s*:\s*"[^"]+"' | head -1 | grep -oP '[a-f0-9]{64}')
echo "Hash: ${HASH1}"

if [[ -n "${HASH1}" ]]; then
  echo "Querying tx..."
  "${JUNOD}" query tx "${HASH1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'gas_used: {d.get(\"gas_used\",\"?\")}')
print(f'gas_wanted: {d.get(\"gas_wanted\",\"?\")}')
print(f'code: {d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'raw_log: {log[:400]}')
"
fi

# Run VerifyProof on pure
echo ""
echo "=== VerifyProof on PURE contract ==="
echo "Contract: ${CONTRACT_PURE}"
EXEC2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
echo "Raw output:"
echo "${EXEC2}"

sleep 8

HASH2=$(echo "${EXEC2}" | grep -oP '"txhash"\s*:\s*"[^"]+"' | head -1 | grep -oP '[a-f0-9]{64}')
echo "Hash: ${HASH2}"

if [[ -n "${HASH2}" ]]; then
  echo "Querying tx..."
  "${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'gas_used: {d.get(\"gas_used\",\"?\")}')
print(f'gas_wanted: {d.get(\"gas_wanted\",\"?\")}')
print(f'code: {d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'raw_log: {log[:400]}')
"
fi

echo ""
echo "=== DONE ==="
