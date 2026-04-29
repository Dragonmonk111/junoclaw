#!/usr/bin/env bash
# Build the zk-verifier contract (both flavours) using the canonical
# cosmwasm/optimizer image. Writes:
#
#   devnet/zk_verifier_pure.wasm
#   devnet/zk_verifier_precompile.wasm
#
# Why cosmwasm/optimizer (not plain `cargo build`):
#   1. rustc ≥ 1.82 emits `call_indirect` with a 5-byte LEB128 table-
#      index padding from the reference-types proposal. cosmwasm-vm's
#      MVP-only parser rejects this with "reference-types not enabled:
#      zero byte expected at offset 0x7d873". Plain RUSTFLAGS=
#      target-feature=-reference-types does NOT collapse the padding.
#   2. The optimizer pipes the cargo output through `wasm-opt` from
#      binaryen, which rewrites the binary into strict MVP form and
#      strips debug sections. This is what every production cosmwasm
#      contract (including the uni-7 baseline we measured at 371 486
#      gas) is built with — so using it here keeps the comparison
#      apples-to-apples.
#   3. Reproducibility: the image is content-addressed by tag, so the
#      same source produces byte-identical .wasm across machines.
#
# Two passes — once without features, once with --features
# bn254-precompile. The optimizer always writes to /code/artifacts/
# inside the container.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

# 0.16.x supports cosmwasm-std v2.x (which the workspace uses).
OPTIMIZER_TAG="${OPTIMIZER_TAG:-0.16.1}"
IMAGE="cosmwasm/optimizer:${OPTIMIZER_TAG}"

# Target dir as a named volume — same rationale as the devnet build:
# avoids root-vs-WSL-user permission churn and 9P I/O overhead.
docker volume create junoclaw-contracts-target >/dev/null

run_optimizer() {
  local features="$1"
  local feat_args=()
  if [ -n "${features}" ]; then
    feat_args=(--features "${features}")
  fi
  # The optimizer expects the workspace at /code, target dir at /target,
  # and writes to /code/artifacts. We pass extra cargo flags through.
  docker run --rm \
    -v "${REPO_ROOT}:/code" \
    -v junoclaw-contracts-target:/target \
    "${IMAGE}" \
    "/code/contracts/zk-verifier" "${feat_args[@]}"
}

echo "── Pass 1/2: pure-Wasm baseline (no features) ──"
run_optimizer ""
cp "${REPO_ROOT}/artifacts/zk_verifier.wasm" "${DEVNET_DIR}/zk_verifier_pure.wasm"

echo
echo "── Pass 2/2: --features bn254-precompile ──"
run_optimizer "bn254-precompile"
cp "${REPO_ROOT}/artifacts/zk_verifier.wasm" "${DEVNET_DIR}/zk_verifier_precompile.wasm"

echo
ls -lh "${DEVNET_DIR}"/zk_verifier_*.wasm
echo
echo "[build-docker] Artefacts ready. Run deploy with BUILD=0:"
echo "  BUILD=0 bash ${DEVNET_DIR}/scripts/deploy-zk-verifier.sh"
