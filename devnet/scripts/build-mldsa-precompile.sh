#!/usr/bin/env bash
# Build the jclaw-credential precompile variant with --features mldsa-precompile.
# The resulting .wasm routes VerifyMlDsaAttestation through the `ml_dsa_verify`
# host function and therefore only loads on an ML-DSA-patched wasmvm-fork chain
# (see wasmvm-fork/patches/v2.2.2/20-28). The default (pure-Wasm) build is
# produced by build-contracts-docker.sh and runs the in-contract fips204
# verifier on stock Juno.
#
# Uses the cosmwasm/optimizer container for its toolchain + wasm-opt, but
# overrides the entrypoint so we can pass --features correctly.
set -euo pipefail

REPO="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw"
DEVNET_DIR="${REPO}/devnet"
IMAGE="cosmwasm/optimizer:0.16.1"

docker volume create junoclaw-contracts-target >/dev/null

echo "── Building jclaw-credential with --features mldsa-precompile ──"
docker run --rm \
  -v "${REPO}:/code" \
  -v junoclaw-contracts-target:/target \
  --entrypoint /bin/sh \
  "${IMAGE}" \
  -c '
    set -e
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    echo "Building with features mldsa-precompile..."
    RUSTFLAGS="-C link-arg=-s" cargo build \
      --release \
      --target wasm32-unknown-unknown \
      --manifest-path /code/contracts/jclaw-credential/Cargo.toml \
      --features mldsa-precompile \
      --target-dir /target
    echo "Running wasm-opt (signext-lowering for the 1.78 toolchain)..."
    wasm-opt -Os --signext-lowering \
      /target/wasm32-unknown-unknown/release/jclaw_credential.wasm \
      -o /code/artifacts/jclaw_credential_mldsa.wasm
    echo "Done. Size:"
    ls -lh /code/artifacts/jclaw_credential_mldsa.wasm
  '

cp "${REPO}/artifacts/jclaw_credential_mldsa.wasm" "${DEVNET_DIR}/jclaw_credential_mldsa.wasm"
echo ""
ls -lh "${DEVNET_DIR}/jclaw_credential_mldsa.wasm"

# Sanity: the precompile build MUST import env.ml_dsa_verify. The symbol
# survives wasm-opt because it is an imported function name in the import
# section.
echo ""
echo "── Import check ──"
if grep -aq "ml_dsa_verify" "${DEVNET_DIR}/jclaw_credential_mldsa.wasm"; then
  echo "OK: jclaw_credential_mldsa.wasm imports ml_dsa_verify"
else
  echo "ERROR: ml_dsa_verify import not found — precompile feature did not take effect" >&2
  exit 1
fi
echo "Build complete."
