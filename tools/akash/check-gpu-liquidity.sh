#!/bin/bash
set -e
AKASH=${AKASH:-akash}
NODE=${AKASH_NODE:-tcp://akash-rpc.polkachu.com:26657}
CHAIN=${AKASH_CHAIN_ID:-akashnet-2}

echo "Querying providers on $CHAIN ($NODE)..."

$AKASH query provider list --chain-id "$CHAIN" --node "$NODE" --output json 2>/dev/null > /tmp/akash_providers.json

python3 << 'PYEOF'
import json

with open("/tmp/akash_providers.json") as f:
    data = json.load(f)

providers = data.get("providers", [])
print(f"Total providers: {len(providers)}")
print()

gpu_keys = ["h100", "h200", "a100", "gpu", "v100", "l40", "l40s"]
gpu_providers = []

for p in providers:
    owner = p.get("owner", "?")
    attrs = p.get("attributes", [])
    gpu_info = []
    for a in attrs:
        key = a.get("key", "")
        val = a.get("value", "")
        combined = (key + " " + str(val)).lower()
        if any(k in combined for k in ["gpu", "vendor", "model", "memory", "storage", "cpu", "tee"]):
            gpu_info.append(f"{key}={val}")
    if gpu_info:
        text = " ".join(str(i).lower() for i in gpu_info)
        if any(k in text for k in gpu_keys):
            gpu_providers.append((owner, gpu_info))

print(f"GPU providers found: {len(gpu_providers)}")
print()
for owner, info in sorted(gpu_providers):
    print(f"  {owner}")
    for i in info:
        print(f"    {i}")
    print()
PYEOF
