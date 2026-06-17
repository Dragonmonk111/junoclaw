#!/usr/bin/env bash
# Non-destructive container-level clock fix experiment.
# Try to force the container clock forward, then check if CometBFT resumes.

set -euo pipefail

echo "=== WSL host clock (frozen) ==="
date -u +'%Y-%m-%dT%H:%M:%S'

echo ""
echo "=== container clock before ==="
docker exec junoclaw-bn254-devnet date -u +'%Y-%m-%dT%H:%M:%S'

echo ""
echo "=== try date -s inside container ==="
docker exec junoclaw-bn254-devnet sh -c 'date -u -s "2026-06-17T10:17:00" 2>&1 || echo SET_FAILED'

echo ""
echo "=== container clock after set ==="
docker exec junoclaw-bn254-devnet date -u +'%Y-%m-%dT%H:%M:%S'

echo ""
echo "=== poll RPC for 15s ==="
for i in $(seq 1 15); do
  out=$(curl -s localhost:26657/status 2>/dev/null | grep -oE 'latest_block_height.*[0-9]+' || true)
  echo "$i: ${out:-rpc_unreach}"
  sleep 1
done

echo ""
echo "=== container clock after 15s ==="
docker exec junoclaw-bn254-devnet date -u +'%Y-%m-%dT%H:%M:%S'

echo ""
echo "=== junod running? ==="
docker exec junoclaw-bn254-devnet ps aux | grep junod | grep -v grep | head -1 || echo 'NO_JUNOD_PROCESS'
