#!/usr/bin/env bash
# regen-mldsa.sh
#
# Generates the ML-DSA (FIPS 204) patch series (v2.2.2/20-28) the same way
# regen-mayo.sh works, one layer up: clone v2.2.2, apply the BN254 + MAYO
# series (00-19), vendor the cosmwasm-crypto-mldsa crate into the cosmwasm
# workspace, make the wiring edits programmatically, then capture `git diff`
# per file. Finally verifies the FULL 00-28 series re-applies cleanly to a
# pristine checkout.
#
# Unlike MAYO (which vendored junoclaw-mayo-verify as a sibling path dep),
# the ML-DSA verifier is the crates.io `fips204` crate, so there is no second
# vendored crate and no path-dep rewrite — cargo fetches `fips204` during the
# cosmwasm-vm build.
#
# Usage (from junoclaw repo root):  bash wasmvm-fork/patches/regen-mldsa.sh

set -euo pipefail

if [[ ! -f "Cargo.toml" ]] || [[ ! -d "wasmvm-fork" ]]; then
  echo "ERROR: run this script from the junoclaw repo root" >&2
  exit 1
fi

ROOT="$(pwd)"
BUILD_DIR="${BUILD_DIR:-${HOME}/junoclaw-build}"
CW="${BUILD_DIR}/cosmwasm-mldsa"
PATCH_DIR="${ROOT}/wasmvm-fork/patches/v2.2.2"
EDITS_PY="${ROOT}/wasmvm-fork/patches/regen-mldsa-edits.py"
MLDSA_CRYPTO_SRC="${ROOT}/wasmvm-fork/cosmwasm-crypto-mldsa"

echo "=== regen-mldsa ==="
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

# 2. Apply BN254 + MAYO series 00-19, then COMMIT so the ML-DSA diffs below are
#    captured relative to that baseline (not pristine v2.2.2).
for p in "${PATCH_DIR}"/0[0-9]-*.patch "${PATCH_DIR}"/1[0-9]-*.patch; do
  sed -i 's/\r$//' "$p"
  ( cd "${CW}" && git apply "$p" )
  echo "[2/6]   applied $(basename "$p")"
done
( cd "${CW}" && git add -A && \
  git -c user.email=regen@junoclaw -c user.name=regen \
      commit -q -m "bn254 + mayo baseline (00-19)" )
echo "[2/6] committed BN254+MAYO baseline as HEAD"

# 3. Vendor the cosmwasm-crypto-mldsa crate into the cosmwasm workspace.
#    No path-dep rewrite: it depends on crates.io `fips204`, not a sibling.
mkdir -p "${CW}/packages/crypto-mldsa"
cp "${MLDSA_CRYPTO_SRC}/Cargo.toml" "${CW}/packages/crypto-mldsa/Cargo.toml"
cp -r "${MLDSA_CRYPTO_SRC}/src" "${CW}/packages/crypto-mldsa/src"
echo "[3/6] vendored crypto-mldsa into packages/"

# 4. Make all wiring edits
python3 "${EDITS_PY}" "${CW}"
echo "[4/6] wiring edits applied"

# 5. Capture diffs as numbered patches
git -C "${CW}" add -N packages/crypto-mldsa >/dev/null

cap() {  # cap <number-name> <path> — diff vs the 00-19 baseline commit
  git -C "${CW}" diff HEAD -- "$2" > "${PATCH_DIR}/$1"
  echo "[5/6]   captured $1 ($(wc -l < "${PATCH_DIR}/$1" | tr -d ' ') lines)"
}
cap "20-cosmwasm-vm.Cargo.toml.patch"           packages/vm/Cargo.toml
cap "21-cosmwasm-vm.imports.rs.patch"           packages/vm/src/imports.rs
cap "22-cosmwasm-vm.compatibility.rs.patch"     packages/vm/src/compatibility.rs
cap "23-cosmwasm-vm.instance.rs.patch"          packages/vm/src/instance.rs
cap "24-cosmwasm-std.Cargo.toml.patch"          packages/std/Cargo.toml
cap "25-cosmwasm-std.imports.rs.patch"          packages/std/src/imports.rs
cap "26-cosmwasm-std.traits.rs.patch"           packages/std/src/traits.rs
cap "27-cosmwasm-std.testing.mock.rs.patch"     packages/std/src/testing/mock.rs
cap "28-cosmwasm-crypto-mldsa-new-crate.patch"  packages/crypto-mldsa

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
echo "=== regen complete: 00-28 all apply cleanly to cosmwasm v2.2.2 ==="
echo "Optional compile check (also fetches fips204 from crates.io):"
echo "  ( cd ${CW} && cargo build --release -p cosmwasm-vm )"
