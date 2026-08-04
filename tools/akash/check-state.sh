#!/bin/bash
# Check active deployments/leases for the akash-jlens wallet + current chain height.
OWNER="akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt"
NODE="https://rpc.akashnet.net:443"

echo "=== Active deployments for $OWNER ==="
TMPJSON=$(mktemp)
akash query deployment list --owner "$OWNER" --node "$NODE" --output json 2>/dev/null > "$TMPJSON"
python3 - "$TMPJSON" <<'EOF'
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
active = [x for x in d.get("deployments", []) if x["deployment"]["state"] != "closed"]
if not active:
    print("  (none — all closed)")
for x in active:
    dep = x["deployment"]
    print(f"  dseq={dep['id']['dseq']} state={dep['state']}")
    for g in x.get("groups", []):
        gs = g["group_spec"]
        for r in gs.get("resources", []):
            res = r["resource"]
            gpu = res.get("gpu", {})
            attrs = ",".join(a["key"] for a in gpu.get("attributes", []))
            print(f"    gseq={g['id']['gseq']} gpu_units={gpu.get('units',{}).get('val','0')} attrs={attrs} price={r['price']['amount']}{r['price']['denom']}")
EOF
rm -f "$TMPJSON"

echo
echo "=== Chain status ==="
akash status --node "$NODE" 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print('  height:', d['sync_info']['latest_block_height'], ' time:', d['sync_info']['latest_block_time'])"
