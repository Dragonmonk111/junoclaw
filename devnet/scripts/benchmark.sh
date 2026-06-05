#!/usr/bin/env bash
# Runs N VerifyProof txs against both deployed variants, records gas
# used, and emits docs/BN254_BENCHMARK_RESULTS.md.
#
# Auto-prepares the prerequisites that are easy to miss:
#   - generates a Groth16 proof bundle into ${REPO_ROOT}/tmpdir/groth16_proof.json
#   - exports the on-chain admin (validator) privkey from the running container
#     into WAVS_OPERATOR_PRIVKEY if neither WAVS_OPERATOR_PRIVKEY nor
#     WAVS_OPERATOR_MNEMONIC is set
#
# Honoured env vars (override-friendly):
#   N                        sample count per variant (default 5)
#   CONTAINER                docker container name (default junoclaw-bn254-devnet)
#   WAVS_OPERATOR_PRIVKEY    hex privkey for the admin signer
#   WAVS_OPERATOR_MNEMONIC   alternative to PRIVKEY
#   ZK_PROOF_PATH            path to a pre-generated proof bundle
#   KEEP_PROOF=1             skip regeneration if ZK_PROOF_PATH already exists

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"
REPO_ROOT="$(cd "${DEVNET_DIR}/.." && pwd)"

if [ ! -f "${DEVNET_DIR}/deploy.env" ]; then
  echo "error: deploy.env missing — run deploy-now.sh first" >&2
  exit 1
fi
# shellcheck disable=SC1091
source "${DEVNET_DIR}/deploy.env"

N=${N:-5}
CONTAINER=${CONTAINER:-junoclaw-bn254-devnet}
KEYRING=${KEYRING_BACKEND:-test}
OUT="${REPO_ROOT}/docs/BN254_BENCHMARK_RESULTS.md"

BENCH_TS="${REPO_ROOT}/wavs/bridge/src/benchmark-zk-verifier-devnet.ts"
if [ ! -f "${BENCH_TS}" ]; then
  echo "error: ${BENCH_TS} missing — bridge harness not on disk" >&2
  exit 2
fi

# ── Auto-export validator privkey if no signer credentials provided ──
# The devnet keys are created with --no-backup so no mnemonic exists; the
# `validator` key is the on-chain admin of both contracts (deploy-now.sh
# uses --from validator).
if [ -z "${WAVS_OPERATOR_PRIVKEY:-}" ] && [ -z "${WAVS_OPERATOR_MNEMONIC:-}" ]; then
  echo "[bench] No signer credentials set; exporting admin privkey from ${CONTAINER}…"
  WAVS_OPERATOR_PRIVKEY=$(echo 'y' | docker exec -i "${CONTAINER}" \
      junod keys export admin \
          --unarmored-hex --unsafe \
          --keyring-backend "${KEYRING}" \
          --home /root/.juno 2>/dev/null | tail -n1)
  if [ -z "${WAVS_OPERATOR_PRIVKEY}" ]; then
    echo "error: failed to export admin privkey from ${CONTAINER}" >&2
    exit 3
  fi
  export WAVS_OPERATOR_PRIVKEY
fi

# ── Ensure proof bundle exists ──
DEFAULT_PROOF="${REPO_ROOT}/tmpdir/groth16_proof.json"
ZK_PROOF_PATH=${ZK_PROOF_PATH:-${DEFAULT_PROOF}}
export ZK_PROOF_PATH

if [ ! -f "${ZK_PROOF_PATH}" ] || [ "${KEEP_PROOF:-0}" != "1" ]; then
  echo "[bench] Generating Groth16 proof bundle → ${ZK_PROOF_PATH}…"
  mkdir -p "$(dirname "${ZK_PROOF_PATH}")"
  # Ensure cargo is on PATH (non-login shells don't source ~/.cargo/env).
  if ! command -v cargo >/dev/null 2>&1; then
    for c in "${CARGO_HOME:-$HOME/.cargo}/bin" /root/.cargo/bin; do
      if [ -x "$c/cargo" ]; then export PATH="$c:$PATH"; break; fi
    done
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found on PATH; install rustup or set CARGO_HOME" >&2
    exit 4
  fi
  ( cd "${REPO_ROOT}/contracts/zk-verifier" && \
      PROOF_OUTPUT="${ZK_PROOF_PATH}" \
      cargo run --quiet --example generate_proof >/dev/null )
fi

echo "[bench] Running ${N} samples against:"
echo "         pure-addr = ${PURE_ADDR}"
echo "         prec-addr = ${PRECOMPILE_ADDR}"
echo "         signer    = ${ADMIN_ADDR}"
echo "         node      = ${NODE}"

( cd "${REPO_ROOT}/wavs/bridge" && \
    npx tsx src/benchmark-zk-verifier-devnet.ts \
        --node "${NODE}" \
        --chain-id "${CHAIN_ID}" \
        --admin "${ADMIN_ADDR}" \
        --pure-addr "${PURE_ADDR}" \
        --precompile-addr "${PRECOMPILE_ADDR}" \
        --samples "${N}" \
        --out "${OUT}" )

echo "[bench] Results written to ${OUT}"
