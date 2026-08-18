#!/usr/bin/env bash
# build-wasmvm-bn254-v3.sh
#
# Track B build harness for libwasmvm.so against
# `wasmvm` v3.0.4 wired to our patched `cosmwasm` v3.0.6.
#
# This is the P2 (no-fork) build script: it clones upstream cosmwasm and
# wasmvm at their tagged releases, applies the 10 BN254 patches from
# wasmvm-fork/patches/v3.0.x/, injects a [patch] block into libwasmvm's
# Cargo.toml, and builds. No public fork required.
#
# WHAT THIS SCRIPT DOES:
#
#   1. Clones (or updates) `CosmWasm/cosmwasm` at tag `v3.0.6`
#   2. Applies the 10 BN254 patches from `wasmvm-fork/patches/v3.0.x/`
#   3. Clones (or updates) `CosmWasm/wasmvm` at tag `v3.0.4`
#   4. Appends a `[patch."https://github.com/CosmWasm/cosmwasm.git"]` block
#      to `libwasmvm/Cargo.toml` redirecting cosmwasm-std and cosmwasm-vm
#      to the patched local copy
#   5. Runs `cargo +1.82 build --release` from libwasmvm/
#   6. Verifies BN254 symbols in the resulting `libwasmvm.so`
#   7. Copies the .so to a configurable output path and stages it for
#      downstream Go-side linking
#
# Prerequisites:
#   * bash, git, awk, nm, rustup, cargo
#   * Rust 1.82 toolchain installed (rustup toolchain install 1.82.0)
#   * Network access to github.com
#   * Run from the `junoclaw/` repo root
#
# Usage:
#   bash wasmvm-fork/patches/build-wasmvm-bn254-v3.sh
#
# Environment:
#   BUILD_DIR       where clones live (default: ${HOME}/junoclaw-build)
#   WASMVM_TAG      override the wasmvm tag (default: v3.0.4)
#   COSMWASM_TAG    override the cosmwasm tag (default: v3.0.6)
#   RUST_VERSION    override the pinned toolchain (default: 1.82.0)
#   OUTPUT_DIR      where to copy the finished .so (default: ${BUILD_DIR})
#
# Exit codes:
#   0  build succeeded and all expected symbols are linked in
#   1  precondition failed (missing tool, wrong cwd, no patches)
#   2  clone or checkout failed
#   3  patch application failed
#   4  Cargo.toml patch insertion failed
#   5  cargo build failed
#   6  symbol verification failed

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
WASMVM_TAG="${WASMVM_TAG:-v3.0.4}"
COSMWASM_TAG="${COSMWASM_TAG:-v3.0.6}"
RUST_VERSION="${RUST_VERSION:-1.82.0}"
OUTPUT_DIR="${OUTPUT_DIR:-${BUILD_DIR}}"

PATCH_DIR="${JUNOCLAW_ROOT}/wasmvm-fork/patches/v3.0.x"
COSMWASM_DIR="${BUILD_DIR}/cosmwasm-v3-bn254"
WASMVM_DIR="${BUILD_DIR}/wasmvm-v3-bn254"

# Sanity: patch directory must exist with patches
if [[ ! -d "${PATCH_DIR}" ]]; then
  echo "ERROR: patch directory ${PATCH_DIR} not found" >&2
  exit 1
fi

PATCH_COUNT=$(ls -1 "${PATCH_DIR}"/*.patch 2>/dev/null | wc -l)
if [[ "${PATCH_COUNT}" -lt 10 ]]; then
  echo "ERROR: expected >=10 patches in ${PATCH_DIR}, found ${PATCH_COUNT}" >&2
  exit 1
fi

if ! rustup toolchain list | grep -q "^${RUST_VERSION}-"; then
  echo "ERROR: Rust ${RUST_VERSION} not installed." >&2
  echo "       Run: rustup toolchain install ${RUST_VERSION}" >&2
  exit 1
fi

mkdir -p "${BUILD_DIR}" "${OUTPUT_DIR}"

echo "=== Track B BN254 Build (P2 — no fork) ==="
echo ""
echo "JunoClaw root:   ${JUNOCLAW_ROOT}"
echo "Build dir:       ${BUILD_DIR}"
echo "cosmwasm tag:    ${COSMWASM_TAG}"
echo "wasmvm tag:      ${WASMVM_TAG}"
echo "Rust toolchain:  ${RUST_VERSION}"
echo "Patch dir:       ${PATCH_DIR} (${PATCH_COUNT} patches)"
echo "cosmwasm dir:    ${COSMWASM_DIR}"
echo "wasmvm dir:      ${WASMVM_DIR}"
echo "Output dir:      ${OUTPUT_DIR}"
echo ""

# ----- 1. Clone and patch cosmwasm -----------------------------------------

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

echo "--- [1/6] Cloning cosmwasm at ${COSMWASM_TAG} ---"
clone_or_update "https://github.com/CosmWasm/cosmwasm" "${COSMWASM_DIR}" || exit 2
checkout_clean "${COSMWASM_DIR}" "${COSMWASM_TAG}" || exit 2
echo "cosmwasm checked out at ${COSMWASM_TAG}"

# ----- 2. Apply BN254 patches ----------------------------------------------

echo ""
echo "--- [2/6] Applying ${PATCH_COUNT} BN254 patches ---"

PATCHES=( $(ls -1 "${PATCH_DIR}"/*.patch | sort) )
FAILED_PATCHES=()

for patch in "${PATCHES[@]}"; do
  patch_name=$(basename "${patch}")
  if ( cd "${COSMWASM_DIR}" && git apply --check "${patch}" 2>/dev/null ); then
    ( cd "${COSMWASM_DIR}" && git apply "${patch}" )
    echo "  ok    ${patch_name}"
  else
    echo "  FAIL  ${patch_name}" >&2
    FAILED_PATCHES+=("${patch_name}")
  fi
done

if [[ ${#FAILED_PATCHES[@]} -gt 0 ]]; then
  echo "" >&2
  echo "ERROR: ${#FAILED_PATCHES[@]} patch(es) failed to apply:" >&2
  for fp in "${FAILED_PATCHES[@]}"; do
    echo "       ${fp}" >&2
  done
  exit 3
fi

# Verify the sentinel: crypto-bn254 crate should exist after patch 09
SENTINEL="${COSMWASM_DIR}/packages/crypto-bn254/Cargo.toml"
if [[ ! -f "${SENTINEL}" ]]; then
  echo "ERROR: ${SENTINEL} not found after patching." >&2
  echo "       Patch 09 (new crate) may not have applied correctly." >&2
  exit 3
fi
echo "All ${PATCH_COUNT} patches applied. Sentinel OK (crypto-bn254 crate present)."

# ----- 3. Clone wasmvm -----------------------------------------------------

echo ""
echo "--- [3/6] Cloning wasmvm at ${WASMVM_TAG} ---"
clone_or_update "https://github.com/CosmWasm/wasmvm" "${WASMVM_DIR}" || exit 2
checkout_clean "${WASMVM_DIR}" "${WASMVM_TAG}" || exit 2
echo "wasmvm checked out at ${WASMVM_TAG}"

# ----- 4. Inject [patch] block into libwasmvm/Cargo.toml -------------------

echo ""
echo "--- [4/6] Injecting [patch] block into libwasmvm/Cargo.toml ---"

CARGO_TOML="${WASMVM_DIR}/libwasmvm/Cargo.toml"

if grep -q '\[patch\.crates-io\]' "${CARGO_TOML}"; then
  echo "[patch] block already present; skipping insert."
else
  cat >> "${CARGO_TOML}" <<EOF

# ── BN254 precompile Track B (P2 no-fork) ────────────────────────────────────
# Redirect cosmwasm crates.io deps to our patched local copy at ${COSMWASM_DIR}/
# which holds cosmwasm ${COSMWASM_TAG} plus the 10 BN254 patches from
# junoclaw/wasmvm-fork/patches/v3.0.x/. Cargo resolves [patch] before
# resolving the original dependency, so the build pulls in the patched
# packages/std and packages/vm crates instead of fetching upstream.
[patch.crates-io]
cosmwasm-std = { path = "${COSMWASM_DIR}/packages/std" }
cosmwasm-vm  = { path = "${COSMWASM_DIR}/packages/vm" }
EOF
  echo "[patch] block appended."
fi

# Validate the manifest parses
( cd "${WASMVM_DIR}/libwasmvm" && \
    cargo "+${RUST_VERSION}" metadata --format-version 1 --no-deps > /dev/null ) || {
  echo "ERROR: libwasmvm/Cargo.toml does not parse after [patch] injection" >&2
  exit 4
}
echo "libwasmvm/Cargo.toml parses cleanly."

# Update Cargo.lock to pick up the patched local crates (v3.0.6 vs pinned v3.0.5)
echo "Updating Cargo.lock to resolve patched crates..."
( cd "${WASMVM_DIR}/libwasmvm" && \
    cargo "+${RUST_VERSION}" update -p cosmwasm-std -p cosmwasm-vm 2>&1 ) || {
  echo "WARNING: cargo update failed, trying full update..." >&2
  ( cd "${WASMVM_DIR}/libwasmvm" && cargo "+${RUST_VERSION}" update 2>&1 ) || true
}

# Verify the patch is now used
( cd "${WASMVM_DIR}/libwasmvm" && \
    cargo "+${RUST_VERSION}" tree -p cosmwasm-vm 2>&1 | head -3 ) | grep -q "not used in the crate graph" && {
  echo "ERROR: patch still not used after cargo update" >&2
  exit 4
}
echo "Patch verified: cosmwasm-vm resolved from local patched source."

# ----- 5. Build libwasmvm.so -----------------------------------------------

echo ""
echo "--- [5/6] Building libwasmvm with cargo +${RUST_VERSION} build --release ---"
echo "(first build is ~5-10 min cold, ~1-2 min incremental)"

if ! ( cd "${WASMVM_DIR}/libwasmvm" && \
       cargo "+${RUST_VERSION}" build --release ); then
  echo "ERROR: libwasmvm build failed" >&2
  exit 5
fi

LIB_OUTPUT="${WASMVM_DIR}/libwasmvm/target/release/libwasmvm.so"
if [[ ! -f "${LIB_OUTPUT}" ]]; then
  echo "ERROR: build completed but ${LIB_OUTPUT} is missing" >&2
  exit 5
fi

echo "Build complete: ${LIB_OUTPUT} ($(ls -l "${LIB_OUTPUT}" | awk '{print $5}') bytes)"

# ----- 6. Verify BN254 symbols ---------------------------------------------

echo ""
echo "--- [6/6] Verifying BN254 symbols ---"

required_symbols=(
  "cosmwasm_vm::imports::do_bn254_add"
  "cosmwasm_vm::imports::do_bn254_scalar_mul"
  "cosmwasm_vm::imports::do_bn254_pairing_equality"
  "cosmwasm_crypto_bn254::bn254::bn254_add"
  "cosmwasm_crypto_bn254::bn254::bn254_scalar_mul"
  "cosmwasm_crypto_bn254::bn254::bn254_pairing_equality"
)

SYMBOL_DUMP_FILE="$(mktemp /tmp/libwasmvm-v3-symbols.XXXXXX.txt)"
trap 'rm -f "${SYMBOL_DUMP_FILE}"' EXIT
nm --demangle=rust "${LIB_OUTPUT}" > "${SYMBOL_DUMP_FILE}" 2>/dev/null || {
  echo "ERROR: nm could not read symbols from ${LIB_OUTPUT}" >&2
  exit 6
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
  echo "ERROR: one or more required BN254 symbols are missing from libwasmvm.so" >&2
  exit 6
fi

# ----- Stage the .so for downstream linking --------------------------------

# Copy to wasmvm internal/api for Go-side linking
INTERNAL_API="${WASMVM_DIR}/internal/api"
if [[ -d "${INTERNAL_API}" ]]; then
  cp "${LIB_OUTPUT}" "${INTERNAL_API}/libwasmvm.x86_64.so"
  echo "Staged: ${INTERNAL_API}/libwasmvm.x86_64.so"
fi

# Copy to output dir
OUTPUT_SO="${OUTPUT_DIR}/libwasmvm.x86_64.so"
cp "${LIB_OUTPUT}" "${OUTPUT_SO}"
echo "Output: ${OUTPUT_SO}"

# ----- Done ----------------------------------------------------------------

echo ""
echo "==============================================="
echo "TRACK B LIBWASMVM BUILD COMPLETE (P2 no-fork)"
echo "==============================================="
echo "Output:           ${OUTPUT_SO}"
echo "Size:             $(ls -l "${OUTPUT_SO}" | awk '{print $5}') bytes"
echo "Patched cosmwasm: ${COSMWASM_DIR} (${COSMWASM_TAG} + 10 BN254 patches)"
echo "wasmvm tag:       ${WASMVM_TAG}"
echo "Toolchain:        Rust ${RUST_VERSION}"
echo ""
echo "All six BN254 entry-point symbols verified in the .so."
echo ""
echo "Next steps (Phase 3):"
echo "  * Build junod against this libwasmvm.so:"
echo "    cp ${OUTPUT_SO} /usr/local/lib/"
echo "    ldconfig"
echo "    # Then build junod from source — it links against the .so in /usr/local/lib/"
echo "  * Or use Go replace directive:"
echo "    replace github.com/CosmWasm/wasmvm/v3 => ${WASMVM_DIR}"
echo "  * Store precompile wasm on devnet to verify BN254 imports work"
echo "  * Benchmark VerifyProof gas (expect ~203k vs ~430k pure-Wasm mainnet)"
