#!/bin/bash
# deploy-and-watch.sh <sdl-file> <deposit-uact> <poll-seconds>
# Creates an Akash deployment, extracts dseq, polls for open bids, prints them.
# Does NOT create a lease — caller decides. Close with close-deployment.mjs or akash tx deployment close.
set -u
SDL="$1"
DEPOSIT="${2:-500000uact}"
POLL_SECS="${3:-300}"
NODE="https://rpc.akashnet.net:443"
FROM="akash-jlens"
OWNER="akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt"

echo "=== Creating deployment from $SDL (deposit $DEPOSIT) ==="
CREATE_OUT=$(akash tx deployment create "$SDL" --from "$FROM" --keyring-backend test \
  --deposit "$DEPOSIT" --gas auto --gas-adjustment 1.4 --fees 20000uakt \
  --chain-id akashnet-2 --node "$NODE" -y -o json 2>&1)
TXHASH=$(echo "$CREATE_OUT" | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); print(d.get("txhash",""))
except Exception:
    print("")')
if [ -z "$TXHASH" ]; then
  echo "DEPLOY CREATE FAILED:"
  echo "$CREATE_OUT" | head -20
  exit 1
fi
echo "txhash: $TXHASH"
sleep 8

DSEQ=$(akash query tx --type hash "$TXHASH" --node "$NODE" -o json 2>/dev/null | \
  python3 -c 'import json,sys
d=json.load(sys.stdin)
for ev in d.get("events",[]):
    for a in ev.get("attributes",[]):
        if a.get("key")=="dseq":
            print(a.get("value")); sys.exit(0)
print("")')
if [ -z "$DSEQ" ]; then
  echo "Could not extract dseq from tx events"
  exit 1
fi
echo "dseq: $DSEQ"

echo "=== Polling bids for up to ${POLL_SECS}s ==="
END=$((SECONDS + POLL_SECS))
while [ $SECONDS -lt $END ]; do
  BIDS=$(akash query market bid list --owner "$OWNER" --dseq "$DSEQ" --state open \
    --node "$NODE" -o json 2>/dev/null)
  COUNT=$(echo "$BIDS" | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); print(len(d.get("bids",[])))
except Exception:
    print(0)')
  echo "[$(date +%H:%M:%S)] open bids: $COUNT"
  if [ "$COUNT" -gt 0 ]; then
    echo "$BIDS" | python3 -c 'import json,sys
d=json.load(sys.stdin)
for b in d.get("bids",[]):
    bid=b.get("bid",{})
    price=bid.get("price",{})
    prov=bid.get("id",{}).get("provider","?")
    print(f"  provider={prov} price={price.get(\"amount\")}{price.get(\"denom\")}/block state={bid.get(\"state\")}")'
    break
  fi
  sleep 30
done

echo "DSEQ=$DSEQ"
echo "To close: akash tx deployment close --dseq $DSEQ --owner $OWNER --from $FROM --keyring-backend test --fees 20000uakt --chain-id akashnet-2 --node $NODE -y"
