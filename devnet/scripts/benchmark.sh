#!/usr/bin/env bash
# Runs N VerifyProof txs against both deployed variants, records gas
# used, and emits docs/BN254_BENCHMARK_RESULTS.md.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

if [ ! -f "${DEVNET_DIR}/deploy.env" ]; then
  echo "error: deploy.env missing — run deploy-zk-verifier.sh first" >&2
  exit 1
fi
# shellcheck disable=SC1091
source "${DEVNET_DIR}/deploy.env"

N=${N:-50}
KEYRING=${KEYRING_BACKEND:-test}
OUT="${REPO_ROOT}/docs/BN254_BENCHMARK_RESULTS.md"

# Provide a fresh VK + proof bundle via the bridge helper. Both flavours
# must see the same bytes so the before/after gas comparison is valid.
BENCH_TS="${REPO_ROOT}/wavs/bridge/src/benchmark-zk-verifier-devnet.ts"
if [ ! -f "${BENCH_TS}" ]; then
  echo "error: ${BENCH_TS} missing — run the TypeScript benchmark harness first" >&2
  exit 2
fi

echo "[bench] Generating VK + proof bundle via bridge harness…"
( cd "${REPO_ROOT}/wavs/bridge" && \
    npm run benchmark-zk-verifier-devnet -- \
        --node "${NODE}" \
        --chain-id "${CHAIN_ID}" \
        --admin "${ADMIN_ADDR}" \
        --pure-addr "${PURE_ADDR}" \
        --precompile-addr "${PRECOMPILE_ADDR}" \
        --samples "${N}" \
        --out "${OUT}" )

echo "[bench] Results written to ${OUT}"
