#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"
export TASK_X_REMOTE_TASKFILES=1

cd /home/azureuser/wavs-foundry-template

echo "=== Running full WAVS deployment ==="
pnpm run deploy:full 2>&1

echo ""
echo "=== Deployment Summary ==="
cat .docker/deployment_summary.json 2>/dev/null || echo "No summary file"
