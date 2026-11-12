import { getDefaultWalletStore } from "./wallet/store.js";

async function main() {
  const store = getDefaultWalletStore();
  const list = await store.list();
  console.log("existing:", list.map((e) => e.id).join(", "));
  const found = list.find((e) => e.id === "probe-op-b");
  if (found) {
    console.log("probe-op-b exists:", found.address);
    return;
  }
  const entry = await store.generateAndAdd("probe-op-b", { bech32Prefix: "junoclaw" });
  console.log("created probe-op-b:", entry.address);
}
main().then(() => process.exit(0)).catch((e) => { console.error("err:", e); process.exit(1); });
