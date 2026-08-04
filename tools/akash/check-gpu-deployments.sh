#!/bin/bash
# Check active GPU deployments to understand market pricing
akash query deployment list --node https://akash-rpc.polkachu.com:443 --output json --state active --limit 200 2>/dev/null | python3 -c "
import sys, json
data = json.load(sys.stdin)
deps = data.get('deployments', [])
gpu_deps = []
for d in deps:
    dep = d.get('deployment', {})
    groups = d.get('groups', [])
    for g in groups:
        spec = g.get('group_spec', {})
        for r in spec.get('resources', []):
            res = r.get('resource', {})
            gpu = res.get('gpu', {})
            if gpu and gpu.get('units', {}).get('val', '0') != '0':
                price = r.get('price', {})
                attrs = gpu.get('attributes', [])
                models = [a.get('value') for a in attrs if 'model' in a.get('key', '')]
                gpu_deps.append({
                    'dseq': dep.get('id', {}).get('dseq', '?'),
                    'price': f\"{price.get('amount','?')} {price.get('denom','?')}\",
                    'gpu': f\"{gpu['units']['val']}x {','.join(models) if models else 'any'}\",
                    'cpu': res.get('cpu', {}).get('units', {}).get('val', '?'),
                    'mem': res.get('memory', {}).get('quantity', {}).get('val', '?'),
                    'state': dep.get('state', '?')
                })
print(f'Active GPU deployments: {len(gpu_deps)}')
for g in gpu_deps[:20]:
    print(f\"  dseq={g['dseq']} gpu={g['gpu']} price={g['price']} cpu={g['cpu']} state={g['state']}\")

# Also check CLOSED deployments with GPU to see what price they got leases at
akash query deployment list --node https://akash-rpc.polkachu.com:443 --output json --state closed --limit 50 2>/dev/null | python3 -c \"
import sys, json
data = json.load(sys.stdin)
deps = data.get('deployments', [])
gpu_closed = []
for d in deps:
    dep = d.get('deployment', {})
    groups = d.get('groups', [])
    for g in groups:
        spec = g.get('group_spec', {})
        for r in spec.get('resources', []):
            res = r.get('resource', {})
            gpu = res.get('gpu', {})
            if gpu and gpu.get('units', {}).get('val', '0') != '0':
                price = r.get('price', {})
                gpu_closed.append({
                    'dseq': dep.get('id', {}).get('dseq', '?'),
                    'price': f\\\"{price.get('amount','?')} {price.get('denom','?')}\\\",
                    'gpu': f\\\"{gpu['units']['val']}x\\\"
                })
print(f'\\\\nClosed GPU deployments: {len(gpu_closed)}')
for g in gpu_closed[:10]:
    print(f\\\"  dseq={g['dseq']} gpu={g['gpu']} price={g['price']}\\\")
\"
"
