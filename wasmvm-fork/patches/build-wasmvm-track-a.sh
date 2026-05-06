#!/usr/bin/env bash
# build-wasmvm-track-a.sh
#
# Track A build harness for libwasmvm.so against
# `wasmvm` v2.2.4 wired to our patched `cosmwasm` v2.2.2.
#
# Companion to `rebase-track-a.sh`, which prepares the patched cosmwasm
# tree at ${BUILD_DIR}/cosmwasm-bn254. This script consumes that tree
# and produces a `libwasmvm.so` linked against it.
#
# WHAT THIS SCRIPT DOES:
#
#   1. Asserts that ${BUILD_DIR}/cosmwasm-bn254 exists and contains the
#      BN254 patches (looks for packages/crypto-bn254/Cargo.toml as a
#      sentinel for patch 09 having been applied).
#   2. Clones (or updates) `CosmWasm/wasmvm` into ${BUILD_DIR}/wasmvm-bn254
#      and resets that checkout cleanly to tag `v2.2.4`.
#   3. Idempotently appends a `[patch."https://github.com/CosmWasm/cosmwasm.git"]`
#      block to `libwasmvm/Cargo.toml` redirecting `cosmwasm-std` and
#      `cosmwasm-vm` to our patched local copy at
#      ${BUILD_DIR}/cosmwasm-bn254/packages/{std,vm}.
#   4. Runs `cargo +1.78.0 build --release` from libwasmvm/.
#      The Rust 1.78.0 pin is required by `wasmer-vm 4.3.7`'s probestack
#      asm (see `docs/POST_VOTE_EXECUTION_PLAN.md` Phase 0.1 notes).
#   5. Verifies the produced `libwasmvm.so` contains the Track-A symbols:
#         cosmwasm_vm::imports::do_bn254_add
#         cosmwasm_vm::imports::do_bn254_scalar_mul
#         cosmwasm_vm::imports::do_bn254_pairing_equality
#         cosmwasm_crypto_bn254::bn254::bn254_add
#         cosmwasm_crypto_bn254::bn254::bn254_scalar_mul
#         cosmwasm_crypto_bn254::bn254::bn254_pairing_equality
#   6. Reports the path of the .so and copies it into
#      ${BUILD_DIR}/wasmvm-bn254/internal/api/libwasmvm.x86_64.so so that
#      a subsequent `make build-go` (or junod build linking against it)
#      can pick it up via Go's standard cgo location.
#
# WHAT THIS SCRIPT DOES NOT DO:
#
#   * Does NOT modify the wasmvm Go-side wrappers. The BN254 host
#     functions are reachable to Wasm contracts via the wasmer import
#     table only; no new C-ABI is exposed at libwasmvm's boundary.
#     This matches the precompile use case (contract-side imports).
#     For direct Go callers, see Track B (wasmvm v3.x).
#   * Does NOT run any tests. For correctness, see `rebase-track-a.sh`.
#     This script is purely the build chain producing the .so artifact.
#   * Does NOT install rustup; assumes Phase 0.1 / `rebase-track-a.sh`
#     has already pinned Rust 1.78.0.
#
# Prerequisites:
#   * bash, git, awk, nm
#   * rustup with the 1.78.0 toolchain (run rebase-track-a.sh first if not)
#   * ${BUILD_DIR}/cosmwasm-bn254 fully patched (run rebase-track-a.sh first)
#   * Network access to github.com (for the wasmvm clone)
#   * Run from the `junoclaw/` repo root
#
# Usage:
#   bash wasmvm-fork/patches/build-wasmvm-track-a.sh
#
# Environment:
#   BUILD_DIR    where the wasmvm clone lives (default: ${HOME}/junoclaw-build)
#   WASMVM_TAG   override the wasmvm tag (default: v2.2.4)
#   RUST_VERSION override the pinned toolchain (default: 1.78.0)
#
# Exit codes:
#   0  build succeeded and all expected symbols are linked in
#   1  precondition failed (missing tool, wrong cwd, no patched cosmwasm)
#   2  clone or checkout failed
#   3  Cargo.toml patch insertion failed
#   4  cargo build failed
#   5  symbol verification failed (the .so is missing expected functions)

set -euo pipefail

# ----- 0. Preconditions ----------------------------------------------------

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi

for tool in git awk nm rustup cargo; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "ERROR: missing required tool: $tool" >&2
    exit 1
  fi
done

JUNOCLAW_ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
WASMVM_TAG="${WASMVM_TAG:-v2.2.4}"
RUST_VERSION="${RUST_VERSION:-1.78.0}"
COSMWASM_DIR="${BUILD_DIR}/cosmwasm-bn254"
WASMVM_DIR="${BUILD_DIR}/wasmvm-bn254"

# Sanity: cosmwasm-bn254 must already be patched.
SENTINEL="${COSMWASM_DIR}/packages/crypto-bn254/Cargo.toml"
if [[ ! -f "${SENTINEL}" ]]; then
  echo "ERROR: ${SENTINEL} not found." >&2
  echo "       The patched cosmwasm tree at ${COSMWASM_DIR} doesn't" >&2
  echo "       look BN254-patched. Run rebase-track-a.sh first." >&2
  exit 1
fi

if ! rustup toolchain list | grep -q "^${RUST_VERSION}-"; then
  echo "ERROR: Rust ${RUST_VERSION} not installed. Run rebase-track-a.sh first." >&2
  exit 1
fi

mkdir -p "${BUILD_DIR}"

echo "JunoClaw root:   ${JUNOCLAW_ROOT}"
echo "Build dir:       ${BUILD_DIR}"
echo "wasmvm tag:      ${WASMVM_TAG}"
echo "Rust toolchain:  ${RUST_VERSION}"
echo "cosmwasm dir:    ${COSMWASM_DIR}"
echo "wasmvm dir:      ${WASMVM_DIR}"
echo ""

# ----- 1. Clone or update upstream wasmvm ---------------------------------

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

clone_or_update "https://github.com/CosmWasm/wasmvm" "${WASMVM_DIR}" || exit 2

checkout_clean() {
  local dir="$1"
  local tag="$2"
  ( cd "${dir}" && \
      git reset --hard >/dev/null && \
      git checkout "${tag}" 2>/dev/null ) || {
    echo "ERROR: failed to checkout ${tag} in ${dir}" >&2
    return 2
  }
}

checkout_clean "${WASMVM_DIR}" "${WASMVM_TAG}" || exit 2
echo "wasmvm checked out at ${WASMVM_TAG}"

# Note: we don't `git clean -fdx` because libwasmvm/target may hold incremental
# build state we want to keep across runs for fast iteration. The Cargo.toml is
# reset by `git reset --hard` above, so the [patch] block insertion below
# starts from a clean baseline every run.

# ----- 2. Inject the [patch] redirect into libwasmvm/Cargo.toml -----------

CARGO_TOML="${WASMVM_DIR}/libwasmvm/Cargo.toml"

if grep -q '\[patch\."https://github.com/CosmWasm/cosmwasm.git"\]' "${CARGO_TOML}"; then
  echo "[patch] block already present in libwasmvm/Cargo.toml; skipping insert."
else
  echo "Appending [patch] block to libwasmvm/Cargo.toml..."
  cat >> "${CARGO_TOML}" <<EOF

# ── BN254 precompile track (junoclaw Phase 0.3) ──────────────────────────────
# Redirect the cosmwasm v2.2.2 git deps above to our patched local copy at
# ${COSMWASM_DIR}/. That tree holds cosmwasm v2.2.2 plus the eight Track-A
# BN254 patches at junoclaw/wasmvm-fork/patches/v2.2.2/. Cargo resolves
# [patch] before resolving the original git dependency, so the build pulls
# in the patched packages/std and packages/vm crates instead of fetching
# upstream.
[patch."https://github.com/CosmWasm/cosmwasm.git"]
cosmwasm-std = { path = "${COSMWASM_DIR}/packages/std" }
cosmwasm-vm  = { path = "${COSMWASM_DIR}/packages/vm" }
EOF
fi

# Validate the manifest parses before we kick off a long build.
( cd "${WASMVM_DIR}/libwasmvm" && \
    cargo "+${RUST_VERSION}" metadata --format-version 1 --no-deps > /dev/null ) || {
  echo "ERROR: libwasmvm/Cargo.toml does not parse after [patch] injection" >&2
  exit 3
}
echo "libwasmvm/Cargo.toml parses cleanly."

# ----- 3. Build libwasmvm.so ----------------------------------------------

echo ""
echo "Building libwasmvm with cargo +${RUST_VERSION} build --release..."
echo "(first build is ~5-10 min cold, ~1-2 min incremental)"

if ! ( cd "${WASMVM_DIR}/libwasmvm" && \
       cargo "+${RUST_VERSION}" build --release ); then
  echo "ERROR: libwasmvm build failed" >&2
  exit 4
fi

LIB_OUTPUT="${WASMVM_DIR}/libwasmvm/target/release/libwasmvm.so"
if [[ ! -f "${LIB_OUTPUT}" ]]; then
  echo "ERROR: build completed but ${LIB_OUTPUT} is missing" >&2
  exit 4
fi

# ----- 4. Verify Track-A symbols are linked in ----------------------------

echo ""
echo "Verifying BN254 symbols in ${LIB_OUTPUT}..."

required_symbols=(
  "cosmwasm_vm::imports::do_bn254_add"
  "cosmwasm_vm::imports::do_bn254_scalar_mul"
  "cosmwasm_vm::imports::do_bn254_pairing_equality"
  "cosmwasm_crypto_bn254::bn254::bn254_add"
  "cosmwasm_crypto_bn254::bn254::bn254_scalar_mul"
  "cosmwasm_crypto_bn254::bn254::bn254_pairing_equality"
)

# nm output for an 8.6 MB cdylib runs into millions of bytes once demangled.
# Capturing into a bash variable is fragile (some shells truncate, and the
# process-substitution echoes are slow), so we dump to a temp file.
SYMBOL_DUMP_FILE="$(mktemp /tmp/libwasmvm-symbols.XXXXXX.txt)"
trap 'rm -f "${SYMBOL_DUMP_FILE}"' EXIT
nm --demangle=rust "${LIB_OUTPUT}" > "${SYMBOL_DUMP_FILE}" 2>/dev/null || {
  echo "ERROR: nm could not read symbols from ${LIB_OUTPUT}" >&2
  exit 5
}

MISSING=0
for sym in "${required_symbols[@]}"; do
  if grep -q -F -- "${sym}" "${SYMBOL_DUMP_FILE}"; then
    echo "  ok  ${sym}"
  else
    echo "  MISSING  ${sym}" >&2
    MISSING=1
  fi
done

if [[ ${MISSING} -eq 1 ]]; then
  echo "ERROR: one or more required Track-A symbols are missing from libwasmvm.so" >&2
  exit 5
fi

# ----- 5. Stage the .so for downstream Go-side linking --------------------

INTERNAL_API="${WASMVM_DIR}/internal/api"
if [[ -d "${INTERNAL_API}" ]]; then
  cp "${LIB_OUTPUT}" "${INTERNAL_API}/libwasmvm.x86_64.so"
  echo ""
  echo "Staged: ${INTERNAL_API}/libwasmvm.x86_64.so"
fi

# ----- 6. Done -------------------------------------------------------------

echo ""
echo "==============================="
echo "TRACK A LIBWASMVM BUILD COMPLETE"
echo "==============================="
echo "Output:           ${LIB_OUTPUT}"
echo "Size:             $(ls -l "${LIB_OUTPUT}" | awk '{print $5}') bytes"
echo "Linked patched:   ${COSMWASM_DIR}/packages/{std,vm}"
echo "Toolchain:        Rust ${RUST_VERSION}"
echo "wasmvm tag:       ${WASMVM_TAG}"
echo ""
echo "All six BN254 entry-point symbols verified in the .so."
echo ""
echo "Next steps (Phase 0.3):"
echo "  * Build junod against this libwasmvm.so (or inject into a Docker image)."
echo "  * Deploy the precompile-variant zk-verifier on devnet."
echo "  * Benchmark gas for ECADD / ECMUL / ECPAIRING flows."
