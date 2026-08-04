import json, sys
data = json.load(sys.stdin)
providers = data if isinstance(data, list) else data.get("providers", [])
print(f"Total providers: {len(providers)}")
print()
# Find H200 and H100 providers
h200 = []
h100 = []
for p in providers:
    models = p.get("gpus") or p.get("gpuModels") or []
    for g in models:
        model = g.get("model", "").lower() if isinstance(g, dict) else str(g).lower()
        if "h200" in model:
            h200.append(p)
            break
        if "h100" in model:
            h100.append(p)
            break
print(f"H200 providers: {len(h200)}")
for p in h200:
    addr = p.get("owner", p.get("address", "?"))
    models = p.get("gpus", [])
    print(f"  {addr} gpus={models}")
print()
print(f"H100 providers: {len(h100)}")
for p in h100:
    addr = p.get("owner", p.get("address", "?"))
    models = p.get("gpus", [])
    print(f"  {addr} gpus={models}")
