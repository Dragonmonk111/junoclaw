#!/bin/bash
set -e

# Real proposal 4 data from Juno uni-7
cat > /tmp/trigger-input.json << 'JSON'
{"outcome_verify":{"market_id":4,"question":"Will JunoClaw WAVS attestation integration pass E2E test?","resolution_criteria":"Attestation successfully submitted and queried on uni-7"}}
JSON

echo "Input: $(cat /tmp/trigger-input.json)"
echo ""

echo "=== Executing with REAL proposal 4 data in SGX TEE ==="
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
echo "=== RESULT ==="
cat /tmp/wavs-output.json 2>/dev/null || echo "No output"
