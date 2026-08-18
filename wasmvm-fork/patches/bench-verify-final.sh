#!/usr/bin/env bash
# bench-verify-final.sh — Robust full benchmark with correct msg schema
# Uses longer sleeps and retry logic to handle WSL2 timing
set -uo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"
PROOF_JSON="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/tmpdir/groth16_proof.json"

pkill -f "junod start" 2>/dev/null || true
sleep 3
rm -rf "${HOME_DIR}"

echo "=== 1. INIT ==="
"${JUNOD}" init bn254-validator --chain-id "${CHAIN_ID}" --home "${HOME_DIR}" 2>&1 | tail -1
"${JUNOD}" keys add validator --keyring-backend test --home "${HOME_DIR}" 2>&1 | grep address || true
VALIDATOR_ADDR=$("${JUNOD}" keys show validator -a --keyring-backend test --home "${HOME_DIR}")

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

# Wait for blocks with longer timeout
HEIGHT=0
for i in $(seq 1 30); do
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
  echo "FATAL: No blocks after 90s"
  exit 1
fi

# Helper: submit tx, wait for inclusion, print gas
submit_and_wait() {
  local label="$1"
  local cmd="$2"
  local gas_limit="$3"
  
  echo ""
  echo "--- ${label} ---"
  local OUT
  OUT=$(eval timeout 30 "${cmd}" 2>&1)
  local HASH
  HASH=$(echo "${OUT}" | grep -oE '[a-f0-9]{64}' | head -1)
  
  if [[ -z "${HASH}" ]]; then
    echo "ERROR: No tx hash found. Output:"
    echo "${OUT}" | tail -10
    echo ""
    return 1
  fi
  echo "tx: ${HASH}"
  
  # Wait for inclusion
  for i in $(seq 1 20); do
    sleep 3
    local GAS
    GAS=$("${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    if g and g != '0':
        print(g)
    else:
        print('0')
except:
    print('0')
" 2>/dev/null || echo "0")
    if [[ "${GAS}" != "0" ]]; then
      echo "GAS_USED=${GAS}"
      "${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'  code={d.get(\"code\",\"?\")} gas_wanted={d.get(\"gas_wanted\",\"?\")}')
log = str(d.get('raw_log',''))
if log:
    print(f'  log={log[:300]}')
"
      return 0
    fi
    echo "  waiting... (${i})"
  done
  echo "ERROR: tx not found after 60s"
  return 1
}

# ── 3. STORE BOTH WASMS ──
echo ""
echo "=== 3. STORE WASMS ==="

submit_and_wait "STORE PRECOMPILE" \
  "\"${JUNOD}\" tx wasm store \"${WASM_DIR}/zk_verifier_precompile.wasm\" --from validator --keyring-backend test --home \"${HOME_DIR}\" --chain-id \"${CHAIN_ID}\" --gas 5000000 --gas-prices 1ujuno -y" \
  5000000

submit_and_wait "STORE PURE" \
  "\"${JUNOD}\" tx wasm store \"${WASM_DIR}/zk_verifier_pure.wasm\" --from validator --keyring-backend test --home \"${HOME_DIR}\" --chain-id \"${CHAIN_ID}\" --gas 5000000 --gas-prices 1ujuno -y" \
  5000000

# ── 4. INSTANTIATE ──
echo ""
echo "=== 4. INSTANTIATE ==="

submit_and_wait "INSTANTIATE PRECOMPILE (code 1)" \
  "\"${JUNOD}\" tx wasm instantiate 1 '{}' --label zk-pre --from validator --keyring-backend test --home \"${HOME_DIR}\" --chain-id \"${CHAIN_ID}\" --gas 5000000 --gas-prices 1ujuno --no-admin -y" \
  5000000

submit_and_wait "INSTANTIATE PURE (code 2)" \
  "\"${JUNOD}\" tx wasm instantiate 2 '{}' --label zk-pure --from validator --keyring-backend test --home \"${HOME_DIR}\" --chain-id \"${CHAIN_ID}\" --gas 5000000 --gas-prices 1ujuno --no-admin -y" \
  5000000

# Get contract addresses
CONTRACT_PRE=$("${JUNOD}" query wasm list-contract-by-code 1 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
c = d.get('contracts', [])
print(c[-1] if c else '')
" 2>/dev/null)
CONTRACT_PURE=$("${JUNOD}" query wasm list-contract-by-code 2 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
c = d.get('contracts', [])
print(c[-1] if c else '')
" 2>/dev/null)

echo ""
echo "Precompile contract: ${CONTRACT_PRE}"
echo "Pure contract: ${CONTRACT_PURE}"

if [[ -z "${CONTRACT_PRE}" || -z "${CONTRACT_PURE}" ]]; then
  echo "ERROR: Missing contract address"
  exit 1
fi

# ── 5. Extract proof data ──
VK_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['vk_base64'])")
PROOF_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['proof_base64'])")
INPUTS_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['public_inputs_base64'])")

# ── 6. Store VK on both contracts ──
echo ""
echo "=== 5. STORE VK ==="

submit_and_wait "STORE_VK PRECOMPILE" \
  "\"${JUNOD}\" tx wasm execute '${CONTRACT_PRE}' '{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}' --from validator --keyring-backend test --home '${HOME_DIR}' --chain-id '${CHAIN_ID}' --gas 2000000 --gas-prices 1ujuno -y" \
  2000000

submit_and_wait "STORE_VK PURE" \
  "\"${JUNOD}\" tx wasm execute '${CONTRACT_PURE}' '{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}' --from validator --keyring-backend test --home '${HOME_DIR}' --chain-id '${CHAIN_ID}' --gas 2000000 --gas-prices 1ujuno -y" \
  2000000

# ── 7. VerifyProof on both contracts ──
echo ""
echo "=== 6. VERIFY PROOF ==="

submit_and_wait "VERIFY PRECOMPILE" \
  "\"${JUNOD}\" tx wasm execute '${CONTRACT_PRE}' '{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}' --from validator --keyring-backend test --home '${HOME_DIR}' --chain-id '${CHAIN_ID}' --gas 5000000 --gas-prices 1ujuno -y" \
  5000000

submit_and_wait "VERIFY PURE" \
  "\"${JUNOD}\" tx wasm execute '${CONTRACT_PURE}' '{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}' --from validator --keyring-backend test --home '${HOME_DIR}' --chain-id '${CHAIN_ID}' --gas 5000000 --gas-prices 1ujuno -y" \
  5000000

# ── 8. wasmvm version ──
echo ""
echo "=== 7. WASMVM VERSION ==="
"${JUNOD}" query wasm libwasmvm-version 2>&1

echo ""
echo "========================================"
echo "=== FULL BENCHMARK COMPLETE ==="
echo "========================================"
echo "Chain: ${CHAIN_ID} (Juno v30.0.0 + BN254 wasmvm v3.0.4)"
echo "PID: ${JUNOD_PID}"
