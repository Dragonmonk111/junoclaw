#!/usr/bin/env bash
# Quick health-check for deployed zk-verifier contracts.
#
#   1. Generate a Groth16 proof bundle (reuses existing if present).
#   2. Query VkStatus on both contracts (should be no-vk initially).
#   3. Store VK on both contracts (admin tx).
#   4. Execute VerifyProof on both contracts.
#   5. Query LastVerify on both contracts (should report verified=true).
#
# Exits 0 if both variants pass, 1 if any step fails.
#
# Usage:
#   bash devnet/scripts/smoke-test.sh
#   bash devnet/scripts/smoke-test.sh  # idempotent — safe to re-run

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CONTAINER=${CONTAINER:-junoclaw-bn254-devnet}
CHAIN_ID=${CHAIN_ID:-junoclaw-bn254-1}
KEYRING=${KEYRING_BACKEND:-test}
GAS=${GAS:-auto}
GAS_ADJ=${GAS_ADJUSTMENT:-1.5}
GAS_PRICES=${GAS_PRICES:-0.075ujuno}

PASS=0
FAIL=0

step() { printf '\n\033[1;36m[smoke] %s\033[0m\n' "$*"; }
ok()   { printf '  \033[1;32m✓ %s\033[0m\n' "$*"; ((PASS++)) || true; }
fail() { printf '  \033[1;31m✗ %s\033[0m\n' "$*"; ((FAIL++)) || true; }

# ── 0. Load deploy.env ────────────────────────────────────────────────

if [ ! -f "${DEVNET_DIR}/deploy.env" ]; then
  echo "error: deploy.env missing — run deploy-zk-verifier.sh or deploy-oneshot.sh first" >&2
  exit 1
fi
# shellcheck disable=SC1091
source "${DEVNET_DIR}/deploy.env"

# ── 1. Ensure proof bundle exists ─────────────────────────────────────

PROOF_JSON="${REPO_ROOT}/tmpdir/groth16_proof.json"
if [ ! -f "${PROOF_JSON}" ]; then
  step "Generating proof bundle → ${PROOF_JSON}…"
  mkdir -p "$(dirname "${PROOF_JSON}")"
  ( cd "${REPO_ROOT}/contracts/zk-verifier" && \
      PROOF_OUTPUT="${PROOF_JSON}" cargo run --quiet --example generate_proof >/dev/null )
fi

# Extract base64 fields
VK_B64=$(jq -r '.vk_base64' "${PROOF_JSON}")
PROOF_B64=$(jq -r '.proof_base64' "${PROOF_JSON}")
INPUTS_B64=$(jq -r '.public_inputs_base64' "${PROOF_JSON}")

if [ -z "${VK_B64}" ] || [ -z "${PROOF_B64}" ] || [ -z "${INPUTS_B64}" ]; then
  echo "error: proof JSON missing required fields" >&2
  exit 2
fi

# ── Helpers ───────────────────────────────────────────────────────────

exec_tx() {
  docker exec "${CONTAINER}" junod tx "$@" \
    --chain-id "${CHAIN_ID}" --keyring-backend "${KEYRING}" \
    --gas "${GAS}" --gas-adjustment "${GAS_ADJ}" --gas-prices "${GAS_PRICES}" \
    --broadcast-mode sync --yes --output json
}

wait_tx() {
  local h="$1" out=""
  for _ in $(seq 1 20); do
    sleep 1
    out=$(docker exec "${CONTAINER}" junod query tx "$h" --output json 2>/dev/null || true)
    if [ -n "$out" ] && [ "$(echo "$out" | jq -r '.code // empty')" != "" ]; then
      local code
      code=$(echo "$out" | jq -r '.code')
      if [ "$code" != "0" ]; then
        echo "tx $h failed code=$code" >&2
        return 1
      fi
      return 0
    fi
  done
  echo "tx $h timeout" >&2
  return 1
}

query_contract() {
  local addr="$1" query="$2"
  docker exec "${CONTAINER}" junod query wasm contract-state smart "$addr" "$query" --output json 2>/dev/null
}

# ── 2. Query VkStatus (pre) ─────────────────────────────────────────

step "Querying VkStatus (pre-store)…"

PURE_PRE=$(query_contract "${PURE_ADDR}"   '{"vk_status":{}}' || true)
PREC_PRE=$(query_contract "${PRECOMPILE_ADDR}" '{"vk_status":{}}' || true)

if echo "${PURE_PRE}" | jq -e '.data.has_vk == false' >/dev/null 2>&1; then
  ok "pure   — no VK stored (expected)"
else
  fail "pure   — VkStatus unexpected: ${PURE_PRE}"
fi

if echo "${PREC_PRE}" | jq -e '.data.has_vk == false' >/dev/null 2>&1; then
  ok "prec   — no VK stored (expected)"
else
  fail "prec   — VkStatus unexpected: ${PREC_PRE}"
fi

# ── 3. Store VK on both contracts ───────────────────────────────────

step "Storing VK on both contracts…"

STORE_MSG="{\"store_vk\":{\"vk_base64\":\"${VK_B64}\"}}"

# Both txs are signed by `admin`; broadcast+commit them serially so the
# account sequence increments before the next tx is built (avoids the
# "account sequence mismatch" race from firing them back-to-back).
PURE_STORE_HASH=$(exec_tx wasm execute "${PURE_ADDR}"   "${STORE_MSG}" --from admin | jq -r '.txhash')
if wait_tx "${PURE_STORE_HASH}"; then ok "pure   — StoreVK tx committed"; else fail "pure   — StoreVK tx failed"; fi

PREC_STORE_HASH=$(exec_tx wasm execute "${PRECOMPILE_ADDR}" "${STORE_MSG}" --from admin | jq -r '.txhash')
if wait_tx "${PREC_STORE_HASH}"; then ok "prec   — StoreVK tx committed"; else fail "prec   — StoreVK tx failed"; fi

# ── 4. VerifyProof on both contracts ──────────────────────────────────

step "Executing VerifyProof on both contracts…"

VERIFY_MSG="{\"verify_proof\":{\"proof_base64\":\"${PROOF_B64}\",\"public_inputs_base64\":\"${INPUTS_B64}\"}}"

# Serial broadcast+commit (same account-sequence reasoning as StoreVK above).
PURE_VERIFY_HASH=$(exec_tx wasm execute "${PURE_ADDR}"   "${VERIFY_MSG}" --from admin | jq -r '.txhash')
if wait_tx "${PURE_VERIFY_HASH}"; then ok "pure   — VerifyProof tx committed"; else fail "pure   — VerifyProof tx failed"; fi

PREC_VERIFY_HASH=$(exec_tx wasm execute "${PRECOMPILE_ADDR}" "${VERIFY_MSG}" --from admin | jq -r '.txhash')
if wait_tx "${PREC_VERIFY_HASH}"; then ok "prec   — VerifyProof tx committed"; else fail "prec   — VerifyProof tx failed"; fi

# ── 5. Query LastVerify (post) ────────────────────────────────────────

step "Querying LastVerify (post-verify)…"

PURE_LAST=$(query_contract "${PURE_ADDR}"   '{"last_verify":{}}' || true)
PREC_LAST=$(query_contract "${PRECOMPILE_ADDR}" '{"last_verify":{}}' || true)

if echo "${PURE_LAST}" | jq -e '.data.verified == true' >/dev/null 2>&1; then
  ok "pure   — last verification succeeded"
else
  fail "pure   — last verification failed or missing: ${PURE_LAST}"
fi

if echo "${PREC_LAST}" | jq -e '.data.verified == true' >/dev/null 2>&1; then
  ok "prec   — last verification succeeded"
else
  fail "prec   — last verification failed or missing: ${PREC_LAST}"
fi

# ── Summary ───────────────────────────────────────────────────────────

step "Summary: ${PASS} passed / ${FAIL} failed"

if [ "${FAIL}" -eq 0 ]; then
  printf '\n\033[1;32mSMOKE TEST PASSED\033[0m\n\n'
  exit 0
else
  printf '\n\033[1;31mSMOKE TEST FAILED (%d failure(s))\033[0m\n\n' "${FAIL}"
  exit 1
fi
