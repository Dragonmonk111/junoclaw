#!/bin/bash
set -e

# Create proper JSON input file for our component
cat > /tmp/trigger-input.json << 'JSON'
{"proposal_id":4,"task_type":"outcome_verify","question":"Should JunoClaw deploy TEE attestation?","resolution_criteria":"Hardware SGX enclave signs the hash"}
JSON

echo "Input: $(cat /tmp/trigger-input.json)"
echo ""

# Run our component via wavs-cli exec with SGX devices
echo "=== Executing JunoClaw component in SGX TEE ==="
docker run --rm --network host \
  --device /dev/sgx_enclave:/dev/sgx_enclave \
  --device /dev/sgx_provision:/dev/sgx_provision \
  -v /home/azureuser/junoclaw/wavs:/data \
  -v /tmp:/tmp \
  ghcr.io/lay3rlabs/wavs:1.5.1 wavs-cli exec \
  --component /data/junoclaw_wavs_component.wasm \
  --input @/tmp/trigger-input.json \
  --home /data \
  --log-level debug \
  --json true \
  -o /tmp/wavs-output.json 2>&1

echo ""
echo "=== Output ==="
cat /tmp/wavs-output.json 2>/dev/null || echo "No output file"
echo ""
echo "SGX: $(ls /dev/sgx_enclave 2>/dev/null && echo ACTIVE)"
