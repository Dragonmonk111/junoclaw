#!/usr/bin/env bash
for i in $(seq 1 12); do
  sleep 5
  docker ps --filter 'name=junoclaw' --format '{{.Names}} {{.Status}}'
  docker exec junoclaw-bn254-devnet junod status 2>/dev/null | grep latest_block_height || echo 'RPC_FAIL'
done
