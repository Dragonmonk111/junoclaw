import json
d = json.load(open('/tmp/providers.json'))
for p in d:
    gpus = p.get('gpus') or []
    for g in gpus:
        model = (g.get('model') or '').lower() if isinstance(g, dict) else str(g).lower()
        if 'h100' in model or 'h200' in model:
            print(json.dumps(p, indent=2)[:1500])
            print('---')
            break
    else:
        continue
    break
