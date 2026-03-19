#!/bin/bash
source ~/.cargo/env

echo "=== Checking warg-server namespace options ==="
warg-server --help 2>&1 | grep -i -A2 namespace

echo ""
echo "=== Restarting server with auto namespace creation ==="
pkill -f warg-server 2>/dev/null
sleep 1

OPKEY=$(cat ~/.config/warg/keyring/'service=warg-signing-key&user=default')

# Try with --auto-create-namespaces flag (common in warg-server)
nohup warg-server --listen 0.0.0.0:8090 \
  --content-dir ~/warg-content \
  --operator-key "$OPKEY" \
  --auto-create-namespaces \
  > ~/warg-server.log 2>&1 &
sleep 3

echo "=== Server log ==="
tail -10 ~/warg-server.log

# If server started, abort previous publish and try again
if curl -s http://localhost:8090 > /dev/null 2>&1; then
  echo "=== Server running, publishing ==="
  warg publish abort 2>/dev/null
  warg publish start junoclaw:verifier
  warg publish init junoclaw:verifier
  warg publish release --name junoclaw:verifier --version 0.1.0 ~/junoclaw_wavs_component.wasm
  warg publish submit
  echo "=== Verify ==="
  warg info junoclaw:verifier
else
  echo "Server failed to start, checking log:"
  cat ~/warg-server.log
fi
