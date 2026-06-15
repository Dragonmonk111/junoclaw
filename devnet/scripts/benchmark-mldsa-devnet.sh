#!/usr/bin/env bash
# Deploys the available jclaw-credential flavours (pure + ML-DSA precompile) to
# the running ML-DSA-patched devnet and benchmarks VerifyMlDsaAttestation gas
# for each variant (ML-DSA-44 / 65 / 87).
#
# Exports the in-container `admin` privkey (test keyring) so cosmjs can sign,
# then runs deploy/benchmark-mldsa-devnet.cjs. Flavours whose wasm is missing
# are skipped, so you can gather the precompile number alone.
#
#   precompile wasm:  devnet/scripts/build-mldsa-precompile.sh  -> jclaw_credential_mldsa.wasm
#   pure wasm:        devnet/jclaw_credential_pure.wasm (default-features build, optional)
#
# Env overrides: CONTAINER, RPC, CHAIN_ID, GAS_PRICE, FRESH=1 (wipe prior results).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
KEYRING="${KEYRING_BACKEND:-test}"
RESULTS="${REPO_ROOT}/deploy/mldsa-devnet-benchmark-results.json"

if [ "${FRESH:-0}" = "1" ] && [ -f "${RESULTS}" ]; then
  echo "[bench] FRESH=1 — removing prior ${RESULTS}"
  rm -f "${RESULTS}"
fi

if [ ! -f "${DEVNET_DIR}/jclaw_credential_mldsa.wasm" ] \
   && [ ! -f "${DEVNET_DIR}/jclaw_credential_pure.wasm" ]; then
  echo "error: no flavour wasm found in ${DEVNET_DIR}" >&2
  echo "  build the precompile flavour first:" >&2
  echo "    bash ${HERE}/build-mldsa-precompile.sh" >&2
  exit 2
fi

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
exec node benchmark-mldsa-devnet.cjs
