#!/bin/bash
set -e

NODE="https://akash-rpc.polkachu.com:443"
CHAIN="akashnet-2"
KEYHOME="/tmp/ak-keyring"
SDL="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/deploy/flux-akash-sdl.yaml"
OWNER="akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt"
KEYNAME="flux-deployer"
KEYRING="test"

echo "=== Step 1: Create deployment ==="
DEPLOY=$(akash tx deployment create "$SDL" \
  --node "$NODE" --chain-id "$CHAIN" \
  --from "$KEYNAME" --keyring-backend "$KEYRING" --home "$KEYHOME" \
  --deposit 5000000uact \
  --gas auto --gas-adjustment 1.5 --fees 80000uakt \
  --output json -y 2>&1)

DSEQ=$(echo "$DEPLOY" | python3 -c "
import sys,json,re
d=json.load(sys.stdin)
for e in d.get('events',[]):
    if e['type']=='akash.deployment.v1.EventDeploymentCreated':
        for a in e['attributes']:
            if a['key']=='id':
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

echo "=== Step 2: Wait 40s for bids ==="
sleep 40

BIDS=$(akash query market bid list --owner "$OWNER" --dseq "$DSEQ" \
  --node "$NODE" --chain-id "$CHAIN" --state open --output json 2>/dev/null)

BID_COUNT=$(echo "$BIDS" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('bids',[])))" 2>/dev/null)
echo "Open bids: $BID_COUNT"

if [ "$BID_COUNT" = "0" ]; then
  echo "Waiting 30 more seconds..."
  sleep 30
  BIDS=$(akash query market bid list --owner "$OWNER" --dseq "$DSEQ" \
    --node "$NODE" --chain-id "$CHAIN" --state open --output json 2>/dev/null)
  BID_COUNT=$(echo "$BIDS" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('bids',[])))" 2>/dev/null)
  echo "Open bids: $BID_COUNT"
fi

if [ "$BID_COUNT" = "0" ]; then
  echo "ERROR: No bids. Closing deployment."
  akash tx deployment close --dseq "$DSEQ" --node "$NODE" --chain-id "$CHAIN" \
    --from "$KEYNAME" --keyring-backend "$KEYRING" --home "$KEYHOME" \
    --gas auto --gas-adjustment 1.5 --fees 80000uakt --output json -y 2>&1 | tail -1
  exit 1
fi

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
echo "Cheapest: $PROVIDER at $PRICE"

echo "=== Step 3: Accept bid (create lease) ==="
akash tx market lease create --dseq "$DSEQ" --gseq 1 --provider "$PROVIDER" \
  --node "$NODE" --chain-id "$CHAIN" \
  --from "$KEYNAME" --keyring-backend "$KEYRING" --home "$KEYHOME" \
  --gas auto --gas-adjustment 1.5 --fees 80000uakt --output json -y 2>&1 | tail -1
echo "Lease created."

echo "=== Step 4: Get provider host URI ==="
PROVIDER_INFO=$(akash query provider get "$PROVIDER" --node "$NODE" --chain-id "$CHAIN" --output json 2>/dev/null)
HOST_URI=$(echo "$PROVIDER_INFO" | python3 -c "import sys,json; print(json.load(sys.stdin)['host_uri'])" 2>/dev/null)
echo "Provider URI: $HOST_URI"

echo "=== Step 5: Send manifest to provider ==="
# Generate manifest from SDL and send it
MANIFEST_RESULT=$(akash tx deployment create "$SDL" \
  --node "$NODE" --chain-id "$CHAIN" \
  --from "$KEYNAME" --keyring-backend "$KEYRING" --home "$KEYHOME" \
  --dseq "$DSEQ" \
  --gas auto --gas-adjustment 1.5 --fees 80000uakt \
  --output json -y 2>&1)
echo "Manifest sent (deployment update)."

echo "=== Step 6: Wait for container to start ==="
echo "Waiting 120s for model download + container start..."
sleep 120

echo "=== Step 7: Check lease status ==="
LEASE_STATUS=$(akash query market lease get --dseq "$DSEQ" --gseq 1 --oseq 1 \
  --provider "$PROVIDER" --owner "$OWNER" \
  --node "$NODE" --chain-id "$CHAIN" --output json 2>/dev/null)
LEASE_STATE=$(echo "$LEASE_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['lease']['state'])" 2>/dev/null)
echo "Lease state: $LEASE_STATE"

if [ "$LEASE_STATE" != "active" ]; then
  echo "WARNING: Lease is not active. Provider may have closed it."
  echo "Reason: $(echo "$LEASE_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['lease'].get('reason','unknown'))" 2>/dev/null)"
  echo ""
  echo "The manifest may not have been sent correctly."
  echo "DSEQ: $DSEQ"
  echo "Provider: $PROVIDER"
  exit 1
fi

echo ""
echo "=== Deployment Active ==="
echo "DSEQ: $DSEQ"
echo "Provider: $PROVIDER"
echo "Host URI: $HOST_URI"
echo ""
echo "The container should be running. To find the service URI,"
echo "query the provider's lease status endpoint."
echo ""
echo "To close when done:"
echo "  akash tx deployment close --dseq $DSEQ --node $NODE --chain-id $CHAIN --from $KEYNAME --keyring-backend $KEYRING --home $KEYHOME --gas auto --gas-adjustment 1.5 --fees 80000uakt -y"
