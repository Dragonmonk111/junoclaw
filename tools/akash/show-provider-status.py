#!/usr/bin/env python3
"""Pretty-print Akash provider /status JSON, focused on GPU inventory."""
import json, sys

path = sys.argv[1]
d = json.load(open(path))

cluster = d.get("cluster", {})
inv = cluster.get("inventory", {})
nodes = inv.get("nodes", [])
res = inv.get("reservations", {})

print(f"Nodes: {len(nodes)}")
for n in nodes:
    name = n.get("name", "?")
    cpu = n.get("cpu", {})
    gpu = n.get("gpu", {})
    mem = n.get("memory", {})
    print(f"\nNode: {name}")
    avail_gpu = gpu.get("available", {})
    print(f"  GPU available: {json.dumps(avail_gpu)}")
    alloc = gpu.get("allocated", {})
    print(f"  GPU allocated: {json.dumps(alloc)}")
    print(f"  Memory available: {json.dumps(mem.get('available', {}))}")
    caps = n.get("capabilities", {})
    if caps:
        print(f"  Capabilities: {json.dumps(caps)[:500]}")

print(f"\nReservations active: {json.dumps(res)[:500]}")

# Also dump raw top-level keys for debugging
print(f"\nTop-level keys: {list(d.keys())}")
