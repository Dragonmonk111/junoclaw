#!/usr/bin/env bash
# Builds (or reuses) moultbook_v0.wasm, uploads it to the running devnet,
# instantiates it, runs a Post -> GetEntry -> Redact smoke test, and writes
# a deploy.env-style file for follow-on scripts (gas measurement, article
# attestation, the dog-fooded "first two entries" sequencing described in
# docs/MOULTBOOK_DEV_COLLABORATION_NOTES.md §6).
#
# Environment overrides:
#   BUILD=1          (default 1)  — set to 0 to reuse devnet/moultbook_v0.wasm
#   SMOKE=1          (default 1)  — set to 0 to skip the smoke test
#   CHAIN_ID         (default junoclaw-bn254-1)
#   CONTAINER        (default junoclaw-bn254-devnet)
#   NODE             (default http://localhost:26657)
#   KEYRING_BACKEND  (default test)
#   GAS_PRICES       (default 0.025ujuno; bump to 0.1 if the devnet's
#                     globalfee floor is 0.1 — see MEDIUM_ARTICLE_BN254_MEASURED)
#   MAX_SIZE_BYTES   (default 1048576)
#   MAX_REFS         (default 8)
#   MAX_CT_LEN       (default 64)

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

BUILD="${BUILD:-1}"
SMOKE="${SMOKE:-1}"
CHAIN_ID="${CHAIN_ID:-junoclaw-bn254-1}"
CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
NODE="${NODE:-http://localhost:26657}"
KEYRING="${KEYRING_BACKEND:-test}"
GAS="${GAS:-auto}"
GAS_ADJ="${GAS_ADJUSTMENT:-1.3}"
GAS_PRICES="${GAS_PRICES:-0.025ujuno}"

MAX_SIZE_BYTES="${MAX_SIZE_BYTES:-1048576}"
MAX_REFS="${MAX_REFS:-8}"
MAX_CT_LEN="${MAX_CT_LEN:-64}"

WASM_OUT="${DEVNET_DIR}/moultbook_v0.wasm"

# ── 0. Sanity: devnet container running. ─────────────────────────────────

if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
  echo "error: devnet container '${CONTAINER}' is not running" >&2
  echo "       start it with: bash ${DEVNET_DIR}/scripts/run-devnet.sh" >&2
  exit 1
fi

# ── 1. Build (or reuse). ─────────────────────────────────────────────────

if [ "${BUILD}" = "1" ]; then
  echo "[deploy-moultbook] BUILD=1 — building via cosmwasm/optimizer"
  bash "${HERE}/build-moultbook.sh"
else
  echo "[deploy-moultbook] BUILD=0 — reusing ${WASM_OUT}"
  if [ ! -f "${WASM_OUT}" ]; then
    echo "error: ${WASM_OUT} missing. Run with BUILD=1 or build first." >&2
    exit 2
  fi
  printf '  size: %s bytes\n' "$(stat -c %s "${WASM_OUT}" 2>/dev/null || wc -c <"${WASM_OUT}")"
fi

docker cp "${WASM_OUT}" "${CONTAINER}:/tmp/moultbook_v0.wasm"

# ── 2. Helpers. ──────────────────────────────────────────────────────────

exec_tx() {
  docker exec "${CONTAINER}" junod tx "$@" \
    --chain-id "${CHAIN_ID}" \
    --keyring-backend "${KEYRING}" \
    --node "${NODE}" \
    --gas "${GAS}" --gas-adjustment "${GAS_ADJ}" --gas-prices "${GAS_PRICES}" \
    --broadcast-mode sync --yes --output json
}

wait_tx() {
  # Poll up to ~20 s for tx inclusion. With ~2 s block time and broadcast
  # mode=sync, normal inclusion is in the 2nd or 3rd block; indexer flush
  # on a busy proposer can take an extra block.
  local hash="$1"
  local out=""
  for _ in $(seq 1 20); do
    sleep 1
    out=$(docker exec "${CONTAINER}" junod query tx "${hash}" \
            --node "${NODE}" --output json 2>/dev/null || true)
    if [ -n "${out}" ] && [ "$(echo "${out}" | jq -r '.code // empty')" != "" ]; then
      printf '%s' "${out}"
      return 0
    fi
  done
  echo "error: tx ${hash} not indexed after 20 s" >&2
  return 4
}

extract_event_attr() {
  # extract_event_attr <tx-json> <event-type> <attr-key> -> first matching value
  local tx_json="$1" ev_type="$2" attr_key="$3"
  echo "${tx_json}" \
    | jq -r --arg t "${ev_type}" --arg k "${attr_key}" '
        .events[] | select(.type == $t) | .attributes[] | select(.key == $k) | .value
      ' | head -n1
}

# ── 3. Store + instantiate. ──────────────────────────────────────────────

ADMIN=$(docker exec "${CONTAINER}" junod keys show admin -a --keyring-backend "${KEYRING}")
echo "[deploy-moultbook] admin: ${ADMIN}"

echo "[deploy-moultbook] Uploading wasm…"
STORE_TX=$(exec_tx wasm store /tmp/moultbook_v0.wasm --from admin | jq -r '.txhash')
STORE_JSON=$(wait_tx "${STORE_TX}")
CODE_ID=$(extract_event_attr "${STORE_JSON}" store_code code_id)
STORE_GAS_USED=$(echo "${STORE_JSON}" | jq -r '.gas_used // empty')
if [ -z "${CODE_ID}" ]; then
  echo "error: store_code did not return a code_id" >&2
  echo "${STORE_JSON}" | jq '.' >&2
  exit 5
fi
echo "[deploy-moultbook] code_id=${CODE_ID}  store_gas=${STORE_GAS_USED}"

INIT=$(jq -nc \
  --arg admin "${ADMIN}" \
  --argjson maxsize "${MAX_SIZE_BYTES}" \
  --argjson maxrefs "${MAX_REFS}" \
  --argjson maxctlen "${MAX_CT_LEN}" \
  '{
     admin: $admin,
     whoami_contract: null,
     max_size_bytes: $maxsize,
     max_refs: $maxrefs,
     max_content_type_len: $maxctlen
   }')

echo "[deploy-moultbook] Instantiating…"
INIT_TX=$(exec_tx wasm instantiate "${CODE_ID}" "${INIT}" \
    --from admin --label "moultbook-v0" --admin "${ADMIN}" | jq -r '.txhash')
INIT_JSON=$(wait_tx "${INIT_TX}")
ADDR=$(extract_event_attr "${INIT_JSON}" instantiate _contract_address)
INIT_GAS_USED=$(echo "${INIT_JSON}" | jq -r '.gas_used // empty')
if [ -z "${ADDR}" ]; then
  echo "error: instantiate did not return a contract address" >&2
  echo "${INIT_JSON}" | jq '.' >&2
  exit 6
fi
echo "[deploy-moultbook] addr=${ADDR}  init_gas=${INIT_GAS_USED}"

# ── 4. Smoke test (Post -> GetEntry -> Redact). ─────────────────────────

POST_TX=""; POST_GAS_USED=""; ENTRY_ID=""; REDACT_TX=""; REDACT_GAS_USED=""

if [ "${SMOKE}" = "1" ]; then
  echo
  echo "[deploy-moultbook] Smoke test — Post → GetEntry → Redact"

  # Deterministic 32-byte commitment: sha256 of a fixed seed phrase.
  # Same seed produces the same commitment across re-runs, which is fine —
  # the contract's id formula includes posted_at_nanos, so re-runs still
  # produce distinct entry IDs on fresh blocks.
  SEED="moultbook devnet smoke v0 — first post"
  COMMIT_HEX=$(printf '%s' "${SEED}" | sha256sum | awk '{print $1}')
  COMMIT_B64=$(printf '%s' "${COMMIT_HEX}" | xxd -r -p | base64 -w0)

  POST_MSG=$(jq -nc --arg c "${COMMIT_B64}" '{
    post: {
      commitment:    $c,
      content_type:  "text/markdown",
      size_bytes:    256,
      attestation_ref: null,
      visibility:    "public",
      refs:          []
    }
  }')

  POST_TX=$(exec_tx wasm execute "${ADDR}" "${POST_MSG}" --from admin | jq -r '.txhash')
  POST_JSON=$(wait_tx "${POST_TX}")
  POST_GAS_USED=$(echo "${POST_JSON}" | jq -r '.gas_used // empty')
  ENTRY_ID=$(extract_event_attr "${POST_JSON}" wasm id)
  if [ -z "${ENTRY_ID}" ]; then
    echo "error: Post did not emit an id attribute" >&2
    echo "${POST_JSON}" | jq '.' >&2
    exit 7
  fi
  echo "[deploy-moultbook]   posted id=${ENTRY_ID}  post_gas=${POST_GAS_USED}"

  # Query it back.
  QUERY_MSG=$(jq -nc --arg id "${ENTRY_ID}" '{get_entry: {id: $id}}')
  ENTRY_JSON=$(docker exec "${CONTAINER}" junod query wasm contract-state smart \
                  "${ADDR}" "${QUERY_MSG}" --node "${NODE}" --output json)
  ENTRY_AUTHOR=$(echo "${ENTRY_JSON}" | jq -r '.data.author // empty')
  ENTRY_CT=$(echo "${ENTRY_JSON}" | jq -r '.data.content_type // empty')
  ENTRY_SIZE=$(echo "${ENTRY_JSON}" | jq -r '.data.size_bytes // empty')
  echo "[deploy-moultbook]   GetEntry: author=${ENTRY_AUTHOR}  content_type=${ENTRY_CT}  size_bytes=${ENTRY_SIZE}"
  if [ "${ENTRY_AUTHOR}" != "${ADMIN}" ]; then
    echo "error: GetEntry returned unexpected author '${ENTRY_AUTHOR}' (expected ${ADMIN})" >&2
    exit 8
  fi

  # Redact (author=admin so this is the author-redact path).
  REDACT_MSG=$(jq -nc --arg id "${ENTRY_ID}" '{redact: {id: $id}}')
  REDACT_TX=$(exec_tx wasm execute "${ADDR}" "${REDACT_MSG}" --from admin | jq -r '.txhash')
  REDACT_JSON=$(wait_tx "${REDACT_TX}")
  REDACT_GAS_USED=$(echo "${REDACT_JSON}" | jq -r '.gas_used // empty')
  echo "[deploy-moultbook]   redacted: tx=${REDACT_TX}  redact_gas=${REDACT_GAS_USED}"

  # Verify redaction took.
  POST_REDACT_JSON=$(docker exec "${CONTAINER}" junod query wasm contract-state smart \
                       "${ADDR}" "${QUERY_MSG}" --node "${NODE}" --output json)
  COMMITMENT_AFTER=$(echo "${POST_REDACT_JSON}" | jq -r '.data.commitment // empty')
  REDACTED_AT=$(echo "${POST_REDACT_JSON}" | jq -r '.data.redacted_at // empty')
  if [ -n "${COMMITMENT_AFTER}" ]; then
    echo "error: commitment not cleared after redact (got '${COMMITMENT_AFTER}')" >&2
    exit 9
  fi
  if [ -z "${REDACTED_AT}" ] || [ "${REDACTED_AT}" = "null" ]; then
    echo "error: redacted_at not set after redact" >&2
    exit 10
  fi
  echo "[deploy-moultbook]   redaction verified: commitment cleared, redacted_at=${REDACTED_AT}"
else
  echo "[deploy-moultbook] SMOKE=0 — skipping smoke test"
fi

# ── 5. Persist deploy outputs. ───────────────────────────────────────────

cat > "${DEVNET_DIR}/moultbook.env" <<EOF
# Generated by deploy-moultbook.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ)
# Consumed by: future gas-measurement and dog-food-anchor scripts.
CHAIN_ID=${CHAIN_ID}
NODE=${NODE}
CONTAINER=${CONTAINER}
ADMIN_ADDR=${ADMIN}
MOULTBOOK_CODE_ID=${CODE_ID}
MOULTBOOK_ADDR=${ADDR}
STORE_TX=${STORE_TX}
STORE_GAS_USED=${STORE_GAS_USED}
INIT_TX=${INIT_TX}
INIT_GAS_USED=${INIT_GAS_USED}
SMOKE_TEST=${SMOKE}
SMOKE_POST_TX=${POST_TX}
SMOKE_POST_GAS_USED=${POST_GAS_USED}
SMOKE_ENTRY_ID=${ENTRY_ID}
SMOKE_REDACT_TX=${REDACT_TX}
SMOKE_REDACT_GAS_USED=${REDACT_GAS_USED}
EOF

echo
echo "[deploy-moultbook] Done. Wrote ${DEVNET_DIR}/moultbook.env"
echo "[deploy-moultbook] Contract: ${ADDR}"
if [ "${SMOKE}" = "1" ]; then
  echo "[deploy-moultbook] Post gas (cold path with one full entry): ${POST_GAS_USED} SDK gas"
  echo "[deploy-moultbook]   ADR-002 projection band: 40,000 – 60,000 SDK gas"
  echo "[deploy-moultbook]   Capture this number in docs/MOULTBOOK_BENCHMARK_RESULTS.md when running the formal benchmark."
fi
