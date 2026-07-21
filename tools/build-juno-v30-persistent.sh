#!/bin/bash
set -euo pipefail

WORK_DIR="/root/juno-v30-build"
TAG="v30.0.0"
COMMIT="c0b3a8d258d52d16e5bc39a75168a99aab9d098e"
OUT_BIN="/usr/local/bin/junod-v30.0.0"

export PATH="/usr/local/go/bin:$PATH"
export GOTOOLCHAIN=go1.25.2

rm -rf "$WORK_DIR"
git clone https://github.com/CosmosContracts/juno.git "$WORK_DIR"
cd "$WORK_DIR"

git fetch --tags
git checkout "$TAG" || git checkout "$COMMIT"

ACTUAL_COMMIT="$(git rev-list -n 1 HEAD)"
if [ "$ACTUAL_COMMIT" != "$COMMIT" ]; then
  echo "WARNING: HEAD ($ACTUAL_COMMIT) does not match expected commit ($COMMIT)"
  exit 1
fi

echo "Building Juno v30 from commit $ACTUAL_COMMIT"
LEDGER_ENABLED=false make build

install -m 0755 "$WORK_DIR/bin/junod" "$OUT_BIN"
"$OUT_BIN" version --long
sha256sum "$OUT_BIN"
echo "Binary installed at: $OUT_BIN"
