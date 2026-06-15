#!/usr/bin/env bash
# regen-mayo.sh
#
# Generates the MAYO patch series (v2.2.2/10-19) the same way regen-v227.sh
# works: clone v2.2.2, apply the BN254 series (00-09), vendor the two MAYO
# crates into the cosmwasm workspace, make the wiring edits programmatically,
# then capture `git diff` per file. Finally verifies the FULL 00-19 series
# re-applies cleanly to a pristine checkout.
#
# Self-contained: both MAYO crates are vendored INTO the patch (like crypto-
# bn254 in patch 09), so the devnet Dockerfile needs no extra COPY step.
#
# Usage (from junoclaw repo root):  bash wasmvm-fork/patches/regen-mayo.sh

set -euo pipefail

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
CW="${BUILD_DIR}/cosmwasm-mayo"
PATCH_DIR="${ROOT}/wasmvm-fork/patches/v2.2.2"
EDITS_PY="${ROOT}/wasmvm-fork/patches/regen-mayo-edits.py"
MAYO_CRYPTO_SRC="${ROOT}/wasmvm-fork/cosmwasm-crypto-mayo"
MAYO_VERIFY_SRC="${ROOT}/crates/junoclaw-mayo-verify"

echo "=== regen-mayo ==="
echo "  clone     : ${CW}"
echo "  patches   : ${PATCH_DIR}"
echo ""

# 1. Clone or reset cosmwasm v2.2.2
if [[ -d "${CW}/.git" ]]; then
  ( cd "${CW}" && git reset --hard v2.2.2 >/dev/null && git clean -fdx >/dev/null )
  echo "[1/6] reset cosmwasm to v2.2.2"
else
  mkdir -p "${BUILD_DIR}"
  git clone --branch v2.2.2 --depth 1 \
    https://github.com/CosmWasm/cosmwasm "${CW}"
  echo "[1/6] cloned cosmwasm v2.2.2"
fi

# 2. Apply BN254 series 00-09, then COMMIT so the MAYO diffs below are
#    captured relative to the BN254 baseline (not pristine v2.2.2).
for p in "${PATCH_DIR}"/0[0-9]-*.patch; do
  sed -i 's/\r$//' "$p"
  ( cd "${CW}" && git apply "$p" )
  echo "[2/6]   applied $(basename "$p")"
done
( cd "${CW}" && git add -A && \
  git -c user.email=regen@junoclaw -c user.name=regen \
      commit -q -m "bn254 baseline (00-09)" )
echo "[2/6] committed BN254 baseline as HEAD"

# 3. Vendor the two MAYO crates into the cosmwasm workspace
mkdir -p "${CW}/packages/crypto-mayo" "${CW}/packages/mayo-verify"
cp "${MAYO_CRYPTO_SRC}/Cargo.toml" "${CW}/packages/crypto-mayo/Cargo.toml"
cp -r "${MAYO_CRYPTO_SRC}/src" "${CW}/packages/crypto-mayo/src"
cp "${MAYO_VERIFY_SRC}/Cargo.toml" "${CW}/packages/mayo-verify/Cargo.toml"
cp -r "${MAYO_VERIFY_SRC}/src" "${CW}/packages/mayo-verify/src"

# Rewrite crypto-mayo's path dep to point at the vendored sibling.
sed -i -E \
  's|junoclaw-mayo-verify = \{ path = "[^"]*" \}|junoclaw-mayo-verify = { path = "../mayo-verify" }|' \
  "${CW}/packages/crypto-mayo/Cargo.toml"
echo "[3/6] vendored crypto-mayo + mayo-verify into packages/"

# 4. Make all wiring edits
python3 "${EDITS_PY}" "${CW}"
echo "[4/6] wiring edits applied"

# 5. Capture diffs as numbered patches
git -C "${CW}" add -N packages/crypto-mayo packages/mayo-verify >/dev/null

cap() {  # cap <number-name> <path> — diff vs the BN254 baseline commit
  git -C "${CW}" diff HEAD -- "$2" > "${PATCH_DIR}/$1"
  echo "[5/6]   captured $1 ($(wc -l < "${PATCH_DIR}/$1" | tr -d ' ') lines)"
}
cap "10-cosmwasm-vm.Cargo.toml.patch"          packages/vm/Cargo.toml
cap "11-cosmwasm-vm.imports.rs.patch"          packages/vm/src/imports.rs
cap "12-cosmwasm-vm.compatibility.rs.patch"    packages/vm/src/compatibility.rs
cap "13-cosmwasm-vm.instance.rs.patch"         packages/vm/src/instance.rs
cap "14-cosmwasm-std.Cargo.toml.patch"         packages/std/Cargo.toml
cap "15-cosmwasm-std.imports.rs.patch"         packages/std/src/imports.rs
cap "16-cosmwasm-std.traits.rs.patch"          packages/std/src/traits.rs
cap "17-cosmwasm-std.testing.mock.rs.patch"    packages/std/src/testing/mock.rs
cap "18-cosmwasm-crypto-mayo-new-crate.patch"  packages/crypto-mayo
cap "19-junoclaw-mayo-verify-new-crate.patch"  packages/mayo-verify

# 6. Verify the FULL series re-applies cleanly to a pristine checkout
( cd "${CW}" && git reset --hard v2.2.2 >/dev/null && git clean -fdx >/dev/null )
FAIL=0
for p in "${PATCH_DIR}"/*.patch; do
  if ( cd "${CW}" && git apply --check "$p" 2>/dev/null ); then
    ( cd "${CW}" && git apply "$p" )
    echo "[6/6]   CLEAN    $(basename "$p")"
  else
    echo "[6/6]   CONFLICT $(basename "$p")"
    FAIL=1
  fi
done

if [[ ${FAIL} -eq 1 ]]; then
  echo "" >&2
  echo "VERIFICATION FAILED — conflicts above" >&2
  exit 3
fi

echo ""
echo "=== regen complete: 00-19 all apply cleanly to cosmwasm v2.2.2 ==="
echo "Optional compile check:"
echo "  ( cd ${CW} && cargo build --release -p cosmwasm-vm )"
