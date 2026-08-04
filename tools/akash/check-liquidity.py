#!/usr/bin/env python3
"""Quick Akash GPU liquidity check via REST API."""
import json, sys, urllib.request

endpoints = [
    "https://api.akashnet.net/akash/provider/v1beta3/providers",
    "https://rest.akashnet.net/akash/provider/v1beta3/providers",
    "https://api.akashnet.net/akash/provider/v1beta2/providers",
]

data = None
for url in endpoints:
    try:
        req = urllib.request.Request(url, headers={"Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=20) as resp:
            data = json.loads(resp.read().decode())
            print(f"Fetched from: {url}")
            break
    except Exception as e:
        print(f"  {url}: {e}")

if data is None:
    print("ERROR: Could not fetch providers from any endpoint")
    sys.exit(1)

providers = data.get("providers", [])
print(f"Total providers: {len(providers)}\n")

gpu_providers = []
for p in providers:
    owner = p.get("owner", "?")
    attrs = p.get("attributes", [])
    gpu_info = []
    for a in attrs:
        key = a.get("key", "")
        val = a.get("value", "")
        if any(k in key.lower() for k in ["gpu", "vendor", "model", "tee", "memory", "storage", "cpu"]):
            gpu_info.append(f"{key}={val}")
    if gpu_info:
        gpu_providers.append((owner, gpu_info))

print(f"Providers with hardware attributes: {len(gpu_providers)}\n")
for owner, info in sorted(gpu_providers):
    combined = " ".join(info).lower()
    has_gpu = any(k in combined for k in ["h100", "h200", "a100", "gpu"])
    marker = "***" if has_gpu else "   "
    print(f"{marker} {owner}")
    for i in info:
        print(f"      {i}")
    print()

print("---\nH100/H200/A100 specifically:")
for owner, info in sorted(gpu_providers):
    combined = " ".join(info).lower()
    if any(k in combined for k in ["h100", "h200", "a100"]):
        print(f"  {owner}: {info}")
