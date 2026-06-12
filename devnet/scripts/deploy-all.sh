#!/usr/bin/env bash
# Deploy all pending contracts to the live BN254 devnet.
# Order: zk-verifier -> jclaw-credential -> moultbook
#
# Environment:
#   BUILD=0 (default) — reuse pre-built wasm artefacts
#   CHAIN_ID          (default junoclaw-bn254-1)
#   CONTAINER         (default junoclaw-bn254-devnet)
#   KEYRING_BACKEND   (default test)

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CHAIN_ID="${CHAIN_ID:-junoclaw-bn254-1}"
CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
KEYRING="${KEYRING_BACKEND:-test}"
GAS="${GAS:-auto}"
GAS_ADJ="${GAS_ADJUSTMENT:-1.5}"
GAS_PRICES="${GAS_PRICES:-0.075ujuno}"
NODE="${NODE:-http://localhost:26657}"
BUILD="${BUILD:-0}"

echo "=============================================="
echo "  Deploy All — junoclaw-bn254-devnet"
echo "  BUILD=${BUILD}  CHAIN_ID=${CHAIN_ID}"
echo "=============================================="

# ── Sanity checks ─────────────────────────────────────────────────────────
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
  echo "error: container '${CONTAINER}' not running" >&2
  exit 1
fi

ADMIN=$(docker exec "${CONTAINER}" junod keys show admin -a --keyring-backend "${KEYRING}")
echo "[deploy-all] admin = ${ADMIN}"

# ── 1. zk-verifier (pure + precompile) ───────────────────────────────────
echo ""
echo "[deploy-all] === 1/3  zk-verifier ==="
bash "${HERE}/deploy-zk-verifier.sh"

# Source the freshly written deploy.env
# shellcheck source=/dev/null
source "${DEVNET_DIR}/deploy.env"
echo "[deploy-all]   pure      code_id=${PURE_CODE_ID} addr=${PURE_ADDR}"
echo "[deploy-all]   precompile code_id=${PRECOMPILE_CODE_ID} addr=${PRECOMPILE_ADDR}"

# ── 2. jclaw-credential ──────────────────────────────────────────────────
echo ""
echo "[deploy-all] === 2/3  jclaw-credential ==="

JCLAW_WASM="${DEVNET_DIR}/artifacts/jclaw_credential.wasm"
if [ ! -f "${JCLAW_WASM}" ]; then
  echo "error: ${JCLAW_WASM} missing" >&2
  exit 2
fi
docker cp "${JCLAW_WASM}" "${CONTAINER}:/tmp/jclaw_credential.wasm"

exec_tx() {
  docker exec "${CONTAINER}" junod tx "$@" \
    --chain-id "${CHAIN_ID}" \
    --keyring-backend "${KEYRING}" \
    --gas "${GAS}" --gas-adjustment "${GAS_ADJ}" --gas-prices "${GAS_PRICES}" \
    --broadcast-mode sync --yes --output json
}

wait_tx() {
  local hash="$1"
  local out=""
  for _ in $(seq 1 25); do
    sleep 1
    out=$(docker exec "${CONTAINER}" junod query tx "${hash}" --output json 2>/dev/null || true)
    if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
      printf '%s' "${out}"
      return 0
    fi
  done
  echo "error: tx ${hash} not indexed" >&2
  return 4
}

extract_attr() {
  local tx_json="$1" ev_type="$2" attr_key="$3"
  echo "${tx_json}" | jq -r --arg t "${ev_type}" --arg k "${attr_key}" \
    '.events[] | select(.type == $t) | .attributes[] | select(.key == $k) | .value' | head -n1
}

# Store
STORE_TX=$(exec_tx wasm store /tmp/jclaw_credential.wasm --from admin | jq -r '.txhash')
STORE_JSON=$(wait_tx "${STORE_TX}")
JCLAW_CODE_ID=$(extract_attr "${STORE_JSON}" store_code code_id)
echo "[deploy-all]   code_id=${JCLAW_CODE_ID}"

# Instantiate
JCLAW_INIT=$(jq -nc --arg admin "${ADMIN}" '{admin: $admin}')
INIT_TX=$(exec_tx wasm instantiate "${JCLAW_CODE_ID}" "${JCLAW_INIT}" \
    --from admin --label "jclaw-credential" --no-admin | jq -r '.txhash')
INIT_JSON=$(wait_tx "${INIT_TX}")
JCLAW_ADDR=$(extract_attr "${INIT_JSON}" instantiate _contract_address)
echo "[deploy-all]   addr=${JCLAW_ADDR}"

# ── 3. moultbook ───────────────────────────────────────────────────────────
echo ""
echo "[deploy-all] === 3/3  moultbook ==="

# Wire zk-verifier precompile into moultbook if available
export ZK_VERIFIER="${PRECOMPILE_ADDR:-}"
export AGENT_REGISTRY="${JCLAW_ADDR:-}"
export BUILD=0
export SMOKE=1

bash "${HERE}/deploy-moultbook.sh"

# Source moultbook.env
# shellcheck source=/dev/null
source "${DEVNET_DIR}/moultbook.env"
echo "[deploy-all]   code_id=${MOULTBOOK_CODE_ID} addr=${MOULTBOOK_ADDR}"

# ── Final summary ────────────────────────────────────────────────────────
echo ""
echo "=============================================="
echo "  Deployment Summary"
echo "=============================================="
echo "  zk-verifier-pure      : ${PURE_ADDR}"
echo "  zk-verifier-precompile: ${PRECOMPILE_ADDR}"
echo "  jclaw-credential      : ${JCLAW_ADDR}"
echo "  moultbook             : ${MOULTBOOK_ADDR}"
echo ""
echo "  Files written:"
echo "    ${DEVNET_DIR}/deploy.env"
echo "    ${DEVNET_DIR}/moultbook.env"
echo "=============================================="
