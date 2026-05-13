#!/usr/bin/env bash
# check-baseline-v3.sh
#
# Track-B forward-port companion to check-baseline.sh. Same one-question
# answer ("do the patches still apply cleanly?") but targets v3.0.x
# cosmwasm/wasmvm tags by default and pulls the patch series from
# `wasmvm-fork/patches/v2.2.7/` (since the v3 patch series doesn't exist
# yet — that's what this script is the first step toward producing).
#
# Usage:
#   bash wasmvm-fork/patches/check-baseline-v3.sh                    # default v3.0.1
#   bash wasmvm-fork/patches/check-baseline-v3.sh v3.0.0             # check v3.0.0
#   COSMWASM_TAG=v3.0.1 PATCH_DIR=... bash check-baseline-v3.sh
#
# Environment:
#   BUILD_DIR       where the cosmwasm clone lives (default: ${HOME}/junoclaw-build)
#   COSMWASM_TAG    upstream tag to check against (also positional arg $1, default v3.0.1)
#   PATCH_DIR       which patch series to apply (default: <repo>/wasmvm-fork/patches/v2.2.7)
#
# Exit codes:
#   0  every patch applies cleanly (unlikely on v3 — would mean no forward-port needed)
#   1  precondition failed
#   2  clone or checkout failed
#   3  one or more patches reported conflicts (full report on stdout)

set -euo pipefail

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

JUNOCLAW_ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
COSMWASM_TAG="${COSMWASM_TAG:-${1:-v3.0.1}}"
PATCH_DIR="${PATCH_DIR:-${JUNOCLAW_ROOT}/wasmvm-fork/patches/v2.2.7}"
COSMWASM_DIR="${BUILD_DIR}/cosmwasm-bn254"

mkdir -p "${BUILD_DIR}"

echo "=== check-baseline-v3 ==="
echo "  cosmwasm tag : ${COSMWASM_TAG}"
echo "  patch dir    : ${PATCH_DIR}"
echo "  build dir    : ${BUILD_DIR}"
echo ""

if [[ ! -d "${PATCH_DIR}" ]]; then
  echo "ERROR: patch directory not found: ${PATCH_DIR}" >&2
  exit 1
fi

# ----- 1. Clone or update upstream cosmwasm --------------------------------

if [[ -d "${COSMWASM_DIR}/.git" ]]; then
  ( cd "${COSMWASM_DIR}" && git fetch --tags --depth=1 origin "tag" "${COSMWASM_TAG}" >/dev/null 2>&1 )
else
  echo "cloning cosmwasm into ${COSMWASM_DIR}..."
  git clone --quiet "https://github.com/CosmWasm/cosmwasm" "${COSMWASM_DIR}" || exit 2
fi

( cd "${COSMWASM_DIR}" && \
    git fetch --tags --depth=1 origin "tag" "${COSMWASM_TAG}" >/dev/null 2>&1 || true ) || exit 2
( cd "${COSMWASM_DIR}" && \
    git reset --hard "${COSMWASM_TAG}" >/dev/null 2>&1 && \
    git clean -fdx >/dev/null 2>&1 ) || {
      echo "ERROR: failed to reset cosmwasm checkout to ${COSMWASM_TAG}" >&2
      exit 2
    }

echo "cosmwasm checked out at ${COSMWASM_TAG} ($(cd "${COSMWASM_DIR}" && git rev-parse --short HEAD))"
echo ""

# ----- 2. git apply --check per patch --------------------------------------

CONFLICTS=()
CLEAN=()
THREE_WAY_OK=()

for patch in "${PATCH_DIR}"/*.patch; do
  name="$(basename "${patch}")"
  if ( cd "${COSMWASM_DIR}" && git apply --check "${patch}" 2>/dev/null ); then
    echo "  CLEAN     ${name}"
    CLEAN+=("${name}")
  else
    # try a 3-way merge attempt (read-only via --check + --3way)
    if ( cd "${COSMWASM_DIR}" && git apply --check --3way "${patch}" 2>/dev/null ); then
      echo "  3WAY-OK   ${name}    (clean 3-way merge possible)"
      THREE_WAY_OK+=("${name}")
    else
      echo "  CONFLICT  ${name}    (manual rewrite required)"
      CONFLICTS+=("${name}")
    fi
  fi
done

echo ""
echo "summary: ${#CLEAN[@]} clean / ${#THREE_WAY_OK[@]} 3-way-ok / ${#CONFLICTS[@]} conflicts (target=${COSMWASM_TAG})"

if [[ ${#CONFLICTS[@]} -gt 0 ]]; then
  echo ""
  echo "Conflicts (failed patches — rewrite needed):"
  for n in "${CONFLICTS[@]}"; do
    echo "  - ${n}"
  done
  echo ""
  echo "To inspect a specific conflict:"
  echo "  ( cd ${COSMWASM_DIR} && git apply --3way ${PATCH_DIR}/<patch-name> )"
  echo ""
  echo "To regenerate a patch after manual fix-up:"
  echo "  ( cd ${COSMWASM_DIR} && git diff packages/<file> ) > ${PATCH_DIR}/<patch>"
  exit 3
fi

if [[ ${#THREE_WAY_OK[@]} -gt 0 ]]; then
  echo ""
  echo "3-way merge candidates (may apply with --3way but need manual review):"
  for n in "${THREE_WAY_OK[@]}"; do
    echo "  - ${n}"
  done
fi

echo "OK — patch series can be applied to ${COSMWASM_TAG} (modulo any 3-way notes above)."
