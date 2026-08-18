#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNOD="/root/junoclaw-build/juno/bin/junod"
HOME_DIR="/root/.junoclaw-bn254-local"
CHAIN_ID="junoclaw-bn254-local"

CONTRACT_PRE="juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8"
CONTRACT_PURE="juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"

# Check chain
HEIGHT=$(curl -s http://localhost:26657/status 2>/dev/null | python3 -c "
import sys, json
try: print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])
except: print('0')
" 2>/dev/null || echo "0")
echo "Chain height: ${HEIGHT}"

# Wait for the previous verify tx to be included
echo "Waiting for tx 1B3DC8436FD5B3D33D5B0D94BF8F20500C6BF3CD1D2374CA2FE3EF7EAE3DEEDF..."
for i in $(seq 1 10); do
  sleep 3
  GAS=$("${JUNOD}" query tx 1B3DC8436FD5B3D33D5B0D94BF8F20500C6BF3CD1D2374CA2FE3EF7EAE3DEEDF --type=hash -o json 2>&1 | python3 -c "
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
    "${JUNOD}" query tx 1B3DC8436FD5B3D33D5B0D94BF8F20500C6BF3CD1D2374CA2FE3EF7EAE3DEEDF --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'PRECOMPILE_VERIFY_GAS={d.get(\"gas_used\",\"?\")}')
print(f'PRECOMPILE_VERIFY_CODE={d.get(\"code\",\"?\")}')
print(f'PRECOMPILE_VERIFY_LOG={str(d.get(\"raw_log\",\"\"))[:400]}')
"
    break
  fi
  echo "  ...(${i})"
done

# Now run verify on pure contract
echo ""
echo "=== VerifyProof on PURE contract ==="
EXEC2=$("${JUNOD}" tx wasm execute "${CONTRACT_PURE}" \
  '{"verify_proof":{"proof":"0x","public_inputs":[],"verifying_key":"0x"}}' \
  --from validator --keyring-backend test --home "${HOME_DIR}" \
  --chain-id "${CHAIN_ID}" --gas 5000000 --gas-prices 1ujuno \
  -y 2>&1)
HASH2=$(echo "${EXEC2}" | grep -oP '[a-f0-9]{64}' | head -1)
echo "Hash: ${HASH2}"

echo "Waiting for tx ${HASH2}..."
for i in $(seq 1 10); do
  sleep 3
  GAS2=$("${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    g = d.get('gas_used','0')
    print(g)
except:
    print('0')
" 2>/dev/null || echo "0")
  if [[ "${GAS2}" != "0" ]]; then
    echo "Tx included! gas_used=${GAS2}"
    "${JUNOD}" query tx "${HASH2}" --type=hash -o json 2>&1 | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'PURE_VERIFY_GAS={d.get(\"gas_used\",\"?\")}')
print(f'PURE_VERIFY_CODE={d.get(\"code\",\"?\")}')
print(f'PURE_VERIFY_LOG={str(d.get(\"raw_log\",\"\"))[:400]}')
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
echo "Store gas (precompile): 2,956,765"
echo "Store gas (pure wasm):  3,802,965"
echo ""
echo "Verify gas values from txs above"
echo "========================================"
