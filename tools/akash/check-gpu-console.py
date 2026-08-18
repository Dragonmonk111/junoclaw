import urllib.request
import json

url = "https://console-api.akash.network/v1/providers?status=active"
req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
with urllib.request.urlopen(req, timeout=30) as resp:
    data = json.loads(resp.read())

providers = data if isinstance(data, list) else data.get("providers", [])
print(f"Total providers fetched: {len(providers)}")
print()

TARGET_GPUS = ["h100", "h200", "a100", "l40s", "l40", "a6000", "rtx6000"]

gpu_providers = []
for p in providers:
    gpu_models = p.get("gpuModels", [])
    if not gpu_models:
        continue
    has_target = False
    models = []
    for g in gpu_models:
        model = g.get("model", "").lower()
        ram = g.get("ram", "")
        iface = g.get("interface", "")
        vendor = g.get("vendor", "")
        models.append(f"{vendor}/{model}/{ram}/{iface}")
        if any(t in model for t in TARGET_GPUS):
            has_target = True
    
    stats = p.get("stats", {}).get("gpu", {})
    is_online = p.get("isOnline", False)
    org = p.get("organization") or p.get("name") or p.get("owner", "?")[:20]
    
    if has_target or any("h100" in m.lower() or "h200" in m.lower() or "a100" in m.lower() for m in models):
        gpu_providers.append({
            "owner": p.get("owner", "?"),
            "org": org,
            "online": is_online,
            "models": models,
            "gpu_total": stats.get("total", 0),
            "gpu_active": stats.get("active", 0),
            "gpu_available": stats.get("available", 0),
            "uptime_1d": p.get("uptime1d", 0),
        })

print("=== H100/H200/A100/L40 GPU Providers ===")
print(f"Found: {len(gpu_providers)}")
print()
for p in sorted(gpu_providers, key=lambda x: (-x["gpu_available"], -x["gpu_total"], x["org"])):
    status = "ON" if p["online"] else "OFF"
    print(f"  [{status}] {p['org']}: avail={p['gpu_available']} total={p['gpu_total']} active={p['gpu_active']} | {', '.join(p['models'])}")

# Summary by GPU type
print()
print("=== SUMMARY: Online providers with available GPUs ===")
online_avail = [p for p in gpu_providers if p["online"] and p["gpu_available"] > 0]
if online_avail:
    for p in online_avail:
        print(f"  {p['org']}: {p['gpu_available']} available of {p['gpu_total']} | {', '.join(p['models'])}")
else:
    print("  NONE - no H100/H200/A100/L40 providers online with available GPUs")

print()
print("=== SUMMARY: Online providers (any GPU count, even 0 available) ===")
online_any = [p for p in gpu_providers if p["online"]]
for p in online_any:
    print(f"  {p['org']}: total={p['gpu_total']} avail={p['gpu_available']} | {', '.join(p['models'])}")

print()
print("=== SUMMARY: All H200 providers ===")
h200 = [p for p in gpu_providers if any("h200" in m.lower() for m in p["models"])]
for p in h200:
    print(f"  [{'ON' if p['online'] else 'OFF'}] {p['org']}: total={p['gpu_total']} avail={p['gpu_available']} | {', '.join(p['models'])}")

print()
print("=== SUMMARY: All H100 providers ===")
h100 = [p for p in gpu_providers if any("h100" in m.lower() and "h200" not in m.lower() for m in p["models"])]
for p in h100:
    print(f"  [{'ON' if p['online'] else 'OFF'}] {p['org']}: total={p['gpu_total']} avail={p['gpu_available']} | {', '.join(p['models'])}")
