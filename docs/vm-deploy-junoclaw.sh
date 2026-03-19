#!/bin/bash
set -e

export PATH="/home/azureuser/.foundry/bin:/home/azureuser/.cargo/bin:$PATH"
export NVM_DIR="/home/azureuser/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

WAVS_ENDPOINT="http://127.0.0.1:8041"
AGGREGATOR_URL="http://127.0.0.1:8040"
IPFS_API="http://127.0.0.1:5001"
IPFS_GATEWAY="http://127.0.0.1:8080/ipfs/"
WASM_FILE="/home/azureuser/wavs-foundry-template/compiled/junoclaw_wavs_component.wasm"

cd /home/azureuser/wavs-foundry-template

echo "=== Step 1: Upload JunoClaw component to WAVS ==="
DIGEST=$(docker run --rm --network host \
  -v /home/azureuser/wavs-foundry-template:/data \
  ghcr.io/lay3rlabs/wavs:1.5.1 wavs-cli upload-component \
  /data/compiled/junoclaw_wavs_component.wasm \
  --wavs-endpoint $WAVS_ENDPOINT \
  --home /data --data /data/.docker 2>&1)
echo "Upload result: $DIGEST"

# Extract just the digest hash
COMPONENT_DIGEST=$(echo "$DIGEST" | grep -oP '[a-f0-9]{64}' | head -1)
if [ -z "$COMPONENT_DIGEST" ]; then
  COMPONENT_DIGEST=$(sha256sum $WASM_FILE | cut -d' ' -f1)
fi
echo "Component digest: $COMPONENT_DIGEST"

echo ""
echo "=== Step 2: Build JunoClaw service manifest ==="
# Create a service manifest that uses Cosmos triggers for Juno
# Using Digest source (references component by hash, no registry needed)
SERVICE_MANAGER=$(cat .docker/deployment_summary.json | python3 -c "import sys,json; print(json.load(sys.stdin)['wavs_service_manager'])")
echo "Service Manager: $SERVICE_MANAGER"

cat > /tmp/junoclaw-service.json << MANIFEST
{
  "name": "junoclaw-verifier",
  "workflows": {
    "outcome-verify": {
      "trigger": {
        "cosmos_contract_event": {
          "contract_address": "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6",
          "chain": "cosmos:uni-7",
          "event_type": "wasm-outcome_create"
        }
      },
      "component": {
        "source": {
          "Digest": "$COMPONENT_DIGEST"
        },
        "permissions": {
          "allowed_http_hosts": "all",
          "file_system": true
        },
        "fuel_limit": 1000000000000,
        "time_limit_seconds": 30,
        "config": {
          "chain_name": "cosmos:uni-7"
        },
        "env_keys": []
      },
      "submit": {
        "aggregator": {
          "url": "$AGGREGATOR_URL",
          "component": {
            "source": {
              "registry": {
                "digest": "64a8de853483f9010762c34ad8a9b4afaeef4b0622716dc4cd2ab55fb78491d9",
                "domain": "localhost:8090",
                "version": "0.1.0",
                "package": "example:aggregator"
              }
            },
            "permissions": {
              "allowed_http_hosts": "all",
              "file_system": true
            },
            "config": {},
            "env_keys": []
          },
          "signature_kind": {
            "algorithm": "secp256k1",
            "prefix": "eip191"
          }
        }
      }
    }
  },
  "status": "active",
  "manager": {
    "evm": {
      "chain": "evm:31337",
      "address": "$SERVICE_MANAGER"
    }
  }
}
MANIFEST

echo "Service manifest created"
cat /tmp/junoclaw-service.json | python3 -m json.tool 2>/dev/null || cat /tmp/junoclaw-service.json

echo ""
echo "=== Step 3: Upload service manifest to IPFS ==="
IPFS_RESULT=$(curl -s -X POST "$IPFS_API/api/v0/add?pin=true" \
  -H "Content-Type: multipart/form-data" \
  -F file=@/tmp/junoclaw-service.json 2>&1)
echo "IPFS result: $IPFS_RESULT"
SERVICE_CID=$(echo "$IPFS_RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['Hash'])" 2>/dev/null || echo "")
echo "Service CID: $SERVICE_CID"

if [ -z "$SERVICE_CID" ]; then
  echo "IPFS upload failed, trying direct file path..."
  SERVICE_URI="file:///tmp/junoclaw-service.json"
else
  SERVICE_URI="ipfs://$SERVICE_CID"
fi

echo ""
echo "=== Step 4: Deploy JunoClaw service to WAVS operator ==="
docker run --rm --network host \
  --env-file .env \
  -v /home/azureuser/wavs-foundry-template:/data \
  -v /tmp:/tmp \
  ghcr.io/lay3rlabs/wavs:1.5.1 wavs-cli deploy-service \
  --service-uri "$SERVICE_URI" \
  --wavs-endpoint $WAVS_ENDPOINT \
  --ipfs-gateway $IPFS_GATEWAY \
  --home /data --data /data/.docker \
  --log-level debug 2>&1

echo ""
echo "=== Step 5: Verify services ==="
curl -s $WAVS_ENDPOINT/services 2>&1 | python3 -m json.tool 2>/dev/null || curl -s $WAVS_ENDPOINT/services 2>&1

echo ""
echo "=== Done ==="
echo "SGX: $(ls /dev/sgx_enclave 2>/dev/null && echo ACTIVE || echo NOT FOUND)"
