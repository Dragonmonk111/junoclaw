#!/usr/bin/env bash
# build-junod-bn254.sh
#
# Builds a patched junod binary linked against our BN254 libwasmvm.so.
# Assumes:
#   - libwasmvm.x86_64.so already installed to /usr/local/lib/ (run build-wasmvm-bn254-v3.sh first)
#   - juno v30.0.0 cloned to /root/junoclaw-build/juno
set -euo pipefail

export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

JUNO_DIR="${JUNO_DIR:-/root/junoclaw-build/juno}"

echo "=== Building patched junod with BN254 libwasmvm ==="
echo "  juno dir: ${JUNO_DIR}"
echo "  libwasmvm: $(ls -la /usr/local/lib/libwasmvm* 2>/dev/null || echo 'NOT FOUND')"
echo ""

# Verify libwasmvm is findable
ldconfig
if ! ldconfig -p | grep -q libwasmvm; then
  echo "ERROR: libwasmvm not in ldconfig cache" >&2
  exit 1
fi
echo "[1/4] libwasmvm verified in ldconfig"

# Download deps
cd "${JUNO_DIR}"
echo "[2/4] Downloading Go dependencies..."
go mod download

# Build
echo "[3/4] Building junod..."
make build

# Verify
echo "[4/4] Verifying binary..."
BIN="${JUNO_DIR}/build/junod"
if [[ -f "${BIN}" ]]; then
  echo ""
  echo "=== BUILD COMPLETE ==="
  echo "  binary: ${BIN}"
  echo "  size: $(du -h "${BIN}" | cut -f1)"
  echo "  version: $(${BIN} version 2>&1 | head -3)"
  echo ""
  echo "  BN254 symbols in binary:"
  strings "${BIN}" | grep -i bn254 | head -10 || echo "  (checking via ldd...)"
  ldd "${BIN}" | grep wasmvm
  echo ""
  echo "Next steps:"
  echo "  ./build/junod query wasm libwasmvm-version"
  echo "  strings build/junod | grep bn254"
else
  echo "ERROR: junod binary not found at ${BIN}" >&2
  exit 2
fi
