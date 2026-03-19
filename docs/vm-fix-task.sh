#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

cd /home/azureuser/wavs-foundry-template

echo "=== Checking task version ==="
task --version 2>&1

echo "=== Downloading remote taskfiles locally ==="
mkdir -p .taskfiles

curl -sL https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/docker.yml -o .taskfiles/docker.yml
curl -sL https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/poa-operator.yml -o .taskfiles/poa-operator.yml
curl -sL https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/wasi.yml -o .taskfiles/wasi.yml
curl -sL https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/core.yml -o .taskfiles/core.yml

echo "Downloaded:"
ls -la .taskfiles/

echo "=== Patching Taskfile.yml to use local taskfiles ==="
sed -i 's|https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/docker.yml|.taskfiles/docker.yml|g' Taskfile.yml
sed -i 's|https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/poa-operator.yml|.taskfiles/poa-operator.yml|g' Taskfile.yml
sed -i 's|https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/wasi.yml|.taskfiles/wasi.yml|g' Taskfile.yml
sed -i 's|https://raw.githubusercontent.com/Lay3rLabs/wavs-taskfiles/main/base/core.yml|.taskfiles/core.yml|g' Taskfile.yml

echo "=== Verifying Taskfile.yml references ==="
grep -n "taskfile:" Taskfile.yml

echo "=== Testing task --list ==="
task --list 2>&1 | head -30

echo "=== Done ==="
