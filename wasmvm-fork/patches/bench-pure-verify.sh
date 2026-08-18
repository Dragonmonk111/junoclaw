#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"
CONTRACT_PURE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"

echo "=== VerifyProof on PURE contract ==="
EXEC=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
echo "${EXEC}" | tail -5

HASH=$(echo "${EXEC}" | grep -oE '[a-f0-9]{64}' | head -1)
echo "Hash: ${HASH}"

echo "Waiting for inclusion..."
for i in $(seq 1 15); do
  sleep 3
  GAS=$("${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS}" != "0" ]]; then
    echo "Tx included!"
    "${JUNOD}" query tx "${HASH}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'PURE_VERIFY_GAS={d.get(\"gas_used\",\"?\")}')
print(f'PURE_VERIFY_CODE={d.get(\"code\",\"?\")}')
log = str(d.get('raw_log',''))
print(f'PURE_VERIFY_LOG={log[:400]}')
"
    break
  fi
  echo "  ...(${i})"
done

echo ""
echo "========================================"
echo "=== FINAL BENCHMARK RESULTS ==="
echo "========================================"
echo "Chain: junoclaw-bn254-local (Juno v30.0.0 + BN254 wasmvm v3.0.4)"
echo "wasmvm: 3.0.4 (17 BN254 symbols)"
echo ""
echo "Store gas (precompile, code 1): 2,956,765"
echo "Store gas (pure wasm, code 2):  3,802,965"
echo "Store gas reduction:            846,200 (1.29x smaller)"
echo ""
echo "Verify gas (precompile):        154,840 (code=5, msg parse error)"
echo "Verify gas (pure wasm):         see PURE_VERIFY_GAS above"
echo "========================================"
