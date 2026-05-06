#!/usr/bin/env bash
# rebase-track-a.sh
#
# Track A verification harness for the BN254 patch set against
# `cosmwasm` v2.2.2 (the version `wasmvm` v2.2.4 / Juno mainnet pins).
#
# WHAT THIS SCRIPT DOES (current behaviour, post-2026-05-06):
#
#   1. Clones (or updates) `CosmWasm/cosmwasm` into ${BUILD_DIR}/cosmwasm-bn254
#   2. Resets that checkout cleanly to tag `v2.2.2`
#   3. Applies `wasmvm-fork/patches/v2.2.2/*.patch` in numeric order (00-09)
#   4. Pins Rust 1.78.0 (installs via rustup if needed)
#   5. Runs `cargo +1.78.0 test -p cosmwasm-crypto-bn254` and `-p cosmwasm-vm --lib`
#   6. Reports pass/fail
#
# WHAT IT NO LONGER DOES (history):
#
#   * Does NOT clone wasmvm or apply Go-wrapper patches. Inspection of
#     `wasmvm` v2.2.4's `lib_libwasmvm.go` and `internal/api/bindings.h`
#     confirmed that BLS12-381 itself has no Go-side wrappers in this
#     version — the original BN254 wasmvm patches were trying to mirror a
#     pattern that doesn't exist on this branch. Go wrappers are deferred
#     to Track B (cosmwasm/wasmvm v3.x). The dropped patches are kept at
#     `wasmvm-fork/patches/wasmvm.*.patch.dropped` for audit.
#   * Does NOT capture / regenerate patches. The `v2.2.2/` set was captured
#     in-session on 2026-05-06 (see `v2.2.2/README.md`). If a future
#     `wasmvm` bump (e.g. v2.2.5, v2.3.x) requires a re-rebase, run
#     `cargo test`, hand-fix any drift, and `git diff` the result.
#
# Prerequisites:
#   * bash, git, curl
#   * rustup (cargo will be sourced via `source ~/.cargo/env`)
#   * Network access to github.com (for the cosmwasm clone and rustup)
#   * Run from the `junoclaw/` repo root
#
# Usage:
#   bash wasmvm-fork/patches/rebase-track-a.sh
#
# Environment:
#   BUILD_DIR    where the cosmwasm clone lives (default: ${HOME}/junoclaw-build)
#   COSMWASM_TAG override the cosmwasm tag (default: v2.2.2)
#   RUST_VERSION override the pinned toolchain (default: 1.78.0)
#
# Exit codes:
#   0  all tests pass
#   1  precondition failed (missing tool, wrong cwd, etc.)
#   2  clone or checkout failed
#   3  patch application failed
#   4  toolchain install failed
#   5  test failure

set -euo pipefail

# ----- 0. Preconditions ----------------------------------------------------

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

# Source cargo env first so non-interactive shells find rustup/cargo on PATH
# before we check for them.
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi

for tool in git curl rustup; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "ERROR: missing required tool: $tool" >&2
    exit 1
  fi
done

JUNOCLAW_ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
COSMWASM_TAG="${COSMWASM_TAG:-v2.2.2}"
RUST_VERSION="${RUST_VERSION:-1.78.0}"
PATCH_DIR="${JUNOCLAW_ROOT}/wasmvm-fork/patches/v2.2.2"

mkdir -p "${BUILD_DIR}"

if [[ ! -d "${PATCH_DIR}" ]]; then
  echo "ERROR: patch directory not found: ${PATCH_DIR}" >&2
  echo "       Did you mean to run this from a checkout that has the v2.2.2 patches?" >&2
  exit 1
fi

echo "JunoClaw root:    ${JUNOCLAW_ROOT}"
echo "Build dir:        ${BUILD_DIR}"
echo "cosmwasm tag:     ${COSMWASM_TAG}"
echo "Rust toolchain:   ${RUST_VERSION}"
echo "Patch dir:        ${PATCH_DIR}"
echo ""

# ----- 1. Clone or update upstream cosmwasm --------------------------------

COSMWASM_DIR="${BUILD_DIR}/cosmwasm-bn254"

clone_or_update() {
  local repo_url="$1"
  local target_dir="$2"
  if [[ -d "${target_dir}/.git" ]]; then
    echo "Updating ${target_dir}..."
    ( cd "${target_dir}" && git fetch --tags origin )
  else
    echo "Cloning ${repo_url}..."
    git clone "${repo_url}" "${target_dir}"
  fi
}

clone_or_update "https://github.com/CosmWasm/cosmwasm" "${COSMWASM_DIR}" || exit 2

# Reset to a clean state at COSMWASM_TAG
checkout_clean() {
  local dir="$1"
  local tag="$2"
  ( cd "${dir}" && \
      git reset --hard >/dev/null && \
      git clean -fdx >/dev/null && \
      git checkout "${tag}" 2>/dev/null ) || {
    echo "ERROR: failed to checkout ${tag} in ${dir}" >&2
    return 2
  }
}

checkout_clean "${COSMWASM_DIR}" "${COSMWASM_TAG}" || exit 2
echo "cosmwasm checked out at ${COSMWASM_TAG}"

# ----- 2. Apply v2.2.2 patches in numeric order ---------------------------

echo ""
echo "Applying patches from ${PATCH_DIR}..."

PATCH_FAILED=0
for patch in "${PATCH_DIR}"/*.patch; do
  patch_name="$(basename "${patch}")"
  echo "  applying ${patch_name}..."
  if ! ( cd "${COSMWASM_DIR}" && git apply "${patch}" ); then
    echo "  CONFLICT: ${patch_name}" >&2
    PATCH_FAILED=1
  fi
done

if [[ ${PATCH_FAILED} -eq 1 ]]; then
  echo ""
  echo "==============================="
  echo "PATCH APPLICATION FAILED"
  echo "==============================="
  echo "One or more patches did not apply cleanly. This usually means"
  echo "upstream cosmwasm has drifted between v2.2.2 and the patch base."
  echo ""
  echo "To inspect:"
  echo "  cd ${COSMWASM_DIR}"
  echo "  git apply --3way ${PATCH_DIR}/<failed-patch>"
  echo ""
  echo "To regenerate a patch after manual fix-up:"
  echo "  ( cd ${COSMWASM_DIR} && git diff <file> ) > ${PATCH_DIR}/<patch>"
  exit 3
fi

# ----- 3. Pin Rust toolchain ----------------------------------------------

echo ""
echo "Ensuring Rust ${RUST_VERSION} is installed..."
if ! rustup toolchain list | grep -q "^${RUST_VERSION}-"; then
  rustup install "${RUST_VERSION}" || {
    echo "ERROR: failed to install Rust ${RUST_VERSION}" >&2
    exit 4
  }
fi
echo "Rust ${RUST_VERSION} is available."

# ----- 4. Run tests --------------------------------------------------------

echo ""
echo "Running cargo +${RUST_VERSION} test -p cosmwasm-crypto-bn254..."
if ! ( cd "${COSMWASM_DIR}" && cargo "+${RUST_VERSION}" test -p cosmwasm-crypto-bn254 ); then
  echo "ERROR: cosmwasm-crypto-bn254 tests failed" >&2
  exit 5
fi

echo ""
echo "Running cargo +${RUST_VERSION} test -p cosmwasm-vm --lib..."
if ! ( cd "${COSMWASM_DIR}" && cargo "+${RUST_VERSION}" test -p cosmwasm-vm --lib ); then
  echo "ERROR: cosmwasm-vm tests failed" >&2
  exit 5
fi

# ----- 5. Done -------------------------------------------------------------

echo ""
echo "==============================="
echo "TRACK A VERIFICATION COMPLETE"
echo "==============================="
echo "cosmwasm tag:     ${COSMWASM_TAG}"
echo "Rust toolchain:   ${RUST_VERSION}"
echo "Patches applied:  $(ls "${PATCH_DIR}"/*.patch | wc -l)"
echo ""
echo "All tests pass. The BN254 patch set is healthy on this baseline."
echo ""
echo "Next steps in the post-vote execution plan:"
echo "  * Phase 0.2 — wire the host functions into a devnet build"
echo "  * Phase 0.3 — measure end-to-end gas with the EIP-1108 vector suite"
echo "  * Phase 1   — open the upstream issue (see docs/UPSTREAM_ISSUE_DRAFTS.md)"
