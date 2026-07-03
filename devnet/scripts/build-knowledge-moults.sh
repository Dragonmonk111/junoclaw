#!/usr/bin/env bash
# Build the knowledge-moults contract using the canonical cosmwasm/optimizer
# image. Writes a single optimized wasm to:
#
#   devnet/knowledge_moults.wasm
#
# Rationale (mirrors build-moultbook.sh):
#   1. rustc >= 1.82 emits call_indirect with a 5-byte LEB128 table-index
#      padding from the reference-types proposal. cosmwasm-vm's MVP-only
#      parser rejects this. RUSTFLAGS=target-feature=-reference-types does
#      NOT collapse the padding; only wasm-opt (via the optimizer image)
#      rewrites the binary into strict MVP form.
#   2. The optimizer also strips debug sections and content-addresses the
#      build for reproducibility across machines.
#
# Single pass — knowledge-moults has no feature flags.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

# 0.16.x supports cosmwasm-std v2.x (which knowledge-moults uses via the
# workspace pin in contracts/Cargo.toml). Override with OPTIMIZER_TAG if
# upstream cuts a v2.2-compatible newer point release.
OPTIMIZER_TAG="${OPTIMIZER_TAG:-0.16.1}"
IMAGE="cosmwasm/optimizer:${OPTIMIZER_TAG}"

# Reuse the same target-dir named volume as build-moultbook.sh so the
# workspace's compiled deps (sha2, cw-storage-plus, etc.) are cached
# between runs.
docker volume create junoclaw-contracts-target >/dev/null

echo "── Building knowledge-moults (cosmwasm/optimizer:${OPTIMIZER_TAG}) ──"
docker run --rm \
  -v "${REPO_ROOT}:/code" \
  -v junoclaw-contracts-target:/target \
  "${IMAGE}" \
  "/code/contracts/knowledge-moults"

# The optimizer writes to /code/artifacts/<crate-name>.wasm.
ARTEFACT_SRC="${REPO_ROOT}/artifacts/knowledge_moults.wasm"
ARTEFACT_DST="${DEVNET_DIR}/knowledge_moults.wasm"

if [ ! -f "${ARTEFACT_SRC}" ]; then
  echo "error: expected ${ARTEFACT_SRC} not produced by optimizer" >&2
  exit 2
fi

cp "${ARTEFACT_SRC}" "${ARTEFACT_DST}"

SIZE_BYTES=$(stat -c %s "${ARTEFACT_DST}" 2>/dev/null || wc -c <"${ARTEFACT_DST}")
SHA256=$(sha256sum "${ARTEFACT_DST}" | awk '{print $1}')

echo
echo "[build-knowledge-moults] Artefact ready:"
printf '  path:    %s\n' "${ARTEFACT_DST}"
printf '  size:    %s bytes (%.1f KiB)\n' "${SIZE_BYTES}" "$(echo "scale=1; ${SIZE_BYTES}/1024" | bc)"
printf '  sha256:  %s\n' "${SHA256}"
echo
echo "[build-knowledge-moults] Next (after A18c-5 passes):"
echo "  cp ${ARTEFACT_DST} <path used by deploy-knowledge-moults.js KNOWLEDGE_MOULTS_WASM>"
echo "  node tools/context-agent/scripts/deploy-knowledge-moults.js"
