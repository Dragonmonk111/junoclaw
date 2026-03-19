#!/bin/bash
source ~/.cargo/env

# Kill any existing warg-server
pkill -f warg-server 2>/dev/null
sleep 1

# Read the signing key from keyring
OPKEY=$(cat ~/.config/warg/keyring/'service=warg-signing-key&user=default')
echo "Starting warg-server with operator key: $OPKEY"

# Start the registry
mkdir -p ~/warg-content
nohup warg-server --listen 0.0.0.0:8090 --content-dir ~/warg-content --operator-key "$OPKEY" > ~/warg-server.log 2>&1 &
sleep 3

echo "=== Server log ==="
tail -10 ~/warg-server.log

echo "=== Health check ==="
curl -s http://localhost:8090 2>&1 | head -5 || echo "Server not responding"

echo "=== Reconfigure warg client ==="
warg config --registry http://localhost:8090 --keyring-backend flat-file --overwrite

echo "=== Done ==="
