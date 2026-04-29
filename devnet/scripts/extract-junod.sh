#!/bin/bash
set -e

cd /mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet

# Create container and extract junod
id=$(docker create junoclaw/junod-bn254:devnet 2>&1 | head -1)
docker cp "${id}:/usr/local/bin/junod" /tmp/junod 2>&1
docker rm "$id" 2>&1

# Make executable and test
chmod +x /tmp/junod
/tmp/junod version 2>&1 | head -1
