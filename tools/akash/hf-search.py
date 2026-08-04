#!/usr/bin/env python3
"""Search HuggingFace models API, filter by keywords, print id + size."""
import json, sys, urllib.request

query = sys.argv[1]
keywords = [k.lower() for k in sys.argv[2:]] or ["awq", "gptq", "int4", "w4a16", "fp8"]

url = f"https://huggingface.co/api/models?search={urllib.request.quote(query)}&limit=100"
req = urllib.request.Request(url, headers={"Accept": "application/json"})
with urllib.request.urlopen(req, timeout=30) as resp:
    ms = json.loads(resp.read().decode())

for m in ms:
    mid = m.get("id", "")
    if any(k in mid.lower() for k in keywords):
        print(f"{mid}  (downloads={m.get('downloads',0)}, likes={m.get('likes',0)})")
