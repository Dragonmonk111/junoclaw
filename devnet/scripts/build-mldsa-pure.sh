#!/usr/bin/env bash
# Build the jclaw-credential PURE (in-Wasm) variant for the ML-DSA benchmark.
#
# This is the default-features build: VerifyMlDsaAttestation runs the
# in-contract fips204 verifier and the wasm loads on STOCK Juno (no host
# function required). It is the "pure" baseline that build-mldsa-precompile.sh
# (--features mldsa-precompile, env.ml_dsa_verify host fn) is compared against.
#
# Output: devnet/jclaw_credential_pure.wasm
#
# NOTE: the previous devnet/jclaw_credential_pure.wasm was a stale MAYO-era
# build whose ExecuteMsg predates set_ml_dsa_pk/verify_ml_dsa_attestation, so
# the benchmark failed with "unknown variant `set_ml_dsa_pk`". Re-running this
# script regenerates it from current source.
#
# Uses the cosmwasm/optimizer container for its toolchain + wasm-opt, but
# overrides the entrypoint so the build is feature-explicit (mirrors
# build-mldsa-precompile.sh exactly, minus the --features flag).
set -euo pipefail

REPO="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw"
DEVNET_DIR="${REPO}/devnet"
IMAGE="cosmwasm/optimizer:0.16.1"

docker volume create junoclaw-contracts-target >/dev/null

echo "── Building jclaw-credential PURE (default features, in-Wasm fips204) ──"
docker run --rm \
  -v "${REPO}:/code" \
  -v junoclaw-contracts-target:/target \
  --entrypoint /bin/sh \
  "${IMAGE}" \
  -c '
    set -e
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    echo "Building default features (no mldsa-precompile)..."
    RUSTFLAGS="-C link-arg=-s" cargo build \
      --release \
      --target wasm32-unknown-unknown \
      --manifest-path /code/contracts/jclaw-credential/Cargo.toml \
      --target-dir /target
    echo "Running wasm-opt (signext-lowering for the 1.78 toolchain)..."
    wasm-opt -Os --signext-lowering \
      /target/wasm32-unknown-unknown/release/jclaw_credential.wasm \
      -o /code/artifacts/jclaw_credential_pure.wasm
    echo "Done. Size:"
    ls -lh /code/artifacts/jclaw_credential_pure.wasm
  '

cp "${REPO}/artifacts/jclaw_credential_pure.wasm" "${DEVNET_DIR}/jclaw_credential_pure.wasm"
echo ""
ls -lh "${DEVNET_DIR}/jclaw_credential_pure.wasm"

# Sanity: the PURE build MUST NOT import env.ml_dsa_verify. If it does, the
# default-features build accidentally pulled in the precompile path.
echo ""
echo "── Import check ──"
if grep -aq "ml_dsa_verify" "${DEVNET_DIR}/jclaw_credential_pure.wasm"; then
  echo "ERROR: jclaw_credential_pure.wasm imports ml_dsa_verify — it should NOT (pure build)" >&2
  exit 1
else
  echo "OK: jclaw_credential_pure.wasm does not import ml_dsa_verify (in-Wasm verifier)"
fi
echo "Build complete."
