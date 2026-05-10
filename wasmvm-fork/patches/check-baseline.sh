#!/usr/bin/env bash
# check-baseline.sh
#
# Lightweight companion to rebase-track-a.sh. Where rebase-track-a.sh does the
# full clone-apply-test loop (slow), this one only answers a single question:
#
#   "Do the v2.2.2 patches still apply cleanly to <upstream tag X>?"
#
# It runs `git apply --check` per patch (no working-tree mutation, no cargo
# test). Useful when an upstream version bump appears and you want a 30-second
# answer about whether re-rebasing is needed before booking a full rebuild.
#
# Usage:
#   bash wasmvm-fork/patches/check-baseline.sh                  # default v2.2.2
#   bash wasmvm-fork/patches/check-baseline.sh v2.2.7           # check v2.2.7
#   COSMWASM_TAG=v2.2.6 bash wasmvm-fork/patches/check-baseline.sh
#
# Environment:
#   BUILD_DIR       where the cosmwasm clone lives (default: ${HOME}/junoclaw-build)
#   COSMWASM_TAG    upstream tag to check against (also positional arg $1)
#   PATCH_DIR       which patch series to apply (default: <repo>/wasmvm-fork/patches/v2.2.2)
#
# Exit codes:
#   0  every patch applies cleanly
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
COSMWASM_TAG="${COSMWASM_TAG:-${1:-v2.2.2}}"
PATCH_DIR="${PATCH_DIR:-${JUNOCLAW_ROOT}/wasmvm-fork/patches/v2.2.2}"
COSMWASM_DIR="${BUILD_DIR}/cosmwasm-bn254"

mkdir -p "${BUILD_DIR}"

echo "=== check-baseline ==="
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

# Reset cleanly to the requested tag. Suppress output unless something fails.
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

for patch in "${PATCH_DIR}"/*.patch; do
  name="$(basename "${patch}")"
  if ( cd "${COSMWASM_DIR}" && git apply --check "${patch}" 2>/dev/null ); then
    echo "  CLEAN     ${name}"
    CLEAN+=("${name}")
  else
    echo "  CONFLICT  ${name}"
    CONFLICTS+=("${name}")
  fi
done

echo ""
echo "summary: ${#CLEAN[@]} clean / ${#CONFLICTS[@]} conflicts (target=${COSMWASM_TAG})"

if [[ ${#CONFLICTS[@]} -gt 0 ]]; then
  echo ""
  echo "Conflicts (failed patches):"
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

echo "OK — patch series applies cleanly to ${COSMWASM_TAG}."
