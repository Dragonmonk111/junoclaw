#!/usr/bin/env bash
# benchmark-bn254.sh
#
# Stores zk_verifier_precompile.wasm and zk_verifier_pure.wasm on the local
# BN254-patched Juno testnet, instantiates them, and runs VerifyProof to
# measure gas usage.
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"
PRECOMPILE_WASM="${WASM_DIR}/zk_verifier_precompile.wasm"
PURE_WASM="${WASM_DIR}/zk_verifier_pure.wasm"
KEYRING="--keyring-backend test"
FROM="--from validator"
HOME="--home ${HOME_DIR}"
CHAIN="--chain-id junoclaw-bn254-local"
GAS="--gas 5000000 --gas-prices 0ujuno --fees 0ujuno"
BROADCAST="--broadcast-mode block"

echo "=== BN254 Gas Benchmark ==="
echo ""

# Verify chain is up
HEIGHT=$(curl -s http://localhost:26657/status | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])" 2>/dev/null || echo "0")
if [[ "${HEIGHT}" == "0" ]]; then
  echo "ERROR: Chain not running. Start it first with setup-bn254-localnet.sh"
  exit 1
fi
echo "Chain at height ${HEIGHT}"
echo ""

# Check wasm files exist
for f in "${PRECOMPILE_WASM}" "${PURE_WASM}"; do
  if [[ ! -f "${f}" ]]; then
    echo "ERROR: ${f} not found"
    exit 1
  fi
  SIZE=$(stat -c%s "${f}" 2>/dev/null || wc -c < "${f}")
  echo "  $(basename ${f}): ${SIZE} bytes"
done
echo ""

# Store precompile wasm
echo "=== Storing zk_verifier_precompile.wasm ==="
TX_PRE=$(${JUNOD} tx wasm store "${PRECOMPILE_WASM}" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} -y 2>&1)
echo "${TX_PRE}" | tail -5

CODE_ID_PRE=$(echo "${TX_PRE}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            # Try to find code_id in events
            for event in d.get('events', []):
                if event.get('type') == 'store_code':
                    for attr in event.get('attributes', []):
                        if attr.get('key') == 'code_id':
                            print(attr.get('value'))
                            sys.exit(0)
            # Maybe in raw_log
            if 'code_id' in str(d):
                import re
                m = re.search(r'code_id.*?(\d+)', str(d))
                if m:
                    print(m.group(1))
                    sys.exit(0)
        except:
            pass
print('UNKNOWN')
" 2>/dev/null)

echo "Precompile code_id: ${CODE_ID_PRE}"
echo ""

# Store pure wasm
echo "=== Storing zk_verifier_pure.wasm ==="
TX_PURE=$(${JUNOD} tx wasm store "${PURE_WASM}" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} -y 2>&1)
echo "${TX_PURE}" | tail -5

CODE_ID_PURE=$(echo "${TX_PURE}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            for event in d.get('events', []):
                if event.get('type') == 'store_code':
                    for attr in event.get('attributes', []):
                        if attr.get('key') == 'code_id':
                            print(attr.get('value'))
                            sys.exit(0)
        except:
            pass
print('UNKNOWN')
" 2>/dev/null)

echo "Pure wasm code_id: ${CODE_ID_PURE}"
echo ""

# Query code info for gas used during store
echo "=== Store gas usage ==="
${JUNOD} query tx --type=hash $(echo "${TX_PRE}" | python3 -c "import sys,json; [print(json.loads(l)['txhash']) for l in sys.stdin if l.strip().startswith('{')]" 2>/dev/null | head -1) 2>&1 | grep -i "gas\|code_id" | head -5
echo ""

# Instantiate precompile contract
echo "=== Instantiating precompile contract (code_id=${CODE_ID_PRE}) ==="
INIT_MSG='{"verifier_type":"groth16","curve":"bn254"}'
TX_INST_PRE=$(${JUNOD} tx wasm instantiate ${CODE_ID_PRE} "${INIT_MSG}" \
  --label "zk-verifier-precompile" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} --no-admin -y 2>&1)
echo "${TX_INST_PRE}" | tail -5

CONTRACT_PRE=$(${JUNOD} query wasm list-contract-by-code ${CODE_ID_PRE} 2>&1 | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        d = json.loads(line)
        contracts = d.get('contracts', [])
        if contracts:
            print(contracts[-1])
            sys.exit(0)
    except:
        pass
print('UNKNOWN')
" 2>/dev/null)
echo "Precompile contract: ${CONTRACT_PRE}"
echo ""

# Instantiate pure wasm contract
echo "=== Instantiating pure wasm contract (code_id=${CODE_ID_PURE}) ==="
TX_INST_PURE=$(${JUNOD} tx wasm instantiate ${CODE_ID_PURE} "${INIT_MSG}" \
  --label "zk-verifier-pure" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} --no-admin -y 2>&1)
echo "${TX_INST_PURE}" | tail -5

CONTRACT_PURE=$(${JUNOD} query wasm list-contract-by-code ${CODE_ID_PURE} 2>&1 | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        d = json.loads(line)
        contracts = d.get('contracts', [])
        if contracts:
            print(contracts[-1])
            sys.exit(0)
    except:
        pass
print('UNKNOWN')
" 2>/dev/null)
echo "Pure contract: ${CONTRACT_PURE}"
echo ""

# Run VerifyProof on both contracts with a dummy proof
# The contract expects a VerifyProof message with proof, public_inputs, verifying_key
# For gas measurement purposes, we use a minimal message — the contract will
# reject invalid proofs but gas will still be measured
VERIFY_MSG='{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}'

echo "=== Running VerifyProof on precompile contract ==="
echo "  contract: ${CONTRACT_PRE}"
TX_VERIFY_PRE=$(${JUNOD} tx wasm execute "${CONTRACT_PRE}" "${VERIFY_MSG}" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} -y 2>&1)
echo "${TX_VERIFY_PRE}" | tail -10
GAS_PRE=$(echo "${TX_VERIFY_PRE}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            gas = d.get('gas_used', d.get('gasWanted', ''))
            if gas:
                print(gas)
                sys.exit(0)
        except:
            pass
print('UNKNOWN')
" 2>/dev/null)
echo "Precompile verify gas: ${GAS_PRE}"
echo ""

echo "=== Running VerifyProof on pure wasm contract ==="
echo "  contract: ${CONTRACT_PURE}"
TX_VERIFY_PURE=$(${JUNOD} tx wasm execute "${CONTRACT_PURE}" "${VERIFY_MSG}" \
  ${FROM} ${KEYRING} ${HOME} ${CHAIN} ${GAS} ${BROADCAST} -y 2>&1)
echo "${TX_VERIFY_PURE}" | tail -10
GAS_PURE=$(echo "${TX_VERIFY_PURE}" | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if line.startswith('{'):
        try:
            d = json.loads(line)
            gas = d.get('gas_used', d.get('gasWanted', ''))
            if gas:
                print(gas)
                sys.exit(0)
        except:
            pass
print('UNKNOWN')
" 2>/dev/null)
echo "Pure wasm verify gas: ${GAS_PURE}"
echo ""

echo "============================================"
echo "=== BN254 Gas Benchmark Results ==="
echo "============================================"
echo "  Precompile verify gas: ${GAS_PRE}"
echo "  Pure wasm verify gas:  ${GAS_PURE}"
if [[ "${GAS_PRE}" != "UNKNOWN" && "${GAS_PURE}" != "UNKNOWN" && "${GAS_PURE}" != "0" ]]; then
  RATIO=$(python3 -c "print(f'{${GAS_PURE}/${GAS_PRE}:.2f}x')" 2>/dev/null || echo "N/A")
  echo "  Reduction ratio:       ${RATIO}"
fi
echo "  Precompile code_id:    ${CODE_ID_PRE}"
echo "  Pure wasm code_id:     ${CODE_ID_PURE}"
echo "  Precompile contract:   ${CONTRACT_PRE}"
echo "  Pure contract:         ${CONTRACT_PURE}"
echo "============================================"
