#!/usr/bin/env bash
# Build the jclaw-credential precompile variant with --features mayo-precompile.
# The resulting .wasm routes VerifyMayoAttestation through the `mayo_verify`
# host function and therefore only loads on a MAYO-patched wasmvm-fork chain
# (see wasmvm-fork/patches/v2.2.2/10-19). The default (pure-Wasm) build is
# produced by build-contracts-docker.sh and runs on stock Juno.
#
# Uses the cosmwasm/optimizer container for its toolchain + wasm-opt, but
# overrides the entrypoint so we can pass --features correctly.
set -euo pipefail

REPO="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw"
DEVNET_DIR="${REPO}/devnet"
IMAGE="cosmwasm/optimizer:0.16.1"

docker volume create junoclaw-contracts-target >/dev/null

echo "── Building jclaw-credential with --features mayo-precompile ──"
docker run --rm \
  -v "${REPO}:/code" \
  -v junoclaw-contracts-target:/target \
  --entrypoint /bin/sh \
  "${IMAGE}" \
  -c '
    set -e
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    echo "Building with features mayo-precompile..."
    RUSTFLAGS="-C link-arg=-s" cargo build \
      --release \
      --target wasm32-unknown-unknown \
      --manifest-path /code/contracts/jclaw-credential/Cargo.toml \
      --features mayo-precompile \
      --target-dir /target
    echo "Running wasm-opt (signext-lowering for the 1.78 toolchain)..."
    wasm-opt -Os --signext-lowering \
      /target/wasm32-unknown-unknown/release/jclaw_credential.wasm \
      -o /code/artifacts/jclaw_credential_precompile.wasm
    echo "Done. Size:"
    ls -lh /code/artifacts/jclaw_credential_precompile.wasm
  '

cp "${REPO}/artifacts/jclaw_credential_precompile.wasm" "${DEVNET_DIR}/jclaw_credential_precompile.wasm"
echo ""
ls -lh "${DEVNET_DIR}/jclaw_credential_precompile.wasm"

# Sanity: the precompile build MUST import env.mayo_verify, and the pure build
# MUST NOT. We grep the raw wasm for the import name (the symbol survives
# wasm-opt because it is an imported function name in the import section).
echo ""
echo "── Import check ──"
if grep -aq "mayo_verify" "${DEVNET_DIR}/jclaw_credential_precompile.wasm"; then
  echo "OK: jclaw_credential_precompile.wasm imports mayo_verify"
else
  echo "ERROR: mayo_verify import not found — precompile feature did not take effect" >&2
  exit 1
fi
echo "Build complete."
