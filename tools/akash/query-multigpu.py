import json, sys
data = json.load(sys.stdin)
providers = data if isinstance(data, list) else data.get("providers", [])
print(f"Total providers: {len(providers)}")

# Look for any provider whose attributes hint at multi-GPU capacity
# Akash provider list doesn't directly show "available count" per lease,
# it shows total inventory. We check total_gpu / capacity fields if present.
candidates = []
for p in providers:
    gpus = p.get("gpus", [])
    if not gpus:
        continue
    for g in gpus:
        model = (g.get("model") or "").lower()
        if "h100" in model or "h200" in model:
            candidates.append((p, g))

print(f"H100/H200 GPU entries: {len(candidates)}")
for p, g in candidates:
    owner = p.get("owner", "?")
    # look for capacity/total keys
    keys_of_interest = {k: v for k, v in p.items() if "capacity" in k.lower() or "total" in k.lower() or "available" in k.lower()}
    print(f"  owner={owner} gpu={g} extra={keys_of_interest}")
