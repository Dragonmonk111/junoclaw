#!/usr/bin/env bash
# Boots the BN254 devnet. Idempotent: safe to re-run.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

if [ ! -d "wasmvm-fork" ]; then
  echo "error: wasmvm-fork/ not found at ${REPO_ROOT}" >&2
  exit 1
fi

echo "[run-devnet] Building BN254 devnet image (first build ~12 min)…"
docker compose -f "${DEVNET_DIR}/docker-compose.yml" build

echo "[run-devnet] Starting container…"
docker compose -f "${DEVNET_DIR}/docker-compose.yml" up -d

echo -n "[run-devnet] Waiting for RPC to be reachable "
for i in $(seq 1 60); do
  if curl -fsS http://localhost:26657/status >/dev/null 2>&1; then
    echo " ok"
    break
  fi
  echo -n "."
  sleep 1
  if [ "${i}" = "60" ]; then
    echo " timeout" >&2
    exit 2
  fi
done

echo -n "[run-devnet] Waiting for the first block "
for i in $(seq 1 30); do
  HEIGHT=$(curl -fsS http://localhost:26657/status | jq -r '.result.sync_info.latest_block_height')
  if [ "${HEIGHT}" != "null" ] && [ "${HEIGHT}" -ge 2 ] 2>/dev/null; then
    echo " ok (height=${HEIGHT})"
    break
  fi
  echo -n "."
  sleep 1
  if [ "${i}" = "30" ]; then
    echo " timeout" >&2
    exit 3
  fi
done

echo "[run-devnet] Devnet is live."
echo "  RPC   : http://localhost:26657"
echo "  REST  : http://localhost:1317"
echo "  gRPC  : localhost:9090"
echo ""
echo "Next steps:"
echo "  ${DEVNET_DIR}/scripts/deploy-zk-verifier.sh"
echo "  ${DEVNET_DIR}/scripts/benchmark.sh"
