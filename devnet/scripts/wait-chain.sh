#!/usr/bin/env bash
# Wait for the devnet chain to produce blocks, then deploy contracts.
set -euo pipefail

CONTAINER="junoclaw-bn254-devnet"
MAX_WAIT=60

echo "[wait] Waiting for chain to reach block height >= 2..."
for i in $(seq 1 $MAX_WAIT); do
  sleep 1
  STATUS=$(docker inspect -f '{{.State.Status}}' "$CONTAINER" 2>/dev/null || echo "gone")
  if [ "$STATUS" != "running" ]; then
    echo "[wait] FAIL: container status=$STATUS at second $i"
    docker logs --tail 30 "$CONTAINER" 2>&1 || true
    exit 1
  fi
  HEIGHT=$(docker exec "$CONTAINER" curl -sf http://localhost:26657/status 2>/dev/null \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['sync_info']['latest_block_height'])" 2>/dev/null || echo "0")
  if [ "$HEIGHT" -ge 2 ] 2>/dev/null; then
    echo "[wait] OK: chain at block $HEIGHT (after ${i}s)"
    exit 0
  fi
  if [ $((i % 5)) -eq 0 ]; then
    echo "[wait]   ...still waiting (${i}s, height=${HEIGHT})"
  fi
done
echo "[wait] TIMEOUT after ${MAX_WAIT}s"
exit 1
