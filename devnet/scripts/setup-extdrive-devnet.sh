#!/bin/bash
# Setup devnet on external drive to avoid filling root partition
set -e

DEVNET_DIR="/mnt/extdrive/junoclaw-devnet"
JUNO_DATA="$DEVNET_DIR/juno-data"

echo "=== Setting up devnet on external drive ==="
echo "Location: $DEVNET_DIR"
echo "Available space: $(df -h /mnt/extdrive | tail -1 | awk '{print $4}')"

# Create directories
mkdir -p "$DEVNET_DIR"
mkdir -p "$JUNO_DATA"

# Check Docker is available
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker not installed on this VM"
    echo "Install: sudo apt update && sudo apt install docker.io docker-compose"
    exit 1
fi

echo ""
echo "=== Pulling junoclaw devnet image ==="
docker pull junoclaw/junod-bn254:devnet 2>&1 | tail -3

echo ""
echo "=== Starting devnet on external drive ==="

# Run with external drive volume
docker run -d \
    --name junoclaw-devnet-ext \
    -v "$JUNO_DATA:/root/.juno" \
    -p 127.0.0.1:36657:26657 \
    -p 127.0.0.1:36656:26656 \
    -p 127.0.0.1:11317:1317 \
    junoclaw/junod-bn254:devnet \
    2>&1

echo ""
echo "Waiting for chain to start..."
sleep 10

# Check status
if docker ps | grep -q junoclaw-devnet-ext; then
    echo "✓ Container running"
    if curl -s http://localhost:36657/status >/dev/null 2>&1; then
        echo "✓ RPC responding on port 36657"
        HEIGHT=$(curl -s http://localhost:36657/status | grep -o '"latest_block_height":"[0-9]*"' | head -1)
        echo "✓ Block height: $HEIGHT"
    else
        echo "⚠ RPC not responding yet (chain still initializing)"
    fi
else
    echo "✗ Container failed to start"
    docker logs junoclaw-devnet-ext 2>&1 | tail -20
    exit 1
fi

echo ""
echo "=== Devnet ready ==="
echo "RPC: http://localhost:36657"
echo "REST: http://localhost:11317"
echo "Data: $JUNO_DATA"
echo ""
echo "Next: Deploy contracts with:"
echo "  docker cp zk_verifier_pure.wasm junoclaw-devnet-ext:/tmp/"
echo "  docker exec junoclaw-devnet-ext junod tx wasm store /tmp/zk_verifier_pure.wasm ..."
