#!/usr/bin/env bash
# Build ONLY the precompile variant with the bn254-precompile feature.
# Uses the cosmwasm/optimizer container for its toolchain + wasm-opt,
# but overrides the entrypoint to pass --features correctly.
set -euo pipefail

REPO="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw"
DEVNET_DIR="${REPO}/devnet"
IMAGE="cosmwasm/optimizer:0.16.1"

docker volume create junoclaw-contracts-target >/dev/null

echo "── Building zk-verifier with --features bn254-precompile ──"
docker run --rm \
  -v "${REPO}:/code" \
  -v junoclaw-contracts-target:/target \
  --entrypoint /bin/sh \
  "${IMAGE}" \
  -c '
    set -e
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    echo "Building with features bn254-precompile..."
    RUSTFLAGS="-C link-arg=-s" cargo build \
      --release \
      --target wasm32-unknown-unknown \
      --manifest-path /code/contracts/zk-verifier/Cargo.toml \
      --features bn254-precompile \
      --target-dir /target
    echo "Running wasm-opt..."
    wasm-opt -Os --signext-lowering \
      /target/wasm32-unknown-unknown/release/zk_verifier.wasm \
      -o /code/artifacts/zk_verifier_precompile.wasm
    echo "Done. Size:"
    ls -lh /code/artifacts/zk_verifier_precompile.wasm
  '

cp "${REPO}/artifacts/zk_verifier_precompile.wasm" "${DEVNET_DIR}/zk_verifier_precompile.wasm"
echo ""
ls -lh "${DEVNET_DIR}/zk_verifier_precompile.wasm"
echo "Build complete."
