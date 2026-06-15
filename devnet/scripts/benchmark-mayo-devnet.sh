#!/usr/bin/env bash
# Deploys both jclaw-credential flavours (pure + precompile) to the running
# MAYO devnet and benchmarks VerifyMayoAttestation gas for each variant.
#
# Exports the in-container `admin` privkey (test keyring) so cosmjs can sign,
# then runs deploy/benchmark-mayo-devnet.cjs.
#
# Env overrides: CONTAINER, RPC, CHAIN_ID, GAS_PRICE, FRESH=1 (wipe prior results).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
KEYRING="${KEYRING_BACKEND:-test}"
RESULTS="${REPO_ROOT}/deploy/mayo-devnet-benchmark-results.json"

if [ "${FRESH:-0}" = "1" ] && [ -f "${RESULTS}" ]; then
  echo "[bench] FRESH=1 — removing prior ${RESULTS}"
  rm -f "${RESULTS}"
fi

for f in jclaw_credential_pure.wasm jclaw_credential_precompile.wasm; do
  if [ ! -f "${DEVNET_DIR}/${f}" ]; then
    echo "error: ${DEVNET_DIR}/${f} missing — build both flavours first" >&2
    exit 2
  fi
done

echo "[bench] Exporting admin privkey from ${CONTAINER}…"
ADMIN_PRIVKEY=$(echo 'y' | docker exec -i "${CONTAINER}" \
    junod keys export admin --unarmored-hex --unsafe \
        --keyring-backend "${KEYRING}" --home /root/.juno 2>/dev/null | tail -n1)
if [ -z "${ADMIN_PRIVKEY}" ]; then
  echo "error: failed to export admin privkey from ${CONTAINER}" >&2
  exit 3
fi
export ADMIN_PRIVKEY

cd "${REPO_ROOT}/deploy"
exec node benchmark-mayo-devnet.cjs
