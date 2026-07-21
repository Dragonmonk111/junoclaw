#!/bin/bash
set -euo pipefail

WORK_DIR="/tmp/juno-v30"
TAG="v30.0.0"
COMMIT="c0b3a8d258d52d16e5bc39a75168a99aab9d098e"

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

export PATH="/usr/local/go/bin:$PATH"
export GOTOOLCHAIN=go1.25.2

echo "Building Juno v30 from commit $ACTUAL_COMMIT"
GOTOOLCHAIN=go1.25.2 LEDGER_ENABLED=false make build

./bin/junod version --long
sha256sum ./bin/junod
echo "Binary: $WORK_DIR/bin/junod"
