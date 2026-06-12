#!/usr/bin/env bash
set -euo pipefail
NODE="http://localhost:26657"
CONTAINER="junoclaw-bn254-devnet"

# Block height check
HEIGHT1=$(docker exec "$CONTAINER" junod status --node "$NODE" 2>/dev/null | jq -r '.SyncInfo.latest_block_height // empty')
echo "height1=$HEIGHT1"
sleep 3
HEIGHT2=$(docker exec "$CONTAINER" junod status --node "$NODE" 2>/dev/null | jq -r '.SyncInfo.latest_block_height // empty')
echo "height2=$HEIGHT2"

if [ -z "$HEIGHT1" ] || [ -z "$HEIGHT2" ]; then
  echo "status=STALLED_NO_HEIGHT"
  exit 1
fi

if [ "$HEIGHT2" -gt "$HEIGHT1" ]; then
  echo "status=ADVANCING delta=$((HEIGHT2 - HEIGHT1))"
else
  echo "status=STALLED delta=0"
fi
