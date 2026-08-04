#!/bin/bash
# Live GPU inventory across Akash providers via console API (no mTLS needed).
python3 - <<'EOF'
import json, urllib.request
from collections import Counter

url = "https://console-api.akash.network/v1/providers"
req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/126 Safari/537.36", "Accept": "application/json"})
d = json.load(urllib.request.urlopen(req, timeout=30))
print(f"total providers in console API: {len(d)}")

model_totals = Counter()
flagship = []
for p in d:
    if not p.get("isOnline"):
        continue
    gpus = p.get("gpuModels") or []
    if not gpus:
        continue
    st = (p.get("stats") or {}).get("gpu") or {}
    avail, total = st.get("available", 0), st.get("total", 0)
    models = sorted({(g.get("model") or "?").lower() for g in gpus})
    for m in models:
        model_totals[m] += 1
    if any(k in ",".join(models) for k in ("h100", "h200", "b200")):
        flagship.append({
            "owner": p["owner"], "host": p.get("hostUri", "?"),
            "models": models, "ram": [g.get("ram") for g in gpus],
            "avail": avail, "total": total,
            "audited": p.get("isAudited", False),
            "up7d": round(p.get("uptime7d") or 0, 2),
        })

print("\n=== GPU model presence across ONLINE providers (provider count) ===")
for m, n in model_totals.most_common(30):
    print(f"  {m}: {n}")

print("\n=== H100/H200/B200 providers (online) — gpu avail/total ===")
if not flagship:
    print("  (none online)")
for f in sorted(flagship, key=lambda x: -x["avail"]):
    print(f"  models={f['models']} ram={f['ram']}")
    print(f"      gpu free={f['avail']}/{f['total']}  audited={f['audited']}  uptime7d={f['up7d']}")
    print(f"      owner={f['owner']}")
    print(f"      host={f['host']}")
EOF
