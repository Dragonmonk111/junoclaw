import { execFileSync } from "child_process";

const dseq = process.argv[2] || "27947243";
const result = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "query", "market", "bid", "list",
  "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt",
  "--dseq", dseq,
  "--node", "https://akash-rpc.polkachu.com:443",
  "--output", "json"], { encoding: "utf-8", timeout: 30000 });

const data = JSON.parse(result);
const bids = data.bids || [];
console.log(`Bids for dseq=${dseq}: ${bids.length}`);
for (const b of bids) {
  const bid = b.bid;
  const id = bid.id || bid.bid_id;
  console.log(`  provider=${id.provider} state=${bid.state} price=${bid.price.amount} ${bid.price.denom} created_at=${bid.created_at}`);
}
