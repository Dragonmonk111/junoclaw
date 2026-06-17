#!/usr/bin/env bash
set -euo pipefail

for i in 0 1 2; do
  out=$(curl -s localhost:26657/status 2>/dev/null | grep -oE 'latest_block_height[^,]+' || true)
  echo "t$((${i}*8)): ${out:-UNREACH}"
  if [ "$i" -lt 2 ]; then sleep 8; fi
done

echo "container: $(docker exec junoclaw-bn254-devnet date -u +'%Y-%m-%dT%H:%M:%S' 2>&1 | head -1)"
