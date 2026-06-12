#!/usr/bin/env bash
# Builds both zk-verifier variants (pure-Wasm + precompile) and uploads
# them to the running BN254 devnet. Captures code_ids + contract
# addresses into devnet/deploy.env for the benchmark script.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CHAIN_ID="${CHAIN_ID:-junoclaw-bn254-1}"
KEYRING="${KEYRING_BACKEND:-test}"
GAS="${GAS:-auto}"
GAS_ADJ="${GAS_ADJUSTMENT:-1.5}"
GAS_PRICES="${GAS_PRICES:-0.075ujuno}"
NODE="${NODE:-http://localhost:26657}"

exec_tx() {
  docker exec junoclaw-bn254-devnet junod tx "$@" \
    --chain-id "${CHAIN_ID}" \
    --keyring-backend "${KEYRING}" \
    --gas "${GAS}" --gas-adjustment "${GAS_ADJ}" --gas-prices "${GAS_PRICES}" \
    --broadcast-mode sync --yes --output json
}

query_ids_from_store() {
  local tx_hash="$1"
  # Poll for inclusion. With ~2 s block time and broadcast-mode=sync, the
  # tx is usually visible after the second block, but indexer flush can
  # take an extra block on a busy proposer.
  local out=""
  for i in $(seq 1 20); do
    sleep 1
    out=$(docker exec junoclaw-bn254-devnet junod query tx "${tx_hash}" \
            --output json 2>/dev/null || true)
    if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
      break
    fi
  done
  echo "${out}" | jq -r '.events[] | select(.type == "store_code")
                            | .attributes[]
                            | select(.key == "code_id")
                            | .value' | head -n1
}

query_addr_from_instantiate() {
  local tx_hash="$1"
  local out=""
  for i in $(seq 1 20); do
    sleep 1
    out=$(docker exec junoclaw-bn254-devnet junod query tx "${tx_hash}" \
            --output json 2>/dev/null || true)
    if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
      break
    fi
  done
  echo "${out}" | jq -r '.events[] | select(.type == "instantiate")
                            | .attributes[]
                            | select(.key == "_contract_address")
                            | .value' | head -n1
}

# ── 1. Build both flavours. ───────────────────────────────────────────────
#
# Set BUILD=0 to skip cargo and reuse pre-built ${DEVNET_DIR}/zk_verifier_*.wasm.
# Useful when cargo lives on the host while this script runs inside WSL, and
# keeps deploy + benchmark decoupled from the build step in CI.

BUILD="${BUILD:-1}"
WASM_PURE_OUT="${DEVNET_DIR}/zk_verifier_pure.wasm"
WASM_PREC_OUT="${DEVNET_DIR}/zk_verifier_precompile.wasm"

if [ "${BUILD}" = "1" ]; then
  echo "[deploy] Building zk-verifier (pure-Wasm)…"
  ( cd "${REPO_ROOT}/contracts" && \
      cargo build --release --target wasm32-unknown-unknown -p zk-verifier )
  cp "${REPO_ROOT}/contracts/target/wasm32-unknown-unknown/release/zk_verifier.wasm" \
     "${WASM_PURE_OUT}"

  echo "[deploy] Building zk-verifier (precompile feature)…"
  ( cd "${REPO_ROOT}/contracts" && \
      cargo build --release --target wasm32-unknown-unknown -p zk-verifier \
          --features bn254-precompile )
  cp "${REPO_ROOT}/contracts/target/wasm32-unknown-unknown/release/zk_verifier.wasm" \
     "${WASM_PREC_OUT}"
else
  echo "[deploy] BUILD=0 — reusing pre-built wasm artefacts"
  for f in "${WASM_PURE_OUT}" "${WASM_PREC_OUT}"; do
    if [ ! -f "${f}" ]; then
      echo "error: ${f} missing. Run with BUILD=1 or produce the artefacts first." >&2
      exit 3
    fi
    printf '  %s  (%s bytes)\n' "${f}" "$(stat -c %s "${f}" 2>/dev/null || wc -c <"${f}")"
  done
fi

# ── 2. Copy into the container. ───────────────────────────────────────────

docker cp "${DEVNET_DIR}/zk_verifier_pure.wasm"       junoclaw-bn254-devnet:/tmp/
docker cp "${DEVNET_DIR}/zk_verifier_precompile.wasm" junoclaw-bn254-devnet:/tmp/

# ── 3. Store + instantiate both. ─────────────────────────────────────────

ADMIN=$(docker exec junoclaw-bn254-devnet junod keys show admin -a --keyring-backend "${KEYRING}")

echo "[deploy] Uploading pure-Wasm variant…"
PURE_TX=$(exec_tx wasm store /tmp/zk_verifier_pure.wasm --from admin | jq -r '.txhash')
sleep 8
PURE_CODE_ID=$(query_ids_from_store "${PURE_TX}")

echo "[deploy] Uploading precompile variant…"
PREC_TX=$(exec_tx wasm store /tmp/zk_verifier_precompile.wasm --from admin | jq -r '.txhash')
sleep 8
PREC_CODE_ID=$(query_ids_from_store "${PREC_TX}")

echo "[deploy] Instantiating both…"
echo "  pure code_id      = ${PURE_CODE_ID}"
echo "  precompile code_id = ${PREC_CODE_ID}"
if [ -z "${PURE_CODE_ID}" ] || [ -z "${PREC_CODE_ID}" ]; then
  echo "error: one of the store_code txs did not return a code_id" >&2
  exit 4
fi
sleep 8
INIT='{"admin":"'${ADMIN}'"}'
PURE_INIT_TX=$(exec_tx wasm instantiate "${PURE_CODE_ID}" "${INIT}" \
    --from admin --label "zk-verifier-pure" --no-admin | jq -r '.txhash')
sleep 8
PREC_INIT_TX=$(exec_tx wasm instantiate "${PREC_CODE_ID}" "${INIT}" \
    --from admin --label "zk-verifier-precompile" --no-admin | jq -r '.txhash')

PURE_ADDR=$(query_addr_from_instantiate "${PURE_INIT_TX}")
PREC_ADDR=$(query_addr_from_instantiate "${PREC_INIT_TX}")

cat > "${DEVNET_DIR}/deploy.env" <<EOF
# generated by deploy-zk-verifier.sh — consumed by benchmark.sh
CHAIN_ID=${CHAIN_ID}
NODE=${NODE}
PURE_CODE_ID=${PURE_CODE_ID}
PURE_ADDR=${PURE_ADDR}
PRECOMPILE_CODE_ID=${PREC_CODE_ID}
PRECOMPILE_ADDR=${PREC_ADDR}
ADMIN_ADDR=${ADMIN}
EOF

echo "[deploy] Done. Pure addr: ${PURE_ADDR}"
echo "[deploy] Done. Precompile addr: ${PREC_ADDR}"
echo "[deploy] Wrote ${DEVNET_DIR}/deploy.env"
