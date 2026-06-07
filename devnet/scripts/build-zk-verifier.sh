#!/usr/bin/env bash
# Build both zk-verifier contract variants (pure-Wasm + precompile).
#
# This is the canonical public entrypoint for contract builds. It wraps
# build-contracts-docker.sh (cosmwasm/optimizer) but provides a simpler
# interface and sane defaults.
#
# Writes:
#   devnet/zk_verifier_pure.wasm
#   devnet/zk_verifier_precompile.wasm
#
# Usage:
#   bash devnet/scripts/build-zk-verifier.sh              # idempotent build
#   FORCE=1 bash devnet/scripts/build-zk-verifier.sh    # rebuild even if artefacts exist

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"

WASM_PURE="${DEVNET_DIR}/zk_verifier_pure.wasm"
WASM_PREC="${DEVNET_DIR}/zk_verifier_precompile.wasm"
FORCE=${FORCE:-0}

step() { printf '\n\033[1;36m[build] %s\033[0m\n' "$*"; }

if [ "${FORCE}" != "1" ] && [ -f "${WASM_PURE}" ] && [ -f "${WASM_PREC}" ]; then
  step "Artefacts already exist (FORCE=1 to rebuild):"
  ls -lh "${WASM_PURE}" "${WASM_PREC}"
  exit 0
fi

step "Building both zk-verifier variants via cosmwasm/optimizer…"
bash "${HERE}/build-contracts-docker.sh"

step "Build complete:"
ls -lh "${DEVNET_DIR}"/zk_verifier_*.wasm
