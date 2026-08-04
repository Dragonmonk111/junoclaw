#!/bin/bash
akash query market bid list --node https://akash-rpc.polkachu.com:443 --output json --limit 100 2>/dev/null | python3 -c "
import sys, json
data = json.load(sys.stdin)
bids = data.get('bids', [])
print(f'Total open bids: {len(bids)}')
for b in bids[:30]:
    bid = b.get('bid', {})
    bid_id = bid.get('bid_id', {})
    price = bid.get('price', {})
    resources = bid.get('resources', [])
    has_gpu = False
    gpu_info = ''
    for r in resources:
        res = r.get('resource', {})
        gpu = res.get('gpu', {})
        if gpu and gpu.get('units', {}).get('val', '0') != '0':
            has_gpu = True
            attrs = gpu.get('attributes', [])
            models = [a.get('value') for a in attrs if 'model' in a.get('key', '')]
            gpu_info = f'GPU:{gpu[\"units\"][\"val\"]}x {\",\".join(models) if models else \"any\"}'
    if has_gpu:
        print(f'  GPU BID dseq={bid_id.get(\"dseq\",\"?\")} price={price.get(\"amount\",\"?\")} {price.get(\"denom\",\"?\")} {gpu_info} provider={bid_id.get(\"provider\",\"?\")[:25]}...')
"
