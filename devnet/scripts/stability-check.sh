#!/usr/bin/env bash
set -euo pipefail

h1=$(curl -s localhost:26657/status 2>/dev/null | grep -oE 'latest_block_height[^,]+' || true)
echo "start=${h1:-UNREACH}"

sleep 30

h2=$(curl -s localhost:26657/status 2>/dev/null | grep -oE 'latest_block_height[^,]+' || true)
echo "end=${h2:-UNREACH}"

clk=$(docker exec junoclaw-bn254-devnet date -u +'%Y-%m-%dT%H:%M:%S' 2>&1 | head -1)
echo "clock=${clk}"
