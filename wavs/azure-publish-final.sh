#!/bin/bash
source ~/.cargo/env

echo "=== Restarting warg-server with junoclaw namespace ==="
pkill -f warg-server 2>/dev/null
sleep 1

OPKEY=$(cat ~/.config/warg/keyring/'service=warg-signing-key&user=default')

nohup warg-server --listen 0.0.0.0:8090 \
  --content-dir ~/warg-content \
  --operator-key "$OPKEY" \
  --namespace junoclaw \
  > ~/warg-server.log 2>&1 &
sleep 3

echo "=== Server log ==="
tail -5 ~/warg-server.log

echo "=== Publishing component ==="
warg publish abort 2>/dev/null
warg publish start junoclaw:verifier
warg publish init junoclaw:verifier
warg publish release --name junoclaw:verifier --version 0.1.0 ~/junoclaw_wavs_component.wasm
warg publish submit

echo "=== Waiting for publish ==="
warg publish wait 2>/dev/null
sleep 2

echo "=== Verify ==="
warg info junoclaw:verifier

echo "=== DONE ==="
