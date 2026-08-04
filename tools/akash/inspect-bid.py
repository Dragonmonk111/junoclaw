import subprocess, json, sys

result = subprocess.run(
    ["akash", "query", "market", "bid", "list",
     "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt",
     "--dseq", sys.argv[1],
     "--node", "https://akash-rpc.polkachu.com:443",
     "--output", "json"],
    capture_output=True, text=True
)
data = json.loads(result.stdout)
bids = data.get("bids", [])
print(f"Bids: {len(bids)}")
if bids:
    print(f"First bid structure:\n{json.dumps(bids[0], indent=2)[:2000]}")
