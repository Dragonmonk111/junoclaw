#!/bin/bash
set -e

NODE="https://akash-rpc.polkachu.com:443"
CHAIN="akashnet-2"
KEYHOME="/tmp/ak-keyring"
SDL="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/deploy/flux-akash-sdl.yaml"
OWNER="akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt"

echo "=== Creating deployment ==="
DEPLOY=$(akash tx deployment create "$SDL" \
  --node "$NODE" --chain-id "$CHAIN" \
  --from flux-deployer --keyring-backend test --home "$KEYHOME" \
  --deposit 5000000uact \
  --gas auto --gas-adjustment 1.5 --fees 80000uakt \
  --output json -y 2>&1)

DSEQ=$(echo "$DEPLOY" | python3 -c "
import sys,json
d=json.load(sys.stdin)
for e in d.get('events',[]):
    if e['type']=='akash.deployment.v1.EventDeploymentCreated':
        for a in e['attributes']:
            if a['key']=='id':
                import re
                m=re.search(r'dseq.{0,5}?(\d+)', a['value'])
                if m: print(m.group(1)); sys.exit()
print(''); sys.exit(1)
" 2>/dev/null)

if [ -z "$DSEQ" ]; then
  echo "ERROR: Could not extract DSEQ"
  echo "$DEPLOY" | head -c 500
  exit 1
fi

echo "DSEQ=$DSEQ"
echo "Waiting 40s for bids..."
sleep 40

BIDS=$(akash query market bid list --owner "$OWNER" --dseq "$DSEQ" \
  --node "$NODE" --chain-id "$CHAIN" --state open --output json 2>/dev/null)

BID_COUNT=$(echo "$BIDS" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('bids',[])))" 2>/dev/null)
echo "Open bids: $BID_COUNT"

if [ "$BID_COUNT" = "0" ]; then
  echo "No open bids yet, waiting 30 more seconds..."
  sleep 30
  BIDS=$(akash query market bid list --owner "$OWNER" --dseq "$DSEQ" \
    --node "$NODE" --chain-id "$CHAIN" --state open --output json 2>/dev/null)
  BID_COUNT=$(echo "$BIDS" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('bids',[])))" 2>/dev/null)
  echo "Open bids: $BID_COUNT"
fi

if [ "$BID_COUNT" = "0" ]; then
  echo "ERROR: No bids. Closing deployment."
  akash tx deployment close --dseq "$DSEQ" --node "$NODE" --chain-id "$CHAIN" \
    --from flux-deployer --keyring-backend test --home "$KEYHOME" \
    --gas auto --gas-adjustment 1.5 --fees 80000uakt --output json -y 2>&1 | tail -1
  exit 1
fi

# Pick cheapest bid
PROVIDER=$(echo "$BIDS" | python3 -c "
import sys,json
bids=json.load(sys.stdin)['bids']
bids.sort(key=lambda b: float(b['bid']['price']['amount']))
print(bids[0]['bid']['id']['provider'])
")
PRICE=$(echo "$BIDS" | python3 -c "
import sys,json
bids=json.load(sys.stdin)['bids']
bids.sort(key=lambda b: float(b['bid']['price']['amount']))
print(bids[0]['bid']['price']['amount'], bids[0]['bid']['price']['denom'])
")
GPU=$(echo "$BIDS" | python3 -c "
import sys,json
bids=json.load(sys.stdin)['bids']
bids.sort(key=lambda b: float(b['bid']['price']['amount']))
r=bids[0].get('resources_offer',[{}])[0].get('resources',{}).get('gpu',{})
attrs=r.get('attributes',[])
for a in attrs:
    if 'model' in a.get('key',''): print(a['key'],'=',a['value']); break
else: print('unknown gpu')
")

echo "Cheapest: $PROVIDER at $PRICE ($GPU)"

echo "Accepting bid..."
akash tx market lease create --dseq "$DSEQ" --gseq 1 --provider "$PROVIDER" \
  --node "$NODE" --chain-id "$CHAIN" \
  --from flux-deployer --keyring-backend test --home "$KEYHOME" \
  --gas auto --gas-adjustment 1.5 --fees 80000uakt --output json -y 2>&1 | tail -1

echo "Lease accepted! DSEQ=$DSEQ PROVIDER=$PROVIDER"
echo ""
echo "Waiting 60s for container to start..."
sleep 60

echo "Checking lease status..."
akash query market lease status --dseq "$DSEQ" --gseq 1 --provider "$PROVIDER" \
  --node "$NODE" --chain-id "$CHAIN" --output json 2>/dev/null | python3 -c "
import sys,json
s=json.load(sys.stdin)
for svc in s.get('services',[]):
    name=svc.get('name','?')
    state=svc.get('state','?')
    uris=svc.get('uris',[])
    ports=svc.get('forwarded_ports',{})
    print(f'Service: {name}, State: {state}, URIs: {uris}, Ports: {ports}')
" 2>/dev/null || echo "Status query failed"

echo ""
echo "=== Deployment active ==="
echo "DSEQ: $DSEQ"
echo "Provider: $PROVIDER"
echo ""
echo "To get the URI:"
echo "  akash query market lease status --dseq $DSEQ --gseq 1 --provider $PROVIDER --node $NODE --chain-id $CHAIN"
echo ""
echo "To close when done:"
echo "  akash tx deployment close --dseq $DSEQ --node $NODE --chain-id $CHAIN --from flux-deployer --keyring-backend test --home $KEYHOME --gas auto --gas-adjustment 1.5 --fees 80000uakt -y"
