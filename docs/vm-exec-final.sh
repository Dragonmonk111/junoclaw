#!/bin/bash
set -e

# Create properly formatted trigger input for our component
cat > /tmp/trigger-input.json << 'JSON'
{"outcome_verify":{"market_id":4,"question":"Should JunoClaw deploy TEE attestation?","resolution_criteria":"Hardware SGX enclave signs the hash"}}
JSON

echo "Input: $(cat /tmp/trigger-input.json)"
echo ""

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
  --log-level info \
  --json true \
  -o /tmp/wavs-output.json 2>&1

echo ""
echo "=== WAVS Output ==="
cat /tmp/wavs-output.json 2>/dev/null || echo "No output file"

echo ""
echo "=== SGX Hardware ==="
ls -la /dev/sgx_enclave /dev/sgx_provision 2>/dev/null
