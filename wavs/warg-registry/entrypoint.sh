#!/bin/bash
# JunoClaw Warg Registry — Akash Entrypoint
# Starts warg-server, generates operator key, publishes junoclaw:verifier component
set -e

echo "=== JunoClaw Warg Registry Starting ==="

# Configure warg client to point at local server
warg config --registry http://localhost:8090 --keyring-backend flat-file --overwrite

# Generate operator key (idempotent — skips if already exists)
warg key new 2>/dev/null || true

# Read operator key from keyring
OPKEY=$(cat ~/.config/warg/keyring/'service=warg-signing-key&user=default')
echo "Operator key: $OPKEY"

# Start warg-server in background
echo "=== Starting warg-server on 0.0.0.0:8090 ==="
warg-server \
  --listen 0.0.0.0:8090 \
  --content-dir /root/warg-content \
  --operator-key "$OPKEY" \
  --namespace junoclaw &

SERVER_PID=$!

# Wait for server to be ready
echo "=== Waiting for registry to be ready ==="
for i in $(seq 1 30); do
  if curl -s http://localhost:8090 > /dev/null 2>&1; then
    echo "Registry is ready (attempt $i)"
    break
  fi
  echo "Waiting... ($i/30)"
  sleep 2
done

# Abort any stale publish session
warg publish abort 2>/dev/null || true

# Publish the baked-in component
echo "=== Publishing junoclaw:verifier v0.1.0 ==="
warg publish start junoclaw:verifier
warg publish init junoclaw:verifier
warg publish release --name junoclaw:verifier --version 0.1.0 /opt/component.wasm
warg publish submit

echo "=== Verifying publication ==="
warg info junoclaw:verifier

echo "========================================="
echo " Registry LIVE — junoclaw:verifier v0.1.0"
echo " Listening on 0.0.0.0:8090"
echo " Akash-to-Akash — zero cloud dependency"
echo "========================================="

# Keep the server running
wait $SERVER_PID
