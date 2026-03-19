import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import dotenv from "dotenv";
import { resolve } from "path";
dotenv.config({ path: resolve(import.meta.dirname, "../../../deploy/.env") });

const mnemonic = process.env.MNEMONIC;
if (!mnemonic) {
  console.error("Set MNEMONIC in deploy/.env");
  process.exit(1);
}

// Derive addresses for multiple Cosmos chains from the same mnemonic
const prefixes = [
  { name: "Juno",   prefix: "juno" },
  { name: "Akash",  prefix: "akash" },
  { name: "Cosmos Hub", prefix: "cosmos" },
  { name: "Osmosis", prefix: "osmo" },
];

for (const { name, prefix } of prefixes) {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix });
  const [account] = await wallet.getAccounts();
  console.log(`${name.padEnd(12)} ${account.address}`);
}
