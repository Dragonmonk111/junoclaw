#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
CHAIN_ID="junoclaw-bn254-local"
HOME_DIR="/root/.junoclaw-bn254-local"

HEIGHT=$(curl -s http://localhost:26657/status | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d['result']['sync_info']['latest_block_height'])
" 2>/dev/null || echo "0")
echo "Chain at height ${HEIGHT}"

# Query store gas for both txs
echo ""
echo "=== Store gas: precompile (code_id=1) ==="
"${JUNOD}" query tx 6F70B5EF46979F4CFB0F594B412CBDD9BADCBAE1629C22210422DAE9BE1721EC --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('gas_used:', d.get('gas_used','?'))
print('gas_wanted:', d.get('gas_wanted','?'))
print('code:', d.get('code','?'))
"

echo ""
echo "=== Store gas: pure wasm (code_id=2) ==="
"${JUNOD}" query tx 8F8CBA5DAF0D86BD876629C307A71DA73D416249B1E81CDB739E6E8D1FAF5E29 --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('gas_used:', d.get('gas_used','?'))
print('gas_wanted:', d.get('gas_wanted','?'))
print('code:', d.get('code','?'))
"

# Instantiate precompile contract
echo ""
echo "=== Instantiating precompile contract (code_id=1) ==="
INIT='{"verifier_type":"groth16","curve":"bn254"}'
INST_OUT=$("${JUNOD}" tx wasm instantiate 1 "${INIT}" \
  --label "zk-precompile" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync --no-admin -y 2>&1)
echo "${INST_OUT}" | tail -3

sleep 7

# Get contract address
CONTRACT_PRE=$("${JUNOD}" query wasm list-contract-by-code 1 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
contracts = d.get('contracts', [])
print(contracts[-1] if contracts else 'NONE')
" 2>/dev/null)
echo "Precompile contract: ${CONTRACT_PRE}"

# Instantiate pure contract
echo ""
echo "=== Instantiating pure wasm contract (code_id=2) ==="
INST_OUT2=$("${JUNOD}" tx wasm instantiate 2 "${INIT}" \
  --label "zk-pure" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync --no-admin -y 2>&1)
echo "${INST_OUT2}" | tail -3

sleep 7

CONTRACT_PURE=$("${JUNOD}" query wasm list-contract-by-code 2 -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
contracts = d.get('contracts', [])
print(contracts[-1] if contracts else 'NONE')
" 2>/dev/null)
echo "Pure contract: ${CONTRACT_PURE}"

# Run VerifyProof on both
echo ""
echo "=== VerifyProof on precompile contract ==="
VERIFY='{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}'
EXEC_OUT=$("${JUNOD}" tx wasm execute "${CONTRACT_PRE}" "${VERIFY}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${EXEC_OUT}" | tail -5

sleep 7

# Get tx hash and query gas
EXEC_HASH=$(echo "${EXEC_OUT}" | python3 -c "
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

if [[ -n "${EXEC_HASH}" ]]; then
  "${JUNOD}" query tx "${EXEC_HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('gas_used:', d.get('gas_used','?'))
print('gas_wanted:', d.get('gas_wanted','?'))
print('code:', d.get('code','?'))
print('raw_log:', str(d.get('raw_log','?'))[:300])
"
fi

echo ""
echo "=== VerifyProof on pure wasm contract ==="
EXEC_OUT2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" "${VERIFY}" \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  --broadcast-mode sync -y 2>&1)
echo "${EXEC_OUT2}" | tail -5

sleep 7

EXEC_HASH2=$(echo "${EXEC_OUT2}" | python3 -c "
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

if [[ -n "${EXEC_HASH2}" ]]; then
  "${JUNOD}" query tx "${EXEC_HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('gas_used:', d.get('gas_used','?'))
print('gas_wanted:', d.get('gas_wanted','?'))
print('code:', d.get('code','?'))
print('raw_log:', str(d.get('raw_log','?'))[:300])
"
fi

echo ""
echo "=== BENCHMARK SUMMARY ==="
echo "Chain: junoclaw-bn254-local (Juno v30.0.0 + BN254 patched wasmvm v3.0.4)"
echo "wasmvm: 3.0.4 (17 BN254 symbols)"
echo "Precompile code_id: 1, contract: ${CONTRACT_PRE}"
echo "Pure wasm code_id: 2, contract: ${CONTRACT_PURE}"
