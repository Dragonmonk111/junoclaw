#!/bin/bash
set -e

cd /mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet

# Create container and extract binaries
id=$(docker create junoclaw/junod-bn254:devnet 2>&1 | head -1)
docker cp "${id}:/usr/local/bin/junod" /tmp/junod 2>&1
docker cp "${id}:/usr/lib/libwasmvm.x86_64.so" /tmp/libwasmvm.x86_64.so 2>&1 || \
  docker cp "${id}:/usr/local/lib/libwasmvm.x86_64.so" /tmp/libwasmvm.x86_64.so 2>&1 || \
  docker cp "${id}:/lib/libwasmvm.x86_64.so" /tmp/libwasmvm.x86_64.so 2>&1
docker rm "$id" 2>&1

# Make executable
chmod +x /tmp/junod

# Test with library path
LD_LIBRARY_PATH=/tmp /tmp/junod version 2>&1 | head -1
