import subprocess, json

result = subprocess.run(
    ["wsl.exe", "-d", "Ubuntu-24.04", "--", "akash", "query", "provider", "list",
     "--node", "https://akash-rpc.polkachu.com:443", "--output", "json"],
    capture_output=True, text=True, timeout=30
)
data = json.loads(result.stdout)
providers = data.get("providers", [])

gpu_providers = []
for p in providers:
    attrs = p.get("attributes", [])
    has_gpu = False
    gpu_models = []
    for a in attrs:
        key = a.get("key", "")
        val = a.get("value", "")
        if "gpu" in key.lower() or "nvidia" in key.lower() or "vendor/nvidia" in key.lower():
            has_gpu = True
        if "model" in key.lower() and "nvidia" in key.lower():
            gpu_models.append(f"{key}={val}")
    if has_gpu or gpu_models:
        gpu_providers.append({
            "owner": p.get("owner", "?"),
            "host_uri": p.get("host_uri", "?"),
            "gpu_models": gpu_models,
        })

print(f"GPU providers: {len(gpu_providers)}")
for gp in gpu_providers[:30]:
    print(f"  {gp['owner']}")
    for m in gp['gpu_models']:
        print(f"    {m}")
