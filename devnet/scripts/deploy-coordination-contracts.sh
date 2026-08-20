#!/usr/bin/env bash
# Deploy the four coordination-layer contracts to the BN254 devnet:
#   1. safety-envelope
#   2. merkle-verifier
#   3. circuit-breaker
#   4. coordination-settler
#
# Each contract is built with the cosmwasm/optimizer, stored on-chain,
# and instantiated with appropriate init params.
#
# Environment:
#   CHAIN_ID          (default junoclaw-bn254-1)
#   CONTAINER         (default junoclaw-bn254-devnet)
#   KEYRING_BACKEND   (default test)
#   BUILD             (default 1 — build with optimizer)
#   OPTIMIZER_TAG     (default 0.16.1)
#   GAS               (default auto)
#   GAS_ADJUSTMENT    (default 1.5)
#   GAS_PRICES        (default 0.075ujuno)
#   NODE              (default http://localhost:26657)
#
# Output: devnet/coordination-contracts.env with addresses + code IDs.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"
CONTRACTS_DIR="${REPO_ROOT}/contracts"
ARTIFACTS_DIR="${DEVNET_DIR}/artifacts"

CHAIN_ID="${CHAIN_ID:-junoclaw-bn254-1}"
CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
KEYRING="${KEYRING_BACKEND:-test}"
GAS="${GAS:-auto}"
GAS_ADJ="${GAS_ADJUSTMENT:-1.5}"
GAS_PRICES="${GAS_PRICES:-0.075ujuno}"
NODE="${NODE:-http://localhost:26657}"
BUILD="${BUILD:-1}"
OPTIMIZER_TAG="${OPTIMIZER_TAG:-0.16.1}"
IMAGE="cosmwasm/optimizer:${OPTIMIZER_TAG}"

echo "=============================================="
echo "  Deploy Coordination Contracts"
echo "  BUILD=${BUILD}  CHAIN_ID=${CHAIN_ID}"
echo "=============================================="

# ── Sanity checks ─────────────────────────────────────────────────────────
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
  echo "error: container '${CONTAINER}' not running" >&2
  exit 1
fi

ADMIN=$(docker exec "${CONTAINER}" junod keys show admin -a --keyring-backend "${KEYRING}")
echo "[deploy] admin = ${ADMIN}"

# ── Helper functions ──────────────────────────────────────────────────────

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
  for _ in $(seq 1 30); do
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

# Build a single contract with the cosmwasm/optimizer.
# Usage: build_contract <contract_name> <artifact_filename>
build_contract() {
  local name="$1"
  local artifact="$2"
  local src_dir="${CONTRACTS_DIR}/${name}"

  if [ ! -d "${src_dir}" ]; then
    echo "error: contract source ${src_dir} not found" >&2
    exit 2
  fi

  echo "[build] ${name} → ${artifact}"
  docker run --rm \
    -e CARGO_TERM_COLOR=never \
    -v "${src_dir}:/code:ro" \
    -v "junoclaw-contracts-target:/code/target" \
    -v "${ARTIFACTS_DIR}:/output:ro" \
    --entrypoint sh \
    "${IMAGE}" \
    -c "cd /code && cargo build --release --lib --target wasm32-unknown-unknown && cp target/wasm32-unknown-unknown/release/*.wasm /output/ 2>/dev/null || true"

  # The optimizer writes to /code/artifacts/ — try that path
  docker run --rm \
    -e CARGO_TERM_COLOR=never \
    -v "${src_dir}:/code:ro" \
    -v "junoclaw-contracts-target:/code/target" \
    -v "${ARTIFACTS_DIR}:/output" \
    --entrypoint sh \
    "${IMAGE}" \
    -c "cd /code && cargo build --release --lib --target wasm32-unknown-unknown 2>&1 && cp target/wasm32-unknown-unknown/release/*.wasm /output/${artifact}"

  if [ ! -f "${ARTIFACTS_DIR}/${artifact}" ]; then
    echo "error: build failed — ${ARTIFACTS_DIR}/${artifact} not created" >&2
    exit 3
  fi
  echo "[build] done: $(wc -c < "${ARTIFACTS_DIR}/${artifact}") bytes"
}

# Store + instantiate a contract.
# Usage: deploy_contract <artifact_filename> <label> <init_json>
# Sets: CODE_ID and ADDR env vars
deploy_contract() {
  local artifact="$1"
  local label="$2"
  local init_json="$3"
  local wasm_path="/tmp/$(basename "${artifact}")"

  echo ""
  echo "[deploy] === ${label} ==="

  # Copy wasm into container
  docker cp "${ARTIFACTS_DIR}/${artifact}" "${CONTAINER}:${wasm_path}"

  # Store code
  local store_tx store_json code_id
  store_tx=$(exec_tx wasm store "${wasm_path}" --from admin | jq -r '.txhash')
  store_json=$(wait_tx "${store_tx}")
  code_id=$(extract_attr "${store_json}" store_code code_id)
  echo "[deploy]   code_id=${code_id}"

  # Check store tx code
  local store_code
  store_code=$(echo "${store_json}" | jq -r '.code // 0')
  if [ "${store_code}" != "0" ]; then
    echo "error: store code failed with code ${store_code}" >&2
    echo "${store_json}" | jq '.raw_log' >&2
    exit 5
  fi

  # Instantiate
  local init_tx init_json_resp addr
  init_tx=$(exec_tx wasm instantiate "${code_id}" "${init_json}" \
    --from admin --label "${label}" --no-admin | jq -r '.txhash')
  init_json_resp=$(wait_tx "${init_tx}")
  addr=$(extract_attr "${init_json_resp}" instantiate _contract_address)
  echo "[deploy]   addr=${addr}"

  # Check instantiate tx code
  local init_code
  init_code=$(echo "${init_json_resp}" | jq -r '.code // 0')
  if [ "${init_code}" != "0" ]; then
    echo "error: instantiate failed with code ${init_code}" >&2
    echo "${init_json_resp}" | jq '.raw_log' >&2
    exit 6
  fi

  CODE_ID="${code_id}"
  ADDR="${addr}"
}

# ── Build contracts ───────────────────────────────────────────────────────

if [ "${BUILD}" = "1" ]; then
  docker volume create junoclaw-contracts-target >/dev/null 2>&1 || true

  build_contract "safety-envelope" "safety_envelope.wasm"
  build_contract "merkle-verifier" "merkle_verifier.wasm"
  build_contract "circuit-breaker" "circuit_breaker.wasm"
  build_contract "coordination-settler" "coordination_settler.wasm"
fi

# ── 1. Safety Envelope ────────────────────────────────────────────────────
# InstantiateMsg: { admin: "<admin_addr>" }
SAFETY_INIT=$(jq -nc --arg admin "${ADMIN}" '{admin: $admin}')
deploy_contract "safety_envelope.wasm" "safety-envelope" "${SAFETY_INIT}"
SAFETY_CODE_ID="${CODE_ID}"
SAFETY_ADDR="${ADDR}"

# ── 2. Merkle Verifier ────────────────────────────────────────────────────
# InstantiateMsg: { admin: "<admin_addr>" }
MERKLE_INIT=$(jq -nc --arg admin "${ADMIN}" '{admin: $admin}')
deploy_contract "merkle_verifier.wasm" "merkle-verifier" "${MERKLE_INIT}"
MERKLE_CODE_ID="${CODE_ID}"
MERKLE_ADDR="${ADDR}"

# ── 3. Circuit Breaker ────────────────────────────────────────────────────
# InstantiateMsg: { admin: "<admin_addr>" }
BREAKER_INIT=$(jq -nc --arg admin "${ADMIN}" '{admin: $admin}')
deploy_contract "circuit_breaker.wasm" "circuit-breaker" "${BREAKER_INIT}"
BREAKER_CODE_ID="${CODE_ID}"
BREAKER_ADDR="${ADDR}"

# ── 4. Coordination Settler ───────────────────────────────────────────────
# InstantiateMsg: { admin, validators: [<binary>], threshold: N }
# For devnet: empty validator set, threshold 1 (updated later via governance)
SETTLER_INIT=$(jq -nc --arg admin "${ADMIN}" '{admin: $admin, validators: [], threshold: 1}')
deploy_contract "coordination_settler.wasm" "coordination-settler" "${SETTLER_INIT}"
SETTLER_CODE_ID="${CODE_ID}"
SETTLER_ADDR="${ADDR}"

# ── Write env file ────────────────────────────────────────────────────────
ENV_FILE="${DEVNET_DIR}/coordination-contracts.env"
cat > "${ENV_FILE}" <<EOF
# Coordination contracts — deployed $(date -u +%Y-%m-%dT%H:%M:%SZ)
SAFETY_ENVELOPE_CODE_ID=${SAFETY_CODE_ID}
SAFETY_ENVELOPE_ADDR=${SAFETY_ADDR}
MERKLE_VERIFIER_CODE_ID=${MERKLE_CODE_ID}
MERKLE_VERIFIER_ADDR=${MERKLE_ADDR}
CIRCUIT_BREAKER_CODE_ID=${BREAKER_CODE_ID}
CIRCUIT_BREAKER_ADDR=${BREAKER_ADDR}
COORDINATION_SETTLER_CODE_ID=${SETTLER_CODE_ID}
COORDINATION_SETTLER_ADDR=${SETTLER_ADDR}
EOF

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo "=============================================="
echo "  Deployment Summary"
echo "=============================================="
echo "  safety-envelope       code=${SAFETY_CODE_ID}  addr=${SAFETY_ADDR}"
echo "  merkle-verifier       code=${MERKLE_CODE_ID}  addr=${MERKLE_ADDR}"
echo "  circuit-breaker       code=${BREAKER_CODE_ID}  addr=${BREAKER_ADDR}"
echo "  coordination-settler  code=${SETTLER_CODE_ID}  addr=${SETTLER_ADDR}"
echo ""
echo "  Env file: ${ENV_FILE}"
echo "=============================================="
