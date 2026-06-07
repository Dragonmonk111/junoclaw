#!/usr/bin/env bash
# One-shot reproduction of the BN254 precompile benchmark.
#
# Idempotent: each phase is skipped if its artefact already exists, so this
# script can be re-run after a partial run without losing state.
#
#   1. Ensure devnet container is up and producing blocks.
#   2. Build pure-Wasm + precompile contracts (skipped if already on disk).
#   3. Deploy both variants (skipped if deploy.env exists).
#   4. Smoke-test both contracts (StoreVK + VerifyProof + query).
#   5. Run benchmark.sh — emits docs/BN254_BENCHMARK_RESULTS.md.
#
# Force a full rebuild/redeploy with FRESH=1.
# Skip smoke test with SKIP_SMOKE=1.
#
# Usage:
#   bash devnet/scripts/reproduce-benchmark.sh                # idempotent re-run
#   FRESH=1 bash devnet/scripts/reproduce-benchmark.sh        # full rebuild + redeploy
#   N=20 bash devnet/scripts/reproduce-benchmark.sh           # 20 samples per variant
#   SKIP_SMOKE=1 bash devnet/scripts/reproduce-benchmark.sh   # skip smoke-test, go straight to benchmark

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CONTAINER=${CONTAINER:-junoclaw-bn254-devnet}
COMPOSE_FILE="${DEVNET_DIR}/docker-compose.yml"
WASM_PURE="${DEVNET_DIR}/zk_verifier_pure.wasm"
WASM_PREC="${DEVNET_DIR}/zk_verifier_precompile.wasm"
DEPLOY_ENV="${DEVNET_DIR}/deploy.env"
FRESH=${FRESH:-0}

step() { printf '\n\033[1;36m[reproduce] %s\033[0m\n' "$*"; }

# ── 1. Devnet up ──────────────────────────────────────────────────────
step "1/6 ensuring devnet container is running"
if [ "${FRESH}" = "1" ]; then
  step "  FRESH=1 → tearing down existing devnet"
  docker compose -f "${COMPOSE_FILE}" down -v 2>/dev/null || true
  rm -f "${DEPLOY_ENV}"
fi

if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
  step "  starting docker compose"
  docker compose -f "${COMPOSE_FILE}" up -d
fi

step "  waiting for chain to produce blocks"
for i in $(seq 1 30); do
  H=$(docker exec "${CONTAINER}" junod status 2>/dev/null \
        | python3 -c 'import sys,json; print(json.load(sys.stdin).get("sync_info",{}).get("latest_block_height",0))' \
        2>/dev/null || echo 0)
  if [ "${H:-0}" -ge 2 ]; then
    step "  chain at height ${H}"
    break
  fi
  sleep 2
done

# ── 2. Build contracts ────────────────────────────────────────────────
step "2/6 building contracts"
if [ "${FRESH}" = "1" ] || [ ! -f "${WASM_PURE}" ] || [ ! -f "${WASM_PREC}" ]; then
  bash "${HERE}/build-zk-verifier.sh"
else
  step "  artefacts exist; skipping (FRESH=1 to force rebuild)"
fi

# ── 3. Deploy ─────────────────────────────────────────────────────────
step "3/6 deploying both variants"
if [ "${FRESH}" = "1" ] || [ ! -f "${DEPLOY_ENV}" ]; then
  # deploy-oneshot.sh runs INSIDE the container (bare `junod`, /tmp paths).
  # Stage the wasm + script in, exec it, then copy the generated deploy.env
  # back out to the host path that benchmark.sh / smoke-test.sh source.
  step "  staging wasm + deploy script into ${CONTAINER}"
  docker cp "${WASM_PURE}" "${CONTAINER}:/tmp/zk_verifier_pure.wasm"
  docker cp "${WASM_PREC}" "${CONTAINER}:/tmp/zk_verifier_precompile.wasm"
  docker cp "${HERE}/deploy-oneshot.sh" "${CONTAINER}:/tmp/deploy-oneshot.sh"
  step "  running deploy-oneshot.sh inside container"
  docker exec "${CONTAINER}" bash /tmp/deploy-oneshot.sh
  step "  copying deploy.env back to host"
  docker cp "${CONTAINER}:/tmp/deploy.env" "${DEPLOY_ENV}"
  cat "${DEPLOY_ENV}"
else
  step "  deploy.env exists; skipping (FRESH=1 to force redeploy)"
  cat "${DEPLOY_ENV}"
fi

# ── 4. Smoke test ─────────────────────────────────────────────────────
step "4/6 smoke-testing both contracts"
if [ "${SKIP_SMOKE:-0}" != "1" ]; then
  bash "${HERE}/smoke-test.sh"
else
  step "  SKIP_SMOKE=1 — skipping"
fi

# ── 4. + 5. Run benchmark ────────────────────────────────────────────
step "5-6/6 running benchmark (proof generation handled inside benchmark.sh)"
N=${N:-5} bash "${HERE}/benchmark.sh"

# ── Summary ──────────────────────────────────────────────────────────
RESULTS="${REPO_ROOT}/docs/BN254_BENCHMARK_RESULTS.md"
echo ""
echo "════════════════════════════════════════════════════════════"
echo "[reproduce] DONE — results: ${RESULTS}"
echo "════════════════════════════════════════════════════════════"
grep -E '^\| (Pure-Wasm|.{0,5}BN254 precompile)' "${RESULTS}" || true
