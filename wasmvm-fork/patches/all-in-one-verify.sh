#!/usr/bin/env bash
# all-in-one-verify.sh — Full benchmark: init, store, instantiate, store_vk, verify_proof
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"
WASM_DIR="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet"
PROOF_JSON="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/tmpdir/groth16_proof.json"

pkill -f "junod start" 2>/dev/null || true
sleep 2
rm -rf "${HOME_DIR}"

# ── 1. INIT ──
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

# ── 2. START ──
echo ""
echo "=== 2. START ==="
"${JUNOD}" start --home "${HOME_DIR}" --log_level error --minimum-gas-prices 0ujuno &
JUNOD_PID=$!

for i in $(seq 1 20); do
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
  echo "FATAL: No blocks"
  exit 1
fi

# ── 3. STORE BOTH WASMS ──
echo ""
echo "=== 3. STORE PRECOMPILE WASM ==="
S1=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_precompile.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
H1=$(echo "${S1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "store tx: ${H1}"
"${JUNOD}" query tx "${H1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
"

echo ""
echo "=== 4. STORE PURE WASM ==="
S2=$("${JUNOD}" tx wasm store "${WASM_DIR}/zk_verifier_pure.wasm" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
H2=$(echo "${S2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "store tx: ${H2}"
"${JUNOD}" query tx "${H2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_PURE_CODE={d.get(\"code\",\"?\")}')
"

# ── 5. INSTANTIATE BOTH ──
echo ""
echo "=== 5. INSTANTIATE PRECOMPILE (code 1) ==="
I1=$("${JUNOD}" tx wasm instantiate 1 "{}" \
  --label "zk-pre" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --no-admin -y 2>&1)
sleep 10
HI1=$(echo "${I1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "instantiate tx: ${HI1}"
"${JUNOD}" query tx "${HI1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'INST_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'INST_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
"

echo ""
echo "=== 6. INSTANTIATE PURE (code 2) ==="
I2=$("${JUNOD}" tx wasm instantiate 2 "{}" \
  --label "zk-pure" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --no-admin -y 2>&1)
sleep 10
HI2=$(echo "${I2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "instantiate tx: ${HI2}"
"${JUNOD}" query tx "${HI2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'INST_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'INST_PURE_CODE={d.get(\"code\",\"?\")}')
"

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

# ── 7. Extract proof data ──
VK_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['vk_base64'])")
PROOF_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['proof_base64'])")
INPUTS_B64=$(python3 -c "import json; d=json.load(open('${PROOF_JSON}')); print(d['public_inputs_base64'])")

# ── 8. Store VK on precompile ──
echo ""
echo "=== 7. STORE VK on PRECOMPILE ==="
VK1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
  "{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 2000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
HK1=$(echo "${VK1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "store_vk tx: ${HK1}"
"${JUNOD}" query tx "${HK1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_VK_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_VK_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'STORE_VK_PRECOMPILE_LOG={log[:300]}')
"

# ── 9. Store VK on pure ──
echo ""
echo "=== 8. STORE VK on PURE ==="
VK2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  "{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 2000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
HK2=$(echo "${VK2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "store_vk tx: ${HK2}"
"${JUNOD}" query tx "${HK2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'STORE_VK_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'STORE_VK_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'STORE_VK_PURE_LOG={log[:300]}')
"

# ── 10. VerifyProof on precompile ──
echo ""
echo "=== 9. VERIFY PROOF on PRECOMPILE ==="
VP1=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" \
  "{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
HP1=$(echo "${VP1}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "verify tx: ${HP1}"
"${JUNOD}" query tx "${HP1}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PRECOMPILE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PRECOMPILE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PRECOMPILE_LOG={log[:400]}')
"

# ── 11. VerifyProof on pure ──
echo ""
echo "=== 10. VERIFY PROOF on PURE ==="
VP2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  "{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno -y 2>&1)
sleep 10
HP2=$(echo "${VP2}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "verify tx: ${HP2}"
"${JUNOD}" query tx "${HP2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'VERIFY_PURE_GAS={d.get(\"gas_used\",\"?\")}')
print(f'VERIFY_PURE_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'VERIFY_PURE_LOG={log[:400]}')
"

# ── 12. wasmvm version ──
echo ""
echo "=== 11. WASMVM VERSION ==="
"${JUNOD}" query wasm libwasmvm-version 2>&1

echo ""
echo "========================================"
echo "=== FULL BENCHMARK COMPLETE ==="
echo "========================================"
echo "Chain: ${CHAIN_ID} (Juno v30.0.0 + BN254 wasmvm v3.0.4)"
echo "PID: ${JUNOD_PID}"
