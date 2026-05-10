#!/usr/bin/env bash
# regen-v227.sh
#
# Regenerates the patch series for cosmwasm v2.2.7 by:
#   1. Resetting the upstream clone to v2.2.7
#   2. Applying the 8 patches from v2.2.2/ that already apply cleanly
#   3. Manually adding the two dependency lines (cosmwasm-crypto-bn254) that
#      drifted because v2.2.7 moved cosmwasm-crypto to workspace inheritance
#   4. Capturing `git diff` for those two files as the regenerated patches
#   5. Writing the new series into wasmvm-fork/patches/v2.2.7/
#
# Idempotent: re-running overwrites the v2.2.7/ directory.
#
# Usage: bash wasmvm-fork/patches/regen-v227.sh

set -euo pipefail

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

JUNOCLAW_ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
COSMWASM_DIR="${BUILD_DIR}/cosmwasm-bn254"
SRC_PATCH_DIR="${JUNOCLAW_ROOT}/wasmvm-fork/patches/v2.2.2"
DST_PATCH_DIR="${JUNOCLAW_ROOT}/wasmvm-fork/patches/v2.2.7"

if [[ ! -d "${COSMWASM_DIR}/.git" ]]; then
  echo "ERROR: ${COSMWASM_DIR} not a git checkout. Run check-baseline.sh first." >&2
  exit 1
fi

echo "=== regen-v227 ==="
echo "  source patches : ${SRC_PATCH_DIR}"
echo "  dest patches   : ${DST_PATCH_DIR}"
echo "  upstream clone : ${COSMWASM_DIR}"
echo ""

# 1. Reset clone to v2.2.7
( cd "${COSMWASM_DIR}" && git reset --hard v2.2.7 >/dev/null && git clean -fdx >/dev/null )
echo "[1/5] cosmwasm reset to v2.2.7 ($(cd "${COSMWASM_DIR}" && git rev-parse --short HEAD))"

# 2. Apply the 8 already-clean patches
CLEAN_PATCHES=(00 01 02 03 05 06 07 09)
for prefix in "${CLEAN_PATCHES[@]}"; do
  patch_file=$(ls "${SRC_PATCH_DIR}"/${prefix}-*.patch)
  ( cd "${COSMWASM_DIR}" && git apply "${patch_file}" )
  echo "[2/5]   applied $(basename "${patch_file}")"
done

# 3. Manually edit the two Cargo.toml files. v2.2.7 uses workspace inheritance;
#    we only add the bn254 line right after the existing cosmwasm-crypto line.

STD_CARGO="${COSMWASM_DIR}/packages/std/Cargo.toml"
VM_CARGO="${COSMWASM_DIR}/packages/vm/Cargo.toml"

# 3a. packages/std/Cargo.toml: add cosmwasm_2_3 feature + cosmwasm-crypto-bn254 dep
#     The feature line goes after cosmwasm_2_2.
#     The dep line goes after cosmwasm-crypto.
python3 - "${STD_CARGO}" <<'PY'
import sys, re, pathlib
p = pathlib.Path(sys.argv[1])
s = p.read_text()
# add cosmwasm_2_3 feature right after cosmwasm_2_2 = ["cosmwasm_2_1"]
s = re.sub(
    r'(cosmwasm_2_2 = \["cosmwasm_2_1"\]\n)',
    r'\1cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254"]\n',
    s, count=1)
# add bn254 dep right after cosmwasm-crypto = { workspace = true }
s = re.sub(
    r'(cosmwasm-crypto = \{ workspace = true \}\n)',
    r'\1cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254", optional = true }\n',
    s, count=1)
p.write_text(s)
PY
echo "[3/5] edited packages/std/Cargo.toml"

# 3b. packages/vm/Cargo.toml: add cosmwasm-crypto-bn254 dep after cosmwasm-crypto
python3 - "${VM_CARGO}" <<'PY'
import sys, re, pathlib
p = pathlib.Path(sys.argv[1])
s = p.read_text()
# add bn254 dep right after cosmwasm-crypto = { workspace = true }
s = re.sub(
    r'(cosmwasm-crypto = \{ workspace = true \}\n)',
    r'\1cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254" }\n',
    s, count=1)
p.write_text(s)
PY
echo "[3/5] edited packages/vm/Cargo.toml"

# 4. Capture diffs as new patches
mkdir -p "${DST_PATCH_DIR}"

# Copy the 8 unchanged patches
for prefix in "${CLEAN_PATCHES[@]}"; do
  src_file=$(ls "${SRC_PATCH_DIR}"/${prefix}-*.patch)
  cp "${src_file}" "${DST_PATCH_DIR}/$(basename "${src_file}")"
done
echo "[4/5] copied 8 unchanged patches into ${DST_PATCH_DIR}"

# Capture the regenerated 04 and 08
( cd "${COSMWASM_DIR}" && git diff -- packages/std/Cargo.toml ) > "${DST_PATCH_DIR}/04-cosmwasm-std.Cargo.toml.patch"
( cd "${COSMWASM_DIR}" && git diff -- packages/vm/Cargo.toml ) > "${DST_PATCH_DIR}/08-cosmwasm-vm.Cargo.toml.patch"
echo "[4/5] captured regenerated 04-cosmwasm-std.Cargo.toml.patch"
echo "[4/5] captured regenerated 08-cosmwasm-vm.Cargo.toml.patch"

# 5. Verify the new series applies cleanly to a fresh v2.2.7 checkout
( cd "${COSMWASM_DIR}" && git reset --hard v2.2.7 >/dev/null && git clean -fdx >/dev/null )
FAIL=0
for p in "${DST_PATCH_DIR}"/*.patch; do
  if ( cd "${COSMWASM_DIR}" && git apply --check "${p}" 2>/dev/null ); then
    echo "[5/5]   CLEAN $(basename "${p}")"
    ( cd "${COSMWASM_DIR}" && git apply "${p}" )
  else
    echo "[5/5]   CONFLICT $(basename "${p}")"
    FAIL=1
  fi
done

if [[ ${FAIL} -eq 1 ]]; then
  echo ""
  echo "REGENERATION VERIFICATION FAILED — see conflicts above" >&2
  exit 3
fi

PATCH_COUNT=$(ls "${DST_PATCH_DIR}"/*.patch | wc -l)
echo ""
echo "=== regen complete ==="
echo "  ${PATCH_COUNT} patches in ${DST_PATCH_DIR}"
echo "  all apply cleanly to cosmwasm v2.2.7"
