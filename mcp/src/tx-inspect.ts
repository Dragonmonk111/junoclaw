import { JunoClawClient } from "./utils/junoclaw-client.js";

async function main() {
  const client = new JunoClawClient("51.195.201.74:9090", "0.025ujclaw");
  const tx = await client.getTx(process.argv[2]!) as any;
  console.log(JSON.stringify(tx, null, 1).slice(0, 4000));
}
main().catch((e) => { console.error("err:", e); process.exit(1); });
