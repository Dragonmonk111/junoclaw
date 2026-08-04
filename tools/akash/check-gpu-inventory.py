#!/usr/bin/env python3
"""Analyze Akash provider GPU inventory from provider list JSON."""
import json, sys

path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/providers.json"
d = json.load(open(path))
ps = d.get("providers", [])
print(f"Total providers: {len(ps)}\n")

for p in ps:
    owner = p.get("owner", "?")
    host = p.get("host_uri", "?")
    attrs = {a["key"]: a["value"] for a in p.get("attributes", [])}
    org = attrs.get("organization", "")
    gpus = [
        k for k in attrs
        if "gpu" in k.lower() and "model" in k.lower()
        and "/ram/" not in k and "/interface/" not in k
    ]
    tee = [f"{k}={v}" for k, v in attrs.items() if "tee" in k.lower()]
    if gpus or tee:
        print(owner)
        print(f"  org: {org}")
        print(f"  host: {host}")
        for g in sorted(gpus):
            print(f"  GPU: {g} = {attrs[g]}")
        for t in tee:
            print(f"  TEE: {t}")
        print()
