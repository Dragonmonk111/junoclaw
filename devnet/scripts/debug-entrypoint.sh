#!/usr/bin/env bash
set -euo pipefail
HOME_DIR=${HOME_DIR:-/root/.juno}
if [ ! -f "${HOME_DIR}/config/genesis.json" ]; then
  /opt/juno/init-genesis.sh start
fi
/opt/juno/init-genesis.sh start &
PID=$!
wait $PID
EXIT_CODE=$?
echo "[debug-entrypoint] junod exited with code: $EXIT_CODE"
sleep 30
